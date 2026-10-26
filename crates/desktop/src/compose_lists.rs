//! Paragraph-based bullet editing. Prefix edits preserve the text buffer's tags,
//! selection marks and undo grouping.
use gtk::gdk;
use gtk::prelude::*;

use luma_core::lists::{Kind, marker};
pub fn prefix(line: &str) -> Option<(usize, i32)> {
    marker(line).map(|m| (m.depth, m.chars))
}
fn line(buffer: &gtk::TextBuffer, n: i32) -> (gtk::TextIter, String) {
    let start = buffer.iter_at_line(n).unwrap_or_else(|| buffer.end_iter());
    let mut end = start;
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    (start, buffer.text(&start, &end, false).into())
}
fn selected_lines(buffer: &gtk::TextBuffer) -> (i32, i32) {
    let (a, z) = buffer.selection_bounds().unwrap_or_else(|| {
        let i = buffer.iter_at_offset(buffer.cursor_position());
        (i, i)
    });
    (
        a.line(),
        (z.line() - i32::from(z.line() > a.line() && z.line_offset() == 0)).max(a.line()),
    )
}
fn delete_prefix(buffer: &gtk::TextBuffer, mut start: gtk::TextIter, count: i32) {
    let mut end = start;
    end.forward_chars(count);
    buffer.delete(&mut start, &mut end);
}
pub fn toggle(buffer: &gtk::TextBuffer) {
    toggle_kind(buffer, Kind::Bullet);
}
pub fn toggle_kind(buffer: &gtk::TextBuffer, kind: Kind) {
    let (a, z) = selected_lines(buffer);
    let remove = (a..=z).all(|n| marker(&line(buffer, n).1).is_some_and(|m| m.kind == kind));
    buffer.begin_user_action();
    for n in (a..=z).rev() {
        let (mut start, text) = line(buffer, n);
        let old = marker(&text);
        if let Some(m) = old {
            delete_prefix(buffer, start, m.chars);
        }
        if !remove {
            start = buffer.iter_at_line(n).unwrap();
            let depth = old.map(|m| m.depth).unwrap_or(0);
            let prefix = if kind == Kind::Bullet {
                format!("{}• ", "  ".repeat(depth))
            } else {
                format!("{}{}. ", "  ".repeat(depth), n - a + 1)
            };
            buffer.insert(&mut start, &prefix);
        }
    }
    renumber(buffer, a);
    buffer.end_user_action();
}
fn renumber(buffer: &gtk::TextBuffer, at: i32) {
    let mut first = at;
    while first > 0 && marker(&line(buffer, first - 1).1).is_some() {
        first -= 1;
    }
    let mut counters = [0u32; 7];
    let mut n = first;
    while n < buffer.line_count() {
        let (start, text) = line(buffer, n);
        let Some(m) = marker(&text) else {
            break;
        };
        for count in &mut counters[m.depth + 1..] {
            *count = 0;
        }
        if m.kind == Kind::Numbered {
            let number = if counters[m.depth] == 0 {
                if n == first { m.number } else { 1 }
            } else {
                counters[m.depth].saturating_add(1).min(999999)
            };
            counters[m.depth] = number;
            if number != m.number {
                delete_prefix(buffer, start, m.chars);
                let mut start = buffer.iter_at_line(n).unwrap();
                buffer.insert(&mut start, &format!("{}{}. ", "  ".repeat(m.depth), number));
            }
        } else {
            counters[m.depth] = 0;
        }
        n += 1;
    }
}
fn indent(buffer: &gtk::TextBuffer, backwards: bool) -> bool {
    let (a, z) = selected_lines(buffer);
    if !(a..=z).any(|n| prefix(&line(buffer, n).1).is_some()) {
        return false;
    }
    buffer.begin_user_action();
    for n in (a..=z).rev() {
        let (mut start, text) = line(buffer, n);
        if let Some((level, count)) = prefix(&text) {
            if backwards {
                delete_prefix(buffer, start, if level > 0 { 2 } else { count });
            } else if level < 6 {
                buffer.insert(&mut start, "  ");
            }
        }
    }
    renumber(buffer, a);
    buffer.end_user_action();
    true
}
pub fn key(buffer: &gtk::TextBuffer, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    if modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
        return false;
    }
    if matches!(key, gdk::Key::Tab | gdk::Key::ISO_Left_Tab) {
        return indent(
            buffer,
            key == gdk::Key::ISO_Left_Tab || modifiers.contains(gdk::ModifierType::SHIFT_MASK),
        );
    }
    if buffer.selection_bounds().is_some() {
        return false;
    }
    let caret = buffer.iter_at_offset(buffer.cursor_position());
    let (start, text) = line(buffer, caret.line());
    let Some((level, count)) = prefix(&text) else {
        return false;
    };
    if key == gdk::Key::Return
        && !modifiers.contains(gdk::ModifierType::SHIFT_MASK)
        && caret.line_offset() >= count
    {
        buffer.begin_user_action();
        if text.chars().skip(count as usize).all(char::is_whitespace) {
            let line_index = caret.line();
            delete_prefix(buffer, start, count);
            buffer.place_cursor(
                &buffer
                    .iter_at_line(line_index)
                    .unwrap_or_else(|| buffer.end_iter()),
            );
        } else {
            let m = marker(&text).unwrap();
            let next = if m.kind == Kind::Numbered {
                format!(
                    "{}{}. ",
                    "  ".repeat(level),
                    m.number.saturating_add(1).min(999999)
                )
            } else {
                text.chars().take(count as usize).collect()
            };
            let n = caret.line();
            buffer.insert_at_cursor(&format!("\n{next}"));
            renumber(buffer, n);
        }
        buffer.end_user_action();
        return true;
    }
    if key == gdk::Key::BackSpace && caret.line_offset() > 0 && caret.line_offset() <= count {
        buffer.begin_user_action();
        delete_prefix(buffer, start, if level > 0 { 2 } else { count });
        buffer.end_user_action();
        return true;
    }
    false
}
#[cfg(any(debug_assertions, test))]
pub fn verify() {
    let b = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    b.set_enable_undo(true);
    b.set_text("First paragraph\nSecond paragraph");
    b.place_cursor(&b.iter_at_offset(5));
    toggle(&b);
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "• First paragraph\nSecond paragraph"
    );
    assert_eq!(
        b.cursor_position(),
        7,
        "bullet belongs at paragraph start and preserves caret content position"
    );
    b.undo();
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "First paragraph\nSecond paragraph"
    );
    b.select_range(&b.start_iter(), &b.end_iter());
    toggle(&b);
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "• First paragraph\n• Second paragraph"
    );
    b.place_cursor(&b.end_iter());
    assert!(key(&b, gdk::Key::Return, gdk::ModifierType::empty()));
    assert!(
        b.text(&b.start_iter(), &b.end_iter(), false)
            .ends_with("\n• ")
    );
    assert!(key(&b, gdk::Key::Return, gdk::ModifierType::empty()));
    assert!(
        b.text(&b.start_iter(), &b.end_iter(), false)
            .ends_with("paragraph\n")
    );
    b.set_text("• Item");
    b.place_cursor(&b.end_iter());
    assert!(key(&b, gdk::Key::Tab, gdk::ModifierType::empty()));
    assert_eq!(b.text(&b.start_iter(), &b.end_iter(), false), "  • Item");
    assert!(key(
        &b,
        gdk::Key::ISO_Left_Tab,
        gdk::ModifierType::SHIFT_MASK
    ));
    assert_eq!(b.text(&b.start_iter(), &b.end_iter(), false), "• Item");
    b.set_text("One\nTwo");
    b.select_range(&b.start_iter(), &b.end_iter());
    toggle_kind(&b, Kind::Numbered);
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "1. One\n2. Two"
    );
    b.place_cursor(&b.end_iter());
    assert!(key(&b, gdk::Key::Tab, gdk::ModifierType::empty()));
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "1. One\n  1. Two"
    );
    assert!(key(
        &b,
        gdk::Key::ISO_Left_Tab,
        gdk::ModifierType::SHIFT_MASK
    ));
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "1. One\n2. Two"
    );
    b.place_cursor(&b.end_iter());
    assert!(key(&b, gdk::Key::Return, gdk::ModifierType::empty()));
    assert!(
        b.text(&b.start_iter(), &b.end_iter(), false)
            .ends_with("\n3. ")
    );
    assert!(key(&b, gdk::Key::Return, gdk::ModifierType::empty()));
    assert!(
        b.text(&b.start_iter(), &b.end_iter(), false)
            .ends_with("Two\n")
    );
}
#[test]
fn recognizes_paragraph_bullets_and_depth() {
    assert_eq!(prefix("  • hello"), Some((1, 4)));
    assert_eq!(prefix("hello • world"), None);
}
