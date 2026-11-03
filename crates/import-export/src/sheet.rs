//! Spreadsheets in (catalog `R.Insert.Files.InsertExistingExcelSpreadsheet`): the first
//! worksheet of an Excel workbook (`.xlsx`, SpreadsheetML from ECMA-376 Part 1, read from the
//! public standard) or a CSV file, as rows of cell text. Values only: formulas give their
//! cached result, formats and other sheets are left out. Bounded and never panics.

use std::io::{Cursor, Read};

/// Most rows read.
pub const MAX_ROWS: usize = 500;
/// Most columns read.
pub const MAX_COLS: usize = 64;
/// Largest part read from a workbook.
const MAX_PART: u64 = 32 << 20;

/// The cells of `bytes`: a workbook (a zip) or CSV text. Trailing empty rows and columns go.
pub fn read(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut grid = if bytes.starts_with(b"PK") { xlsx(bytes)? } else { csv(&String::from_utf8_lossy(bytes)) };
    while grid.last().is_some_and(|r| r.iter().all(|c| c.trim().is_empty())) {
        grid.pop();
    }
    let width = grid.iter().map(|r| r.iter().rposition(|c| !c.trim().is_empty()).map_or(0, |i| i + 1)).max().unwrap_or(0);
    for r in &mut grid {
        r.resize(width, String::new());
    }
    Ok(grid)
}

fn part(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.take(MAX_PART).read_to_string(&mut s).ok()?;
    Some(s)
}

fn xlsx(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a workbook: {e}"))?;
    let shared = part(&mut zip, "xl/sharedStrings.xml").map(|x| elements(&x, "si").into_iter().map(texts).collect::<Vec<_>>()).unwrap_or_default();
    // The first worksheet: sheet1, else the first one by name.
    let name = if zip.by_name("xl/worksheets/sheet1.xml").is_ok() {
        "xl/worksheets/sheet1.xml".to_string()
    } else {
        let mut names: Vec<String> = zip
            .file_names()
            .filter_map(|n| n.ok().map(|n| n.to_string()))
            .filter(|n| n.starts_with("xl/worksheets/") && n.ends_with(".xml"))
            .collect();
        names.sort();
        names.into_iter().next().ok_or("the workbook has no worksheet")?
    };
    let sheet = part(&mut zip, &name).ok_or("the worksheet can't be read")?;
    let mut grid: Vec<Vec<String>> = Vec::new();
    for row in elements(&sheet, "row").into_iter().take(MAX_ROWS) {
        let r = attr(open_tag(row), "r").and_then(|n| n.parse::<usize>().ok()).unwrap_or(grid.len() + 1).clamp(1, MAX_ROWS);
        let mut cells: Vec<String> = Vec::new();
        for c in elements(row, "c") {
            let tag = open_tag(c);
            let col = attr(tag, "r").map(|r| column(&r)).unwrap_or(cells.len());
            if col >= MAX_COLS {
                continue;
            }
            let value = match attr(tag, "t").as_deref() {
                Some("s") => first(c, "v").and_then(|v| v.trim().parse::<usize>().ok()).and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                Some("inlineStr") => texts(c),
                Some("b") => {
                    if first(c, "v").map(str::trim) == Some("1") {
                        "TRUE".into()
                    } else {
                        "FALSE".into()
                    }
                }
                _ => first(c, "v").map(unescape).unwrap_or_default(),
            };
            if cells.len() <= col {
                cells.resize(col + 1, String::new());
            }
            cells[col] = value;
        }
        if grid.len() < r {
            grid.resize(r, Vec::new());
        }
        grid[r - 1] = cells;
    }
    Ok(grid)
}

/// The column of a cell reference (`B3` → 1).
fn column(r: &str) -> usize {
    r.chars()
        .take_while(char::is_ascii_alphabetic)
        .fold(0usize, |n, ch| n.saturating_mul(26).saturating_add((ch.to_ascii_uppercase() as usize).saturating_sub(64)))
        .saturating_sub(1)
}

