//! **[MS-CAB]** cabinet files, the container of a `.onepkg` notebook package: the files it
//! holds, with their paths, decompressed in memory.
//!
//! Read-only and bounded like the rest of the crate: every offset is checked against the file,
//! the number of files and the bytes decompressed are capped by [`CabLimits`], and a damaged
//! folder costs only its own files (they are listed in [`Cabinet::warnings`]). Supported
//! compression: none and MSZIP ([MS-MCI] §2: RFC 1951 blocks behind a `CK` signature, with the
//! history kept from block to block). Quantum and LZX, and cabinets split over several files,
//! are refused with [`OneError::Unsupported`].

use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::{DecompressorOxide, decompress, inflate_flags};

use crate::bytes::Cursor;
use crate::error::{OneError, Result};

/// Limits against hostile cabinets.
#[derive(Debug, Clone, Copy)]
pub struct CabLimits {
    /// Files read (CFHEADER.cFiles can claim 65,535).
    pub max_files: usize,
    /// Bytes decompressed in total: the size of the memory the files take.
    pub max_total_bytes: u64,
}

impl Default for CabLimits {
    fn default() -> Self {
        Self { max_files: 10_000, max_total_bytes: 4 << 30 }
    }
}

/// One file of a cabinet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CabFile {
    /// The path inside the cabinet, by component (`Work\Notes.one` → `["Work", "Notes.one"]`);
    /// never empty, never `.` or `..`.
    pub path: Vec<String>,
    /// The file's bytes.
    pub data: Vec<u8>,
}

/// The files of a cabinet, and what could not be read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cabinet {
    /// The files read, in the cabinet's order.
    pub files: Vec<CabFile>,
    /// Files skipped, with the reason.
    pub warnings: Vec<String>,
}

/// §2.1.10 CFHEADER.flags.
const PREV_CABINET: u16 = 0x0001;
const NEXT_CABINET: u16 = 0x0002;
const RESERVE_PRESENT: u16 = 0x0004;
/// §2.3.3 iFolder values of files continued from or to another cabinet (0xFFFD and above).
const IFOLD_CONTINUED: u16 = 0xFFFD;
/// §2.3.6 _A_NAME_IS_UTF.
const NAME_IS_UTF: u16 = 0x80;
/// §2.1.17, §2.3.7: names are NUL-terminated, at most 255 bytes plus the NUL (`szName` has no
/// stated limit; 1 KiB stops a runaway read).
const MAX_NAME: usize = 1024;
/// [MS-MCI] §2: an MSZIP block holds at most 32 KB, and its history is the previous 32 KB.
const MSZIP_WINDOW: usize = 32 * 1024;

/// §2.2 CFFOLDER.
struct Folder {
    /// §2.2.1 coffCabStart.
    start: u32,
    /// §2.2.2 cCFData.
    blocks: u16,
    /// §2.2.3 typeCompress (low four bits).
    compress: u16,
}

/// §2.3 CFFILE.
struct Entry {
    size: u32,
    offset: u32,
    folder: u16,
    path: Vec<String>,
}

