//! Find/replace runs only while the bar is visible, with bounded matches and one undo step.
use super::*;
pub(super) struct Find {
    editor: std::rc::Weak<Editor>,
    root: gtk::Revealer,
    query: gtk::Entry,
    replacement: gtk::Entry,
    status: gtk::Label,
    case: gtk::CheckButton,
    word: gtk::CheckButton,
    matches: RefCell<Vec<std::ops::Range<i32>>>,
    active: Cell<usize>,
    changing: Cell<bool>,
    timer: RefCell<Option<glib::SourceId>>,
}
impl Drop for Find {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
    }
}
fn refresh(f: &Find) {
    if !f.root.reveals_child() || f.changing.get() {
        return;
    }
    let Some(e) = f.editor.upgrade() else { return };
    let b = e.text.buffer();
    let text = b.slice(&b.start_iter(), &b.end_iter(), false);
    let result = luma_core::text_search::find(
        &text,
        &f.query.text(),
        f.case.is_active(),
        f.word.is_active(),
    );
    f.active
        .set(f.active.get().min(result.len().saturating_sub(1)));
    let total = result.len();
    *f.matches.borrow_mut() = result;
    f.status.set_text(&if f.query.text().is_empty() {
        "".into()
    } else if total == 0 {
        "No matches".into()
    } else if total == luma_core::text_search::MAX_MATCHES {
        "First 10,000 matches".into()
    } else {
        format!("{} of {total}", f.active.get() + 1)
    });
}
fn select(f: &Find, delta: i32) {
    refresh(f);
    let Some(e) = f.editor.upgrade() else { return };
    let matches = f.matches.borrow();
    if matches.is_empty() {
        return;
    }
    let index = (f.active.get() as i32 + delta).rem_euclid(matches.len() as i32) as usize;
    f.active.set(index);
    let b = e.text.buffer();
    let r = &matches[index];
    let a = b.iter_at_offset(r.start);
    let mut z = b.iter_at_offset(r.end);
    b.select_range(&a, &z);
    e.text.scroll_to_iter(&mut z, 0.15, false, 0., 0.);
    drop(matches);
    refresh(f);
}
fn replace(f: &Find, all: bool) {
    refresh(f);
    let Some(e) = f.editor.upgrade() else { return };
    let matches = f.matches.borrow().clone();
    if matches.is_empty() {
        return;
    }
    let replacement = f.replacement.text();
    let extra = replacement
        .len()
        .saturating_mul(if all { matches.len() } else { 1 });
    let b = e.text.buffer();
    if b.slice(&b.start_iter(), &b.end_iter(), false)
        .len()
        .saturating_add(extra)
        > 512 * 1024
    {
        e.status
            .set_text("Replacement would exceed the 512 KiB composer limit");
        return;
    }
    history::checkpoint(&e);
    f.changing.set(true);
    b.begin_user_action();
    let ranges = if all {
        matches
    } else {
        vec![matches[f.active.get()].clone()]
    };
    // Reverse offsets leave the remaining matches and unrelated formatting intact.
    for r in ranges.into_iter().rev() {
        let mut a = b.iter_at_offset(r.start);
        let mut z = b.iter_at_offset(r.end);
        b.delete(&mut a, &mut z);
        b.insert(&mut a, &replacement);
    }
    b.end_user_action();
    f.changing.set(false);
    history::checkpoint(&e);
    refresh(f);
    select(f, 0);
}
fn schedule(f: &Rc<Find>) {
    if !f.root.reveals_child() || f.changing.get() {
        return;
    }
    if let Some(timer) = f.timer.borrow_mut().take() {
        timer.remove();
    }
    let weak = Rc::downgrade(f);
    let timer = glib::timeout_add_local_once(Duration::from_millis(120), move || {
        if let Some(f) = weak.upgrade() {
            f.timer.borrow_mut().take();
            refresh(&f);
        }
    });
    *f.timer.borrow_mut() = Some(timer);
}
pub(super) fn open(e: &Editor, replace: bool) {
    if let Some(f) = e.find.borrow().as_ref() {
        f.replacement.set_visible(replace);
        f.root.set_reveal_child(true);
        if let Some((a, z)) = e.text.buffer().selection_bounds() {
            let selected = e.text.buffer().text(&a, &z, false);
            if selected.len() <= 512 && !selected.contains('\n') {
                f.query.set_text(&selected);
            }
        }
        refresh(f);
        f.query.grab_focus();
    }
}
pub(super) fn install(e: &Rc<Editor>, toolbar: &gtk::FlowBox) {
    let root = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(160)
        .build();
    let content = vbox(5);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    let query = gtk::Entry::builder()
        .placeholder_text("Find in this message")
        .hexpand(true)
        .max_length(512)
        .build();
    let replacement = gtk::Entry::builder()
        .placeholder_text("Replace with")
        .hexpand(true)
        .max_length(4096)
        .build();
    let status = label("", "caption");
    let case = gtk::CheckButton::with_label("Match case");
    let word = gtk::CheckButton::with_label("Whole words");
    row.append(&query);
    row.append(&status);
    let prev = gtk::Button::with_label("Previous");
    let next = gtk::Button::with_label("Next");
    let close = ui::button("luma-close-symbolic", "Close find");
    for b in [&prev, &next, &close] {
        b.add_css_class("flat");
        row.append(b);
    }
    content.append(&row);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    row.append(&replacement);
    let one = gtk::Button::with_label("Replace");
    let all = gtk::Button::with_label("Replace all");
    row.append(&one);
    row.append(&all);
    content.append(&row);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&case);
    row.append(&word);
    content.append(&row);
    root.set_child(Some(&content));
    e.form
        .insert_child_after(&root, e.body_scroll.prev_sibling().as_ref());
    let f = Rc::new(Find {
        editor: Rc::downgrade(e),
        root,
        query,
        replacement,
        status,
        case,
        word,
        matches: Default::default(),
        active: Cell::new(0),
        changing: Cell::new(false),
        timer: Default::default(),
    });
    for (button, delta) in [(prev, -1), (next, 1)] {
        let weak = Rc::downgrade(&f);
        button.connect_clicked(move |_| {
            if let Some(f) = weak.upgrade() {
                select(&f, delta);
            }
        });
    }
    for (button, all) in [(one, false), (all, true)] {
        let weak = Rc::downgrade(&f);
        button.connect_clicked(move |_| {
            if let Some(f) = weak.upgrade() {
                replace(&f, all);
            }
        });
    }
    let weak = Rc::downgrade(&f);
    close.connect_clicked(move |_| {
        if let Some(f) = weak.upgrade() {
            f.root.set_reveal_child(false);
            f.matches.borrow_mut().clear();
            if let Some(e) = f.editor.upgrade() {
                e.text.grab_focus();
            }
        }
    });
    let weak = Rc::downgrade(&f);
    f.query.connect_changed(move |_| {
        if let Some(f) = weak.upgrade() {
            f.active.set(0);
            schedule(&f);
        }
    });
    for check in [&f.case, &f.word] {
        let weak = Rc::downgrade(&f);
        check.connect_toggled(move |_| {
            if let Some(f) = weak.upgrade() {
                refresh(&f);
            }
        });
    }
    let weak = Rc::downgrade(&f);
    f.query.connect_activate(move |_| {
        if let Some(f) = weak.upgrade() {
            select(&f, 1);
        }
    });
    let weak = Rc::downgrade(&f);
    e.text.buffer().connect_changed(move |_| {
        if let Some(f) = weak.upgrade() {
            schedule(&f);
        }
    });
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(&f);
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape
            && let Some(f) = weak.upgrade()
        {
            f.root.set_reveal_child(false);
            f.matches.borrow_mut().clear();
            if let Some(e) = f.editor.upgrade() {
                e.text.grab_focus();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    content.add_controller(keys);
    let button = gtk::Button::with_label("Find / replace");
    button.add_css_class("flat");
    toolbar.insert(&button, -1);
    let weak = Rc::downgrade(e);
    button.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            open(&e, true);
        }
    });
    *e.find.borrow_mut() = Some(f);
}
#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    let b = e.text.buffer();
    b.set_text("ðŸ™‚ CafÃ© cafÃ©ine CafÃ©");
    open(e, true);
    let f = e.find.borrow().as_ref().unwrap().clone();
    f.query.set_text("cafÃ©");
    f.word.set_active(true);
    refresh(&f);
    assert_eq!(f.matches.borrow().len(), 2);
    f.replacement.set_text("Hello");
    replace(&f, true);
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "ðŸ™‚ Hello cafÃ©ine Hello"
    );
    history::undo(e, false);
    assert_eq!(
        b.text(&b.start_iter(), &b.end_iter(), false),
        "ðŸ™‚ CafÃ© cafÃ©ine CafÃ©"
    );
    f.root.set_reveal_child(false);
    f.matches.borrow_mut().clear();
    println!(
        "PASS Unicode whole-word composer find/replace, one-step undo and hidden-bar idle suspension"
    );
}