/// The text of every `<t>` inside `x`, joined.
fn texts(x: &str) -> String {
    elements(x, "t").into_iter().map(|t| inner(t).map(unescape).unwrap_or_default()).collect()
}

fn first<'a>(x: &'a str, name: &str) -> Option<&'a str> {
    elements(x, name).into_iter().next().and_then(inner)
}

/// Each `<name …>…</name>` (or `<name …/>`) in `x`, outermost only.
fn elements<'a>(x: &'a str, name: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{name}"), format!("</{name}>"));
    let mut out = Vec::new();
    let mut rest = x;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        // `<c` must not match `<col`: the name ends at a space, `>` or `/`.
        if !after.starts_with([' ', '>', '/', '\t', '\n', '\r']) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            out.push(&rest[i..i + open.len() + gt + 1]);
            rest = &after[gt + 1..];
            continue;
        }
        let Some(end) = after.find(&close) else { break };
        out.push(&rest[i..i + open.len() + end + close.len()]);
        rest = &after[end + close.len()..];
    }
    out
}

fn open_tag(el: &str) -> &str {
    el.find('>').map_or(el, |i| &el[..i])
}

fn inner(el: &str) -> Option<&str> {
    let start = el.find('>')? + 1;
    let end = el.rfind("</")?;
    el.get(start..end)
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let end = tag[i..].find('"')?;
    Some(unescape(&tag[i..i + end]))
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// CSV with `,`, `;` or tab (whichever the first line has most of), quotes doubling.
fn csv(text: &str) -> Vec<Vec<String>> {
    let head = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| head.matches(*c).count()).unwrap_or(',');
    let mut rows = vec![Vec::new()];
    let (mut cell, mut quoted) = (String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if rows.len() > MAX_ROWS {
            break;
        }
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => push_cell(&mut rows, &mut cell),
            '\n' if !quoted => {
                push_cell(&mut rows, &mut cell);
                rows.push(Vec::new());
            }
            '\r' if !quoted => {}
            c => cell.push(c),
        }
    }
    push_cell(&mut rows, &mut cell);
    rows.truncate(MAX_ROWS);
    rows
}

fn push_cell(rows: &mut [Vec<String>], cell: &mut String) {
    if let Some(r) = rows.last_mut()
        && r.len() < MAX_COLS
    {
        r.push(std::mem::take(cell));
    }
    cell.clear();
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn workbook(sheet: &str, shared: Option<&str>) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("xl/worksheets/sheet1.xml", o).unwrap();
        z.write_all(sheet.as_bytes()).unwrap();
        if let Some(s) = shared {
            z.start_file("xl/sharedStrings.xml", o).unwrap();
            z.write_all(s.as_bytes()).unwrap();
        }
        z.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_the_first_sheet_of_a_workbook() {
        let sheet = r#"<worksheet><cols><col min="1"/></cols><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" t="inlineStr"><is><t>Q &amp; A</t></is></c></row><row r="3"><c r="A3"><v>42</v></c><c r="B3" t="b"><v>1</v></c></row></sheetData></worksheet>"#;
        let shared = r#"<sst><si><t>Name</t></si></sst>"#;
        let g = read(&workbook(sheet, Some(shared))).unwrap();
        assert_eq!(
            g,
            vec![vec!["Name".to_string(), String::new(), "Q & A".into()], vec![String::new(); 3], vec!["42".into(), "TRUE".into(), String::new()]]
        );
        assert!(read(b"PK\x03\x04 broken").is_err());
    }

    #[test]
    fn reads_csv_with_quotes_and_its_separator() {
        assert_eq!(read(b"a;b\n\"x;y\";\"say \"\"hi\"\"\"\n\n").unwrap(), vec![vec!["a", "b"], vec!["x;y", "say \"hi\""]]);
        assert_eq!(column("AB12"), 27);
    }
}