/// Reads the cabinet in `file`.
pub fn read_cabinet(file: &[u8], limits: &CabLimits) -> Result<Cabinet> {
    let mut c = Cursor::new(file, 0);
    // §2.1 CFHEADER.
    if c.array::<4>("CFHEADER.signature")? != *b"MSCF" {
        return Err(OneError::Unsupported("not a cabinet (.onepkg) file".into()));
    }
    c.skip(4, "CFHEADER.reserved1")?;
    c.skip(4, "CFHEADER.cbCabinet")?;
    c.skip(4, "CFHEADER.reserved2")?;
    let coff_files = c.u32("CFHEADER.coffFiles")?;
    // §3 example: a third reserved field sits before the version.
    c.skip(4, "CFHEADER.reserved3")?;
    let _minor = c.u8("CFHEADER.versionMinor")?;
    let major = c.u8("CFHEADER.versionMajor")?;
    if major != 1 {
        return Err(OneError::Unsupported(format!("cabinet format version {major}")));
    }
    let n_folders = c.u16("CFHEADER.cFolders")?;
    let n_files = c.u16("CFHEADER.cFiles")?;
    let flags = c.u16("CFHEADER.flags")?;
    c.skip(4, "CFHEADER.setID, iCabinet")?;
    if flags & (PREV_CABINET | NEXT_CABINET) != 0 {
        return Err(OneError::Unsupported("packages split over several cabinet files".into()));
    }
    let (mut folder_reserve, mut data_reserve) = (0usize, 0usize);
    if flags & RESERVE_PRESENT != 0 {
        // §2.1.13–2.1.16: the reserve sizes, then the header's own reserve (≤ 60,000).
        let header_reserve = c.u16("CFHEADER.cbCFHeader")?;
        folder_reserve = usize::from(c.u8("CFHEADER.cbCFFolder")?);
        data_reserve = usize::from(c.u8("CFHEADER.cbCFData")?);
        if header_reserve > 60_000 {
            return Err(OneError::corrupt(c.at(), "CFHEADER.cbCFHeader is over 60,000"));
        }
        c.skip(usize::from(header_reserve), "CFHEADER.abReserve")?;
    }
    if usize::from(n_files) > limits.max_files {
        return Err(OneError::TooLarge(format!("{n_files} files in the package (the limit is {})", limits.max_files)));
    }
    // §2.2: the CFFOLDER entries follow the header.
    let mut folders = Vec::with_capacity(usize::from(n_folders));
    for _ in 0..n_folders {
        let start = c.u32("CFFOLDER.coffCabStart")?;
        let blocks = c.u16("CFFOLDER.cCFData")?;
        let compress = c.u16("CFFOLDER.typeCompress")? & 0x000F;
        c.skip(folder_reserve, "CFFOLDER.abReserve")?;
        folders.push(Folder { start, blocks, compress });
    }
    // §2.3: the CFFILE entries, from coffFiles.
    let mut c = Cursor::new(
        file.get(coff_files as usize..).ok_or_else(|| OneError::corrupt(0x10, "CFHEADER.coffFiles is past the end"))?,
        u64::from(coff_files),
    );
    let mut entries = Vec::with_capacity(usize::from(n_files));
    for _ in 0..n_files {
        let size = c.u32("CFFILE.cbFile")?;
        let offset = c.u32("CFFILE.uoffFolderStart")?;
        let folder = c.u16("CFFILE.iFolder")?;
        c.skip(4, "CFFILE.date, time")?;
        let attribs = c.u16("CFFILE.attribs")?;
        let name = name(&mut c, attribs)?;
        if folder >= IFOLD_CONTINUED {
            return Err(OneError::Unsupported("packages split over several cabinet files".into()));
        }
        entries.push(Entry { size, offset, folder, path: path(&name) });
    }
    let mut cab = Cabinet::default();
    let mut total = 0u64;
    for (index, folder) in folders.iter().enumerate() {
        let mine: Vec<&Entry> = entries.iter().filter(|e| usize::from(e.folder) == index).collect();
        if mine.is_empty() {
            continue;
        }
        // Only as much of the folder as its files reach is decompressed.
        let needed = mine.iter().map(|e| u64::from(e.offset) + u64::from(e.size)).max().unwrap_or(0);
        if total.saturating_add(needed) > limits.max_total_bytes {
            return Err(OneError::TooLarge(format!("the package unpacks to more than {} MiB", limits.max_total_bytes >> 20)));
        }
        let data = match folder_data(file, folder, data_reserve, needed) {
            Ok(d) => d,
            Err(e) => {
                cab.warnings.extend(mine.iter().map(|f| format!("{}: {e}", f.path.join("/"))));
                continue;
            }
        };
        total = total.saturating_add(data.len() as u64);
        for f in mine {
            let end = f.offset.checked_add(f.size);
            match end.and_then(|end| data.get(f.offset as usize..end as usize)) {
                Some(bytes) if !f.path.is_empty() => cab.files.push(CabFile { path: f.path.clone(), data: bytes.to_vec() }),
                Some(_) => cab.warnings.push("a file without a name was skipped".into()),
                None => cab.warnings.push(format!("{}: its data lies past the end of its folder", f.path.join("/"))),
            }
        }
    }
    for e in entries.iter().filter(|e| usize::from(e.folder) >= folders.len()) {
        cab.warnings.push(format!("{}: CFFILE.iFolder names a folder that doesn't exist", e.path.join("/")));
    }
    Ok(cab)
}

