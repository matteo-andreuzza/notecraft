//! The built-in sample notebook (`--sample`): original NoteCraft text, used for screenshots,
//! demos and tests. Nothing here is copied from any other product's sample content.

use std::sync::Arc;

use notecraft_doc::geom::Point;
use notecraft_doc::{
    CharFormat, IdGen, ListFormat, NavNode, Notebook, Numbering, Outline, Page, PageObject, ParaStyle, Paragraph, Run, Section, SectionColor,
    SectionGroup, TagUse, Timestamp, builtin_tags,
};

fn para(ids: &mut IdGen, text: &str) -> Paragraph {
    Paragraph::with_text(ids.next_id(), text)
}

fn tagged(ids: &mut IdGen, text: &str, key: &str, checked: bool) -> Paragraph {
    let mut p = para(ids, text);
    if let Some(def) = builtin_tags().into_iter().find(|d| d.key == key) {
        let mut t = TagUse::of(&def);
        t.checked = checked;
        p.tags.push(t);
    }
    p
}

fn page(ids: &mut IdGen, title: &str, now: Timestamp, level: u8) -> Page {
    let mut p = Page::new(ids.next_id(), ids.next_id(), now, "NoteCraft");
    p.title = Paragraph::with_text(p.title.id, title);
    p.title.style = ParaStyle::PageTitle;
    p.level = level;
    p
}

fn outline(ids: &mut IdGen, x: f32, y: f32, paras: Vec<Paragraph>) -> PageObject {
    let mut o = Outline::new(ids.next_id(), Point::new(x, y), ids.next_id());
    o.paragraphs = paras;
    o.normalize();
    PageObject::Outline(o)
}

