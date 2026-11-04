//! Review › Check Accessibility: what on the page is hard to follow with a screen reader.

use notecraft_doc::PageObject;
use serde_json::{Value, json};

use crate::ops::{need_page, no_page};
use crate::{CmdResult, CommandSpec, Session, p};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Review.Accessibility.AccessibilityChecker: the checks are NoteCraft's own choice
        // (a missing title, pictures without alt text, tables without a header row or with
        // merged cells); OneNote's list of rules is unverified.
        CommandSpec::new(
            "review.accessibility",
            "R.Review.Accessibility.AccessibilityChecker",
            "Check Accessibility",
            "Review/Accessibility/Check Accessibility",
            check,
        )
        .params(r#"{"open"?: bool (the pane; default: switch it)} → {"issues": [{"kind", "object"?, "message"}]}"#)
        .pure()
        .when(need_page),
    ]
}

fn check(s: &mut Session, v: &Value) -> CmdResult {
    let page = s.page().ok_or_else(no_page)?;
    let mut issues = Vec::new();
    if page.title.text().trim().is_empty() {
        issues.push(json!({"kind": "title", "message": "The page has no title"}));
    }
    for o in &page.objects {
        match o {
            PageObject::Image(im) if im.alt_text.trim().is_empty() => {
                issues.push(json!({"kind": "altText", "object": im.id.to_string(), "message": "A picture has no alternative text"}));
            }
            PageObject::Table(t) => {
                if !t.header_row {
                    issues.push(json!({"kind": "tableHeader", "object": t.id.to_string(), "message": "A table has no header row"}));
                }
                if t.rows.iter().flat_map(|r| r.cells.iter()).any(|c| c.row_span > 1 || c.col_span > 1) {
                    issues.push(json!({"kind": "mergedCells", "object": t.id.to_string(), "message": "A table has merged cells"}));
                }
            }
            _ => {}
        }
    }
    s.view.accessibility_pane = p::bool(v, "open").unwrap_or(!s.view.accessibility_pane);
    Ok(json!({"issues": issues}))
}
