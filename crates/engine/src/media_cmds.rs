//! Audio and video recordings (Insert › Recording, the Audio & Video tab): the desktop host
//! records with the system's tools while the toggle is on and hands the finished file to
//! `insert.audio` / `insert.video`, which attach it to the page like any other file; Play opens
//! the attached recording with the system player (decision: no audio stack inside NoteCraft).

use notecraft_doc::media::MAX_ATTACHMENT_BYTES;
use serde_json::{Value, json};

use crate::ops::need_page;
use crate::picture_cmds::{need_attachment, open_attachment, push_attachment, read_input, room_for_one};
use crate::selection::PageSel;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Insert.Recording.AudioRecordingInsert — a toggle: the UI starts and stops the
        // recording; the finished file arrives here.
        CommandSpec::new("insert.audio", "R.Insert.Recording.AudioRecordingInsert", "Record Audio", "Insert/Recording/Record Audio", |s, v| {
            attach(s, v, "Audio recording", "ogg")
        })
        .params(r#"{"data": base64 recording, "name"?: file name, "x"?: pt, "y"?: pt}"#)
        .when(need_page),
        CommandSpec::new("insert.video", "R.Insert.Recording.VideoRecordingInsert", "Record Video", "Insert/Recording/Record Video", |s, v| {
            attach(s, v, "Video recording", "mkv")
        })
        .params(r#"{"data": base64 recording, "name"?: file name, "x"?: pt, "y"?: pt}"#)
        .when(need_page),
        // CM.FileAttachment.PlayFromPage / CT.AudioVideo.Playback.RecordingPlay — the system
        // player plays the attached recording (the host opens the returned path).
        CommandSpec::new("media.play", "CM.FileAttachment.PlayFromPage", "Play", "Context/File Attachment/Play (attached media)", open_attachment)
            .key("Mod+Alt+P")
            .params(r#"{"dir"?: folder for the copy} → {"path"} for the host to open"#)
            .pure()
            .when(need_attachment),
    ]
}

/// Attach `v`'s recording; without a name, "<kind> <date> <time>.<ext>".
fn attach(s: &mut Session, v: &Value, kind: &str, ext: &str) -> CmdResult {
    let (bytes, _) = read_input(v, MAX_ATTACHMENT_BYTES)?;
    if bytes.is_empty() {
        return Err(CmdError::Failed("The recording is empty.".into()));
    }
    let name = match p::str(v, "name").filter(|n| !n.trim().is_empty()) {
        Some(n) => n.to_string(),
        None => {
            let (now, off) = (s.now(), s.clock.utc_offset_min);
            format!("{kind} {} {}.{ext}", now.short_date(off), now.short_time(off)).replace(['/', ':'], "-")
        }
    };
    room_for_one(s)?;
    let id = push_attachment(s, v, name.clone(), bytes)?;
    s.sel.on_page = PageSel::Objects { ids: vec![id] };
    Ok(json!({"attachment": id.to_string(), "name": name}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::tests::{blank_page, run, sample};

    #[test]
    fn a_recording_becomes_an_attachment() {
        // parity: R.Insert.Recording.AudioRecordingInsert
        let mut s = sample();
        blank_page(&mut s);
        let r = run(&mut s, "insert.audio", json!({"data": crate::b64::encode(b"OggS...")}));
        assert!(r["name"].as_str().unwrap().starts_with("Audio recording"));
        assert!(r["name"].as_str().unwrap().ends_with(".ogg"));
        let r = run(&mut s, "insert.video", json!({"data": crate::b64::encode(b"mkv"), "name": "Talk.mkv"}));
        assert_eq!(r["name"], "Talk.mkv");
        assert!(s.run("insert.audio", json!({"data": ""})).is_err(), "an empty recording is refused");
        // parity: CM.FileAttachment.PlayFromPage
        let r = run(&mut s, "media.play", json!({"dir": std::env::temp_dir().join("notecraft-media-test").display().to_string()}));
        assert!(r["path"].as_str().unwrap().ends_with("Talk.mkv"));
    }
}