/// §2.3.7 szName: UTF-8 with _A_NAME_IS_UTF, else the locale's code page (Windows-1252 here,
/// the code page of the reference's Western installs — unverified for others).
fn name(c: &mut Cursor<'_>, attribs: u16) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        let b = c.u8("CFFILE.szName")?;
        if b == 0 {
            break;
        }
        if bytes.len() >= MAX_NAME {
            return Err(OneError::corrupt(c.at(), "CFFILE.szName is longer than 1 KiB"));
        }
        bytes.push(b);
    }
    Ok(if attribs & NAME_IS_UTF != 0 { String::from_utf8_lossy(&bytes).into_owned() } else { crate::one::windows_1252(&bytes) })
}

/// A stored name as path components: `\` and `/` separate, and empty, `.` and `..` parts are
/// dropped (a forged name must not climb out of the package).
fn path(name: &str) -> Vec<String> {
    name.split(['\\', '/']).filter(|p| !p.is_empty() && *p != "." && *p != "..").map(str::to_string).collect()
}

/// §2.4 CFDATA: the folder's blocks decompressed, until `needed` bytes are out.
fn folder_data(file: &[u8], folder: &Folder, reserve: usize, needed: u64) -> Result<Vec<u8>> {
    match folder.compress {
        0 | 1 => {}
        2 => return Err(OneError::Unsupported("Quantum compression".into())),
        3 => return Err(OneError::Unsupported("LZX compression".into())),
        n => return Err(OneError::Unsupported(format!("compression type {n}"))),
    }
    let mut c = Cursor::new(
        file.get(folder.start as usize..).ok_or_else(|| OneError::corrupt(0, "CFFOLDER.coffCabStart is past the end"))?,
        u64::from(folder.start),
    );
    let mut out: Vec<u8> = Vec::new();
    for _ in 0..folder.blocks {
        if out.len() as u64 >= needed {
            break;
        }
        // The checksum is optional (§2.4.1: zero when not supplied) and damage shows anyway in
        // the decompressor and the size checks, so it is not verified.
        c.skip(4, "CFDATA.csum")?;
        let packed = usize::from(c.u16("CFDATA.cbData")?);
        let size = usize::from(c.u16("CFDATA.cbUncomp")?);
        c.skip(reserve, "CFDATA.abReserve")?;
        let at = c.at();
        let ab = c.take(packed, "CFDATA.ab")?;
        if size == 0 {
            // §2.4.3: a block continued in the next cabinet.
            return Err(OneError::Unsupported("packages split over several cabinet files".into()));
        }
        if folder.compress == 0 {
            if packed != size {
                return Err(OneError::corrupt(at, "an uncompressed CFDATA block whose sizes differ"));
            }
            out.extend_from_slice(ab);
        } else {
            mszip_block(ab, size, &mut out).map_err(|what| OneError::corrupt(at, what))?;
        }
    }
    if (out.len() as u64) < needed {
        return Err(OneError::corrupt(c.at(), "the folder holds less data than its files claim"));
    }
    Ok(out)
}

/// [MS-MCI] §2.1: one MSZIP block (`CK`, then RFC 1951 blocks ending with a final one) giving
/// exactly `size` bytes (≤ 32 KB), which may refer back into the previous 32 KB of `out`.
fn mszip_block(ab: &[u8], size: usize, out: &mut Vec<u8>) -> std::result::Result<(), &'static str> {
    let deflate = ab.strip_prefix(b"CK").ok_or("an MSZIP block without its CK signature")?;
    if size > MSZIP_WINDOW {
        return Err("an MSZIP block of more than 32 KB");
    }
    let start = out.len();
    let history = start.saturating_sub(MSZIP_WINDOW);
    out.resize(start + size, 0);
    let window = out.get_mut(history..).ok_or("MSZIP history out of range")?;
    // A fresh decoder per block (its trees are discarded) on a buffer that starts with the
    // history: references before the buffer start are refused by the decoder.
    let mut d = DecompressorOxide::new();
    let (status, _, written) = decompress(&mut d, deflate, window, start - history, inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF);
    if status != TINFLStatus::Done || written != size {
        out.truncate(start);
        return Err("an MSZIP block that doesn't decompress to its stated size");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
