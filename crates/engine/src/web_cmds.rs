//! Commands that hand the web to the browser: Research and Online Pictures search for the
//! selected text (or a given query) on the web, Online Video links a video's address into the
//! page, Show Training opens the project's guide. NoteCraft has no web panes of its own.

use serde_json::{Value, json};

use crate::ops::need_page;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The project's guide (Show Training).
const TRAINING_URL: &str = "https://github.com/matteo-andreuzza/notecraft#readme";

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Help.Help.ShowTraining — unverified: the reference opens Microsoft's training videos.
        CommandSpec::new("help.training", "R.Help.Help.ShowTraining", "Show Training", "Help/Help/Show Training", |_, _| {
            Ok(json!({"open_url": TRAINING_URL}))
        })
        .params(r#"{} → {"open_url"}"#)
        .pure(),
        // R.Review.Proofing.ResearchPane — unverified: the reference opens a Research task pane.
        CommandSpec::new("review.research", "R.Review.Proofing.ResearchPane", "Research", "Review/Proofing/Research", |s, v| {
            search(s, v, "https://duckduckgo.com/?q=")
        })
        .params(r#"{"query"?: text (default: the selected text)} → {"open_url"}"#)
        .pure(),
        // R.Insert.Images.ClipArtInsertDialog — pictures are found on the web, then inserted from a file.
        CommandSpec::new(
            "insert.onlinePicture",
            "R.Insert.Images.ClipArtInsertDialog",
            "Online Pictures",
            "Insert/Images/Online Pictures",
            |s, v| search(s, v, "https://commons.wikimedia.org/w/index.php?ns6=1&search="),
        )
        .params(r#"{"query"?: text (default: the selected text)} → {"open_url"}"#)
        .pure(),
        // R.Insert.Media.InsertOnlineVideo — unverified: the reference embeds a player; here the
        // video is a link.
        CommandSpec::new("insert.onlineVideo", "R.Insert.Media.InsertOnlineVideo", "Online Video", "Insert/Media/Online Video", online_video)
            .params(r#"{"url": http(s) address of the video, "text"?: link text}"#)
            .when(need_page),
    ]
}

fn search(s: &Session, v: &Value, base: &str) -> CmdResult {
    let query = p::str(v, "query").map(str::to_string).or_else(|| s.selected_text()).unwrap_or_default();
    let query = query.trim();
    if query.is_empty() {
        return Err(CmdError::Failed("Select some text to search for first.".into()));
    }
    let q: String = query.chars().take(200).collect();
    Ok(json!({"open_url": format!("{base}{}", encode(&q))}))
}

/// Percent-encode a query component.
fn encode(s: &str) -> String {
    s.bytes().fold(String::new(), |mut out, b| {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
        out
    })
}

fn online_video(s: &mut Session, v: &Value) -> CmdResult {
    let url = p::str(v, "url").unwrap_or_default().trim().to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.len() > 2048 || url.contains(char::is_whitespace) {
        return Err(CmdError::Params("`url`: the http(s) address of the video".into()));
    }
    let text = p::str(v, "text").filter(|t| !t.trim().is_empty()).map(str::to_string).unwrap_or_else(|| url.clone());
    s.run("insert.link", json!({"url": url, "text": text}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::tests::{blank_page, run, sample};

    #[test]
    fn the_web_commands_hand_over_addresses() {
        // parity: R.Help.Help.ShowTraining R.Review.Proofing.ResearchPane R.Insert.Images.ClipArtInsertDialog R.Insert.Media.InsertOnlineVideo
        let mut s = sample();
        blank_page(&mut s);
        assert!(run(&mut s, "help.training", json!({}))["open_url"].as_str().unwrap().starts_with("https://"));
        let r = run(&mut s, "review.research", json!({"query": "a b&c"}));
        assert!(r["open_url"].as_str().unwrap().ends_with("a%20b%26c"));
        assert!(s.run("review.research", json!({})).is_err(), "needs something to search for");
        assert!(run(&mut s, "insert.onlinePicture", json!({"query": "cat"}))["open_url"].as_str().unwrap().contains("search=cat"));
        assert!(s.run("insert.onlineVideo", json!({"url": "javascript:alert(1)"})).is_err());
        run(&mut s, "insert.onlineVideo", json!({"url": "https://example.org/v", "text": "Demo"}));
        // parity: R.View.Immersive.ImmersiveReader
        assert_eq!(run(&mut s, "view.immersiveReader", json!({}))["open"], true);
        assert_eq!(run(&mut s, "view.immersiveReader", json!({}))["open"], false);
    }
}
