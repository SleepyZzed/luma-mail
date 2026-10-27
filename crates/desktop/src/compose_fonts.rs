//! Installed font metadata only. Fonts are never copied into drafts or caches.
use super::*;
pub(super) const SIZES: [u16; 13] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 32, 48, 72];
pub(super) fn controls() -> (gtk::FontDialogButton, gtk::DropDown) {
    let family = gtk::FontDialogButton::new(Some(gtk::FontDialog::new()));
    family.set_level(gtk::FontLevel::Family);
    family.set_use_font(false);
    family.set_use_size(false);
    family.set_font_desc(&gtk::pango::FontDescription::from_string("Sans"));
    family.set_tooltip_text(Some(
        "Font family · All installed system fonts, including installed Microsoft fonts",
    ));
    let names: Vec<_> = SIZES.iter().map(|v| format!("{v} pt")).collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    let size = gtk::DropDown::from_strings(&names);
    size.set_selected(3);
    size.set_tooltip_text(Some("Email font size · Independent of interface scaling"));
    (family, size)
}
pub(super) fn family_tag(buffer: &gtk::TextBuffer, family: &str) -> gtk::TextTag {
    let name = format!("font:family:{family}");
    buffer.tag_table().lookup(&name).unwrap_or_else(|| {
        let tag = gtk::TextTag::new(Some(&name));
        tag.set_family(Some(family));
        buffer.tag_table().add(&tag);
        tag
    })
}
pub(super) fn size_tag(e: &Editor, size: u16) -> gtk::TextTag {
    let buffer = e.text.buffer();
    let name = format!("font:size:{size}");
    let tag = buffer.tag_table().lookup(&name).unwrap_or_else(|| {
        let tag = gtk::TextTag::new(Some(&name));
        buffer.tag_table().add(&tag);
        tag
    });
    tag.set_size_points(size as f64 * crate::presentation::scale(&e.state));
    tag
}
fn apply(e: &Editor, begin: &gtk::TextIter, end: &gtk::TextIter) {
    let buffer = e.text.buffer();
    let (family, size) = e.typing_font.borrow().clone();
    buffer.tag_table().foreach(|tag| {
        if tag.name().is_some_and(|n| n.starts_with("font:")) {
            buffer.remove_tag(tag, begin, end);
        }
    });
    if let Some(family) = family {
        buffer.apply_tag(&family_tag(&buffer, &family), begin, end);
    }
    if let Some(size) = size {
        buffer.apply_tag(&size_tag(e, size), begin, end);
    }
}
fn chosen(e: &Rc<Editor>) {
    if e.font_sync.get() {
        return;
    }
    if e.html.is_active() {
        e.status
            .set_text("Switch off HTML source to use the font picker.");
        return;
    }
    let family = e
        .font_family
        .font_desc()
        .and_then(|d| d.family())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "Sans".into());
    if family.len() > 128 || family.chars().any(char::is_control) {
        return;
    }
    if e.text.buffer().tag_table().size() > 256 {
        e.status
            .set_text("This draft has too many formatting styles.");
        return;
    }
    super::history::checkpoint(e);
    let size = SIZES
        .get(e.font_size.selected() as usize)
        .copied()
        .unwrap_or(11);
    *e.typing_font.borrow_mut() = (Some(family), Some(size));
    if let Some((begin, end)) = e.text.buffer().selection_bounds() {
        apply(e, &begin, &end);
        changed(e);
    }
    e.text.grab_focus();
    super::history::checkpoint(e);
}
pub(super) fn install(e: &Rc<Editor>, toolbar: &gtk::FlowBox) {
    let group = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    group.append(&e.font_family);
    group.append(&e.font_size);
    toolbar.insert(&group, -1);
    let reset = ui::button(
        "luma-clear-symbolic",
        "Reset font for selection or further typing",
    );
    reset.add_css_class("flat");
    group.append(&reset);
    let weak = Rc::downgrade(e);
    e.font_family.connect_font_desc_notify(move |_| {
        if let Some(e) = weak.upgrade() {
            chosen(&e);
        }
    });
    let weak = Rc::downgrade(e);
    e.font_size.connect_selected_notify(move |_| {
        if let Some(e) = weak.upgrade() {
            chosen(&e);
        }
    });
    let weak = Rc::downgrade(e);
    reset.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            super::history::checkpoint(&e);
            *e.typing_font.borrow_mut() = (None, None);
            if let Some((a, z)) = e.text.buffer().selection_bounds() {
                apply(&e, &a, &z);
                changed(&e);
            }
            sync_controls(&e);
            e.text.grab_focus();
            super::history::checkpoint(&e);
        }
    });
    let weak = Rc::downgrade(e);
    e.text.buffer().connect_insert_text(move |_, _, _| {
        if let Some(e) = weak.upgrade() {
            e.font_inserting.set(true);
        }
    });
    let weak = Rc::downgrade(e);
    e.text
        .buffer()
        .connect_local("insert-text", true, move |values| {
            if let Some(e) = weak.upgrade() {
                if !e.html.is_active() && !e.history.restoring.get() {
                    let end = values[1]
                        .get::<gtk::TextIter>()
                        .expect("insert-text iterator");
                    let text = values[2].get::<String>().expect("insert-text contents");
                    let begin = e
                        .text
                        .buffer()
                        .iter_at_offset(end.offset() - text.chars().count() as i32);
                    apply(&e, &begin, &end);
                }
                e.font_inserting.set(false);
            }
            None
        });
    let weak = Rc::downgrade(e);
    e.text.buffer().connect_delete_range(move |_, _, _| {
        if let Some(e) = weak.upgrade() {
            e.font_inserting.set(true);
        }
    });
    let weak = Rc::downgrade(e);
    e.text
        .buffer()
        .connect_local("delete-range", true, move |_| {
            if let Some(e) = weak.upgrade() {
                e.font_inserting.set(false);
            }
            None
        });
    let weak = Rc::downgrade(e);
    e.text.buffer().connect_mark_set(move |buffer, at, mark| {
        let Some(e) = weak.upgrade() else {
            return;
        };
        if mark.name().as_deref() != Some("insert") || e.font_inserting.get() {
            return;
        }
        let at = buffer
            .selection_bounds()
            .map(|(a, _)| a)
            .unwrap_or_else(|| {
                let mut prev = *at;
                if prev.is_end() {
                    prev.backward_char();
                }
                prev
            });
        let mut style = (None, None);
        for tag in at.tags() {
            if let Some(name) = tag.name() {
                if let Some(family) = name.strip_prefix("font:family:") {
                    style.0 = Some(family.into());
                }
                if let Some(size) = name.strip_prefix("font:size:") {
                    style.1 = size.parse().ok();
                }
            }
        }
        *e.typing_font.borrow_mut() = style;
        sync_controls(&e);
    });
}
pub(super) fn sync_controls(e: &Editor) {
    e.font_sync.set(true);
    let (family, size) = e.typing_font.borrow().clone();
    let mut desc = gtk::pango::FontDescription::new();
    desc.set_family(family.as_deref().unwrap_or("Sans"));
    e.font_family.set_font_desc(&desc);
    e.font_size
        .set_selected(SIZES.iter().position(|v| Some(*v) == size).unwrap_or(3) as u32);
    e.font_sync.set(false);
}
pub(super) fn rescale(e: &Editor) {
    e.text.buffer().tag_table().foreach(|tag| {
        if let Some(size) = tag.name().and_then(|n| {
            n.strip_prefix("font:size:")
                .and_then(|n| n.parse::<u16>().ok())
        }) {
            tag.set_size_points(size as f64 * crate::presentation::scale(&e.state));
        }
    });
    e.window
        .set_decorated(e.state.settings.borrow().window_chrome);
    e.header.set_show_end_title_buttons(
        e.placement.get() == 2 && e.state.settings.borrow().window_chrome,
    );
}
pub(super) fn open(tag: &str) -> Option<String> {
    if let Some(family) = tag.strip_prefix("font:family:") {
        // CSS quoted-string escaping, followed by HTML attribute escaping.
        let css: String = family
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                    c.to_string()
                } else {
                    format!("\\{:x} ", c as u32)
                }
            })
            .collect();
        Some(format!(
            "<span data-luma-family=\"{}\" style=\"font-family:{}\">",
            escape(family),
            escape(&format!("'{css}',sans-serif"))
        ))
    } else {
        tag.strip_prefix("font:size:")
            .and_then(|n| n.parse::<u16>().ok())
            .filter(|v| (8..=72).contains(v))
            .map(|n| format!("<span data-luma-size=\"{n}\" style=\"font-size:{n}pt\">"))
    }
}
#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    let b = e.text.buffer();
    b.set_text("Font sample");
    b.select_range(&b.start_iter(), &b.end_iter());
    e.font_family
        .set_font_desc(&gtk::pango::FontDescription::from_string("Serif"));
    e.font_size.set_selected(6);
    let before = formatted_html(&b);
    assert_eq!(
        luma_core::rich_draft::parse(&before).unwrap().text,
        "Font sample"
    );
    assert!(before.contains("'Serif',sans-serif"));
    assert!(before.contains("font-size:16pt"));
    b.place_cursor(&b.end_iter());
    b.insert_at_cursor(" next");
    assert!(
        b.iter_at_offset(b.char_count() - 1)
            .tags()
            .iter()
            .any(|t| t.name().as_deref() == Some("font:size:16"))
    );
    let before = formatted_html(&b);
    e.state.settings.borrow_mut().ui_scale_percent = 150;
    crate::presentation::apply(&e.state);
    assert_eq!(
        formatted_html(&b),
        before,
        "UI scaling must not change outgoing email font size"
    );
    assert_eq!(
        b.tag_table().lookup("font:size:16").unwrap().size_points(),
        24.
    );
    let outgoing = snapshot(e).unwrap();
    assert!(outgoing.html);
    let restored =
        luma_core::rich_draft::parse(&outgoing.text).expect("own rich draft imports visually");
    assert_eq!(restored.text, "Font sample next");
    assert!(restored.spans.iter().any(|s| s.tag == "font:family:Serif"));
    assert!(restored.spans.iter().any(|s| s.tag == "font:size:16"));
    e.state.settings.borrow_mut().reduced_motion = true;
    crate::presentation::apply(&e.state);
    assert_eq!(crate::presentation::duration(&e.state, 260), 0);
    assert_eq!(e.state.compose_stack.transition_duration(), 0);
    e.state.settings.borrow_mut().reduced_motion = false;
    e.state.settings.borrow_mut().ui_scale_percent = 100;
    crate::presentation::apply(&e.state);
    println!(
        "PASS installed-font selection, typing, rich draft restoration, independent UI scaling and reduced motion"
    );
}