/// Build the sample notebook.
pub fn sample_notebook(ids: &mut IdGen, now: Timestamp) -> Notebook {
    let mut nb = Notebook::new(ids.next_id(), "Sample Notebook", SectionColor::index(4));

    // Getting started.
    let mut welcome = page(ids, "Welcome to NoteCraft", now, 0);
    let mut h = para(ids, "Your notes, your way");
    h.style = ParaStyle::Heading1;
    let mut b1 = para(ids, "Click anywhere on a page and start typing: a note container appears where you clicked.");
    b1.list = Some(ListFormat::Bullet(notecraft_doc::Bullet::Disc));
    let mut b2 = para(ids, "Drag a container by its top bar; drag its right edge to set its width.");
    b2.list = Some(ListFormat::Bullet(notecraft_doc::Bullet::Disc));
    let mut b3 = para(ids, "Press Tab to indent a paragraph and Shift+Tab to bring it back.");
    b3.list = Some(ListFormat::Bullet(notecraft_doc::Bullet::Disc));
    let mut b3a = para(ids, "Indented paragraphs move with their parent (Alt+Shift+Up/Down).");
    b3a.level = 1;
    b3a.list = Some(ListFormat::Bullet(notecraft_doc::Bullet::Circle));
    let mut tip = Paragraph::new(ids.next_id());
    tip.runs = vec![
        Run::new("Tags ", CharFormat { bold: Some(true), ..Default::default() }),
        Run::new("mark what matters: press Ctrl+1 for a to-do, Ctrl+2 for important, and open ", CharFormat::default()),
        Run::new("Find Tags", CharFormat { italic: Some(true), ..Default::default() }),
        Run::new(" to see them all.", CharFormat::default()),
    ];
    welcome.objects.push(outline(ids, 36.0, 86.0, vec![h, b1, b2, b3, b3a, tip]));
    let mut note = para(ids, "Everything is saved as you type. Notebooks are plain folders on your disk.");
    note.runs[0].fmt.highlight = Some(notecraft_doc::color::DEFAULT_HIGHLIGHT);
    welcome.objects.push(outline(ids, 36.0, 270.0, vec![note]));

    let mut shortcuts = page(ids, "Handy shortcuts", now, 1);
    let mut k = Vec::new();
    for line in [
        "Ctrl+N  new page",
        "Ctrl+T  new section",
        "Ctrl+E  search every notebook",
        "Ctrl+F  find on this page",
        "Alt+Shift+1…9  show outline levels",
    ] {
        let mut p = para(ids, line);
        p.list = Some(ListFormat::Number { style: Numbering::Decimal, restart: None });
        k.push(p);
    }
    shortcuts.objects.push(outline(ids, 36.0, 86.0, k));

    let mut start = Section::new(ids.next_id(), "Getting Started", SectionColor::index(0), now);
    start.pages = vec![Arc::new(welcome), Arc::new(shortcuts)];

    // Meetings.
    let mut weekly = page(ids, "Weekly planning", now, 0);
    let mut agenda = para(ids, "Agenda");
    agenda.style = ParaStyle::Heading2;
    let a1 = para(ids, "Review last week's goals");
    let a2 = para(ids, "Garden club newsletter: layout and deadline");
    let a3 = para(ids, "Budget for spring seeds");
    let mut actions = para(ids, "Action items");
    actions.style = ParaStyle::Heading2;
    let t1 = tagged(ids, "Send the newsletter draft to Robin", "todo", true);
    let t2 = tagged(ids, "Order tomato and basil seeds", "todo", false);
    let t3 = tagged(ids, "Ask about the shared greenhouse key", "question", false);
    let t4 = tagged(ids, "Next meeting moves to Thursday", "important", false);
    weekly.objects.push(outline(ids, 36.0, 86.0, vec![agenda, a1, a2, a3, actions, t1, t2, t3, t4]));

    let mut retro = page(ids, "Retrospective", now, 0);
    let went_well = para(ids, "What went well: the seed swap drew twice the expected crowd.");
    let improve = tagged(ids, "Improve: start the sign-up sheet a week earlier.", "remember-for-later", false);
    retro.objects.push(outline(ids, 36.0, 86.0, vec![went_well, improve]));

    let mut meetings = Section::new(ids.next_id(), "Meetings", SectionColor::index(1), now);
    meetings.pages = vec![Arc::new(weekly), Arc::new(retro)];

    // Projects (a section group).
    let mut plan = page(ids, "Raised beds", now, 0);
    let mut steps = Vec::new();
    for (i, s) in ["Measure the south fence", "Buy untreated boards", "Fill with compost and soil", "Plant herbs along the edges"].iter().enumerate()
    {
        let mut p = para(ids, s);
        p.list = Some(ListFormat::Number { style: Numbering::Decimal, restart: None });
        if i == 2 {
            let mut sub = para(ids, "Two parts soil, one part compost");
            sub.level = 1;
            sub.list = Some(ListFormat::Number { style: Numbering::LowerAlpha, restart: None });
            steps.push(p);
            steps.push(sub);
            continue;
        }
        steps.push(p);
    }
    plan.objects.push(outline(ids, 36.0, 86.0, steps));
    let mut ideas = page(ids, "Ideas", now, 0);
    let idea1 = tagged(ids, "A rain barrel under the shed gutter", "idea", false);
    let idea2 = tagged(ids, "Visit the botanical garden library", "web-site-to-visit", false);
    ideas.objects.push(outline(ids, 36.0, 86.0, vec![idea1, idea2]));
    let mut garden = Section::new(ids.next_id(), "Garden", SectionColor::index(2), now);
    garden.pages = vec![Arc::new(plan), Arc::new(ideas)];
    let group = SectionGroup { id: ids.next_id(), name: "Projects".into(), children: vec![NavNode::Section(Arc::new(garden))] };

    let mut quick = Section::new(ids.next_id(), "Quick Notes", SectionColor::index(5), now);
    let mut qn = page(ids, "Phone message", now, 0);
    let call = tagged(ids, "Call the hardware store back about the boards", "phone-number", false);
    qn.objects.push(outline(ids, 36.0, 86.0, vec![call]));
    quick.pages = vec![Arc::new(qn)];

    nb.children =
        vec![NavNode::Section(Arc::new(start)), NavNode::Section(Arc::new(meetings)), NavNode::Group(group), NavNode::Section(Arc::new(quick))];
    nb.normalize();
    nb
}
