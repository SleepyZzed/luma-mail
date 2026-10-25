//! Explicit debug-only synthetic fixture. No remote accounts or email actions.
use crate::{
    attachments::{self, Source},
    ui::{self, State, label, vbox},
};
use adw::prelude::*;
use luma_core::{
    config::Account,
    model::{Conversation, ThreadKey},
    preview::Kind,
};
use std::rc::Rc;

pub fn show(s: &Rc<State>) {
    s.window.set_title(Some("Luma Mail · Synthetic preview"));
    assert!(
        s.clients.borrow().is_empty(),
        "Use a fresh isolated test root"
    );
    let account = Account {
        id: "design-fixture".into(),
        email: "alex@example.com".into(),
        name: "Alex Morgan".into(),
        enabled: true,
        notifications: false,
        signature: String::new(),
        drive_enabled: false,
        sender_controls: false,
        blocked_senders: vec![],
    };
    s.settings.borrow_mut().accounts.push(account);
    s.local_compose
        .set_accounts(s.settings.borrow().accounts.iter().map(|a| a.id.clone()));
    ui::sidebar(s);
    let items = [
        (
            "Jamie Chen",
            "Weekend by the coast",
            "A few photos and the plan for Saturday. See you there!",
            true,
            true,
        ),
        (
            "Studio North",
            "Your next chapter starts here",
            "A little inspiration for your next creative project.",
            true,
            false,
        ),
        (
            "Morgan Lee",
            "Design review · next steps",
            "Thanks everyone. I’ve attached the notes from today.",
            false,
            true,
        ),
        (
            "Sam Rivera",
            "Coffee next week?",
            "That place on the corner has finally opened.",
            false,
            false,
        ),
        (
            "The Reading Room",
            "Something good to read",
            "Five stories for a quieter Sunday.",
            false,
            false,
        ),
    ];
    for (i, (sender, subject, snippet, unread, attached)) in items.iter().enumerate() {
        let row = gtk::ListBoxRow::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        ui::margins(&content, 14);
        content.append(&adw::Avatar::new(40, Some(sender), true));
        let text = vbox(5);
        text.set_hexpand(true);
        text.append(&label(
            sender,
            if *unread { "heading" } else { "mail-sender" },
        ));
        text.append(&label(subject, "mail-subject"));
        let snippet = label(snippet, "mail-snippet");
        snippet.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&snippet);
        content.append(&text);
        if *attached {
            content.append(&gtk::Image::from_icon_name("mail-attachment-symbolic"));
        }
        row.add_css_class("mail-row");
        row.set_child(Some(&content));
        s.list.append(&row);
        s.rows.borrow_mut().push(Conversation {
            key: ThreadKey {
                account: "design-fixture".into(),
                id: i.to_string(),
            },
            sender: sender.to_string(),
            subject: subject.to_string(),
            snippet: String::new(),
            date: 0,
            unread: *unread,
            starred: false,
            attachments: *attached,
            count: 1,
        });
    }
    ui::clear(&s.reader);
    s.reader
        .append(&label("Weekend by the coast", "reader-subject"));
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.add_css_class("reader-toolbar");
    for (icon, title) in [
        ("luma-archive-symbolic", "Archive"),
        ("user-trash-symbolic", "Trash"),
        ("mail-unread-symbolic", "Unread"),
        ("non-starred-symbolic", "Star"),
        ("luma-tag-symbolic", "Labels"),
    ] {
        let b = ui::button(icon, title);
        b.add_css_class("flat");
        bar.append(&b);
    }
    s.reader.append(&bar);
    let card = vbox(18);
    card.add_css_class("mail-card");
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    heading.append(&adw::Avatar::new(40, Some("Jamie Chen"), true));
    let name = vbox(3);
    name.set_hexpand(true);
    name.append(&label("Jamie Chen", "heading"));
    name.append(&label("to Alex Morgan", "caption"));
    heading.append(&name);
    heading.append(&label("Today · 10:42", "caption"));
    card.append(&heading);
    let strip = vbox(6);
    strip.add_css_class("attachment-strip");
    strip.append(&label("Attachments", "caption"));
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(8)
        .row_spacing(8)
        .max_children_per_line(2)
        .min_children_per_line(1)
        .build();
    strip.append(&flow);
    let mut paths = Vec::new();
    if let Some(root) = std::env::var_os("LUMA_FIXTURE_DIR") {
        for filename in [
            "Coast.jpg",
            "Weekend plan.docx",
            "Voice note.wav",
            "Coast.mp4",
        ] {
            let path = std::path::PathBuf::from(&root).join(filename);
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let c = attachments::card(filename, "", meta.len(), false);
            let state = s.clone();
            let source = Source::Local(path.clone());
            let open_source = source.clone();
            let bytes = meta.len();
            c.open.connect_clicked(move |_| {
                attachments::open(&state, filename, "", bytes, open_source.clone())
            });
            let state = s.clone();
            let save_source = source.clone();
            c.action
                .connect_clicked(move |_| attachments::save(&state, filename, save_source.clone()));
            flow.insert(&c.root, -1);
            if Kind::of(filename, "") == Kind::Image {
                attachments::thumbnail(s, &c, source);
            }
            paths.push(path);
        }
    }
    card.append(&strip);
    card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let text = label(
        "Hey Alex,\n\nHere’s everything for the weekend — a photo from last time, the plan, and a quick voice note.\n\nThe forecast looks great. Let’s meet at the station at 9:30, grab a coffee, and head down together.\n\nSee you Saturday!\nJamie",
        "reader-body",
    );
    text.set_wrap(true);
    text.set_selectable(true);
    card.append(&text);
    let reply = gtk::Button::with_label("Reply");
    reply.set_halign(gtk::Align::Start);
    card.append(&reply);
    let state = s.clone();
    let compose_paths = paths.clone();
    let slot = card.clone();
    reply.connect_clicked(move |_| {
        crate::compose::design_reply(&state, compose_paths.clone(), slot.clone())
    });
    s.reader.append(&card);
    s.info("Synthetic design preview · No remote mailbox connected");
    crate::compose::verify_formatting();
    if std::env::args().any(|a| a == "--compose-diagnostics") {
        crate::compose::diagnostic(s);
        return;
    }
    if std::env::args().any(|a| a == "--preview-html") {
        ui::clear(&s.reader);
        s.reader
            .append(&label("Newsletter rendering check", "reader-subject"));
        let html=std::env::var_os("LUMA_PREVIEW_HTML").and_then(|p|std::fs::read_to_string(p).ok()).unwrap_or_else(|| r##"<table width="100%" style="background:#E5E3E8" cellpadding="24"><tr><td><table width="600" align="center" style="background:#fff;text-align:center" cellpadding="32"><tr><td><h1 style="color:#9146ff">Newsletter</h1><p style="font-size:24px">A proper HTML email</p><p>Tables, colours, headings and links stay in their original layout.</p><p><a href="https://example.com" style="background:#9147ff;color:white;display:inline-block;padding:12px 32px">Watch now</a></p></td></tr></table></td></tr></table>"##.into());
        crate::rendering::preview_html(s, &s.reader, html);
    }
    if std::env::args().any(|a| a == "--preview-compose") {
        crate::compose::design_preview(s, paths);
    }
}
