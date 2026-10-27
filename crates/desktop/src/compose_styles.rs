//! Native rich formatting, including typing state. No rendering process or disk cache.
use super::*;
#[derive(Default)]
pub(super) struct Styles {
    pub typing: RefCell<Vec<String>>,
    sync: Cell<bool>,
    toggles: RefCell<Vec<(String, gtk::ToggleButton)>>,
    paragraphs: RefCell<Option<(gtk::DropDown, gtk::DropDown)>>,
}
fn kind(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(kind, _)| kind)
}
fn inline(name: &str) -> bool {
    matches!(name, "strong" | "em" | "u" | "s") || name.starts_with("style:")
}
pub(super) fn tag(e: &Editor, name: &str) -> Option<gtk::TextTag> {
    luma_core::rich_style::properties(name)?;
    let b = e.text.buffer();
    if let Some(t) = b.tag_table().lookup(name) {
        return Some(t);
    }
    if b.tag_table().size() >= 512 {
        return None;
    }
    let t = gtk::TextTag::new(Some(name));
    let (key, value, _) = luma_core::rich_style::properties(name)?;
    match key {
        "data-luma-color" => t.set_foreground(Some(&value)),
        "data-luma-highlight" => t.set_background(Some(&value)),
        "data-luma-code" => t.set_family(Some("Monospace")),
        "data-luma-script" => {
            t.set_scale(0.8);
            t.set_rise(
                (if value == "super" { 5. } else { -3. }
                    * gtk::pango::SCALE as f64
                    * crate::presentation::scale(&e.state)) as i32,
            );
        }
        "data-luma-align" => t.set_justification(match value.as_str() {
            "center" => gtk::Justification::Center,
            "right" => gtk::Justification::Right,
            "justify" => gtk::Justification::Fill,
            _ => gtk::Justification::Left,
        }),
        "data-luma-indent" => t.set_left_margin(
            (value.parse::<f64>().ok()? * 24. * crate::presentation::scale(&e.state)) as i32,
        ),
        "data-luma-spacing" => t.set_line_height(value.parse::<f32>().ok()? / 100.),
        _ => return None,
    }
    b.tag_table().add(&t);
    Some(t)
}
pub(super) fn resolve(e: &Editor, name: &str) -> Option<gtk::TextTag> {
    let b = e.text.buffer();
    if let Some(f) = name.strip_prefix("font:family:") {
        Some(fonts::family_tag(&b, f))
    } else if let Some(n) = name.strip_prefix("font:size:").and_then(|s| s.parse().ok()) {
        Some(fonts::size_tag(e, n))
    } else if name.starts_with("style:") || name.starts_with("paragraph:") {
        tag(e, name)
    } else if let Some(url) = name.strip_prefix("link:") {
        Some(link_tag(&b, url))
    } else {
        b.tag_table().lookup(name)
    }
}
fn selection(e: &Editor, paragraph: bool) -> (gtk::TextIter, gtk::TextIter) {
    let b = e.text.buffer();
    let (mut a, mut z) = b.selection_bounds().unwrap_or_else(|| {
        let p = b.iter_at_offset(b.cursor_position());
        (p, p)
    });
    if paragraph {
        a.set_line_offset(0);
        if z.offset() > a.offset() && z.line_offset() == 0 {
            z.backward_char();
        }
        z.forward_to_line_end();
        if !z.is_end() {
            z.forward_char();
        }
    }
    (a, z)
}
fn remove_kind(e: &Editor, key: &str, a: &gtk::TextIter, z: &gtk::TextIter) {
    let b = e.text.buffer();
    b.tag_table().foreach(|t| {
        if t.name().is_some_and(|name| kind(&name) == key) {
            b.remove_tag(t, a, z);
        }
    });
}
pub(super) fn set(e: &Rc<Editor>, name: &str) {
    if e.html.is_active() {
        e.status
            .set_text("Switch off HTML source to use visual formatting");
        return;
    }
    if !inline(name) && !name.starts_with("paragraph:") {
        return;
    }
    history::checkpoint(e);
    let paragraph = name.starts_with("paragraph:");
    let (a, z) = selection(e, paragraph);
    if a.offset() != z.offset() {
        remove_kind(e, kind(name), &a, &z);
        if let Some(t) = resolve(e, name) {
            e.text.buffer().apply_tag(&t, &a, &z);
        } else {
            e.status.set_text("This draft has too many styles");
            return;
        }
        changed(e);
    }
    {
        let mut typing = e.styling.typing.borrow_mut();
        typing.retain(|n| kind(n) != kind(name));
        typing.push(name.into());
    }
    history::checkpoint(e);
    sync(e);
    e.text.grab_focus();
}
pub(super) fn toggle(e: &Rc<Editor>, name: &str) {
    if e.html.is_active() {
        e.status
            .set_text("Switch off HTML source to use visual formatting");
        return;
    }
    let b = e.text.buffer();
    let (a, z) = selection(e, false);
    let active = if a.offset() == z.offset() {
        e.styling.typing.borrow().iter().any(|n| n == name)
    } else {
        let t = resolve(e, name);
        let mut i = a;
        let mut all = true;
        while i.offset() < z.offset() {
            if !t.as_ref().is_some_and(|t| i.has_tag(t)) {
                all = false;
                break;
            }
            i.forward_char();
        }
        all
    };
    if active {
        history::checkpoint(e);
        if let Some(t) = resolve(e, name) {
            b.remove_tag(&t, &a, &z);
            if a.offset() != z.offset() {
                changed(e);
            }
        }
        e.styling.typing.borrow_mut().retain(|n| n != name);
        history::checkpoint(e);
        sync(e);
        e.text.grab_focus();
    } else {
        set(e, name);
    }
}
fn clear_kind(e: &Rc<Editor>, key: &str) {
    history::checkpoint(e);
    let (a, z) = selection(e, key.starts_with("paragraph:"));
    remove_kind(e, key, &a, &z);
    e.styling.typing.borrow_mut().retain(|n| kind(n) != key);
    if a.offset() != z.offset() {
        changed(e);
    }
    history::checkpoint(e);
    sync(e);
    e.text.grab_focus();
}
fn clear(e: &Rc<Editor>) {
    if e.html.is_active() {
        return;
    }
    history::checkpoint(e);
    let (a, z) = selection(e, true);
    e.text.buffer().remove_all_tags(&a, &z);
    e.styling.typing.borrow_mut().clear();
    *e.typing_font.borrow_mut() = (None, None);
    changed(e);
    history::checkpoint(e);
    sync(e);
    fonts::sync_controls(e);
    highlight_links(e);
    e.text.grab_focus();
}
fn colour(toolbar: &gtk::FlowBox, e: &Rc<Editor>, highlight: bool) {
    let button = gtk::MenuButton::builder()
        .label(if highlight {
            "Highlight"
        } else {
            "Text colour"
        })
        .tooltip_text(if highlight {
            "Highlight colour"
        } else {
            "Text colour"
        })
        .build();
    button.add_css_class("flat");
    let pop = gtk::Popover::new();
    let body = vbox(8);
    ui::margins(&body, 10);
    let grid = gtk::Grid::builder()
        .row_spacing(5)
        .column_spacing(5)
        .build();
    let palette = if highlight {
        [
            "#fef08a", "#bbf7d0", "#bfdbfe", "#fecaca", "#e9d5ff", "#fed7aa", "#fbcfe8", "#e2e8f0",
            "#ffffff", "#93c5fd", "#d9f99d", "#fca5a5",
        ]
    } else {
        [
            "#111111", "#ffffff", "#d92d20", "#f79009", "#ca8504", "#039855", "#1570ef", "#6938ef",
            "#c11574", "#667085", "#344054", "#b54708",
        ]
    };
    for (i, value) in palette.into_iter().enumerate() {
        let b = gtk::Button::new();
        b.set_tooltip_text(Some(value));
        let swatch = gtk::DrawingArea::builder()
            .content_width(24)
            .content_height(24)
            .build();
        let c = gtk::gdk::RGBA::parse(value).unwrap();
        swatch.set_draw_func(move |_, cr, w, h| {
            cr.set_source_rgb(c.red() as f64, c.green() as f64, c.blue() as f64);
            cr.rectangle(0., 0., w as f64, h as f64);
            let _ = cr.fill();
        });
        b.set_child(Some(&swatch));
        grid.attach(&b, (i % 6) as i32, (i / 6) as i32, 1, 1);
        let weak = Rc::downgrade(e);
        let pop = pop.downgrade();
        b.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                set(
                    &e,
                    &format!(
                        "style:{}:{value}",
                        if highlight { "highlight" } else { "color" }
                    ),
                );
            }
            if let Some(pop) = pop.upgrade() {
                pop.popdown();
            }
        });
    }
    body.append(&grid);
    let automatic = gtk::Button::with_label(if highlight {
        "No highlight"
    } else {
        "Automatic text colour"
    });
    body.append(&automatic);
    let weak = Rc::downgrade(e);
    let wp = pop.downgrade();
    automatic.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            clear_kind(
                &e,
                if highlight {
                    "style:highlight"
                } else {
                    "style:color"
                },
            );
        }
        if let Some(p) = wp.upgrade() {
            p.popdown();
        }
    });
    let custom = gtk::Button::with_label("Custom colour…");
    body.append(&custom);
    let weak = Rc::downgrade(e);
    let wp = pop.downgrade();
    custom.connect_clicked(move |_| {
        let Some(e) = weak.upgrade() else {
            return;
        };
        if let Some(p) = wp.upgrade() {
            p.popdown();
        }
        glib::MainContext::default().spawn_local(async move {
            let d = gtk::ColorDialog::new();
            d.set_with_alpha(false);
            if let Ok(c) = d.choose_rgba_future(Some(&e.parent()), None).await {
                let hex = format!(
                    "#{:02x}{:02x}{:02x}",
                    (c.red() * 255.).round() as u8,
                    (c.green() * 255.).round() as u8,
                    (c.blue() * 255.).round() as u8
                );
                set(
                    &e,
                    &format!(
                        "style:{}:{hex}",
                        if highlight { "highlight" } else { "color" }
                    ),
                );
            }
        });
    });
    pop.set_child(Some(&body));
    button.set_popover(Some(&pop));
    toolbar.insert(&button, -1);
}
pub(super) fn sync(e: &Editor) {
    e.styling.sync.set(true);
    let typing = e.styling.typing.borrow();
    for (name, b) in e.styling.toggles.borrow().iter() {
        b.set_active(typing.contains(name));
    }
    if let Some((align, spacing)) = e.styling.paragraphs.borrow().as_ref() {
        let b = e.text.buffer();
        let i = b.iter_at_offset(b.cursor_position());
        let mut names: Vec<_> = i
            .tags()
            .iter()
            .filter_map(|t| t.name().map(|n| n.to_string()))
            .collect();
        for name in typing.iter().filter(|n| n.starts_with("paragraph:")) {
            if !names.iter().any(|n| kind(n) == kind(name)) {
                names.push(name.clone());
            }
        }
        align.set_selected(
            ["left", "center", "right", "justify"]
                .iter()
                .position(|v| names.contains(&format!("paragraph:align:{v}")))
                .unwrap_or(0) as u32,
        );
        spacing.set_selected(
            [100, 115, 150, 200]
                .iter()
                .position(|v| names.contains(&format!("paragraph:spacing:{v}")))
                .unwrap_or(0) as u32,
        );
    }
    e.styling.sync.set(false);
}
pub(super) fn install(e: &Rc<Editor>, toolbar: &gtk::FlowBox) {
    colour(toolbar, e, false);
    colour(toolbar, e, true);
    let menu = gtk::MenuButton::builder()
        .icon_name("luma-more-symbolic")
        .tooltip_text("Paragraph and more formatting")
        .build();
    menu.add_css_class("flat");
    let pop = gtk::Popover::new();
    let body = vbox(8);
    ui::margins(&body, 12);
    body.append(&label("Paragraph alignment", "caption"));
    let align = gtk::DropDown::from_strings(&["Left", "Centre", "Right", "Justify"]);
    body.append(&align);
    body.append(&label("Line spacing", "caption"));
    let spacing = gtk::DropDown::from_strings(&["Single", "1.15", "1.5", "Double"]);
    body.append(&spacing);
    let weak = Rc::downgrade(e);
    align.connect_selected_notify(move |v| {
        if let Some(e) = weak.upgrade()
            && !e.styling.sync.get()
        {
            set(
                &e,
                &format!(
                    "paragraph:align:{}",
                    ["left", "center", "right", "justify"][v.selected() as usize]
                ),
            );
        }
    });
    let weak = Rc::downgrade(e);
    spacing.connect_selected_notify(move |v| {
        if let Some(e) = weak.upgrade()
            && !e.styling.sync.get()
        {
            set(
                &e,
                &format!(
                    "paragraph:spacing:{}",
                    [100, 115, 150, 200][v.selected() as usize]
                ),
            );
        }
    });
    *e.styling.paragraphs.borrow_mut() = Some((align, spacing));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    for (title, delta) in [("Decrease indent", -1), ("Increase indent", 1)] {
        let b = gtk::Button::with_label(title);
        row.append(&b);
        let weak = Rc::downgrade(e);
        b.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                let b = e.text.buffer();
                let i = b.iter_at_offset(b.cursor_position());
                let old = i
                    .tags()
                    .iter()
                    .filter_map(|t| t.name())
                    .find_map(|n| {
                        n.strip_prefix("paragraph:indent:")
                            .and_then(|s| s.parse::<i32>().ok())
                    })
                    .unwrap_or(0);
                set(
                    &e,
                    &format!("paragraph:indent:{}", (old + delta).clamp(0, 8)),
                );
            }
        });
    }
    body.append(&row);
    for (title, name) in [
        ("Superscript", "style:script:super"),
        ("Subscript", "style:script:sub"),
        ("Monospace code", "style:code:1"),
    ] {
        let b = gtk::ToggleButton::with_label(title);
        body.append(&b);
        e.styling
            .toggles
            .borrow_mut()
            .push((name.into(), b.clone()));
        let weak = Rc::downgrade(e);
        b.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade()
                && !e.styling.sync.get()
            {
                toggle(&e, name);
            }
        });
    }
    let clear_button = gtk::Button::with_label("Clear formatting");
    body.append(&clear_button);
    let weak = Rc::downgrade(e);
    clear_button.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            clear(&e);
        }
    });
    pop.set_child(Some(&body));
    menu.set_popover(Some(&pop));
    toolbar.insert(&menu, -1);
    let weak = Rc::downgrade(e);
    e.text
        .buffer()
        .connect_local("insert-text", true, move |values| {
            if let Some(e) = weak.upgrade()
                && !e.html.is_active()
                && !e.history.restoring.get()
            {
                let z = values[1].get::<gtk::TextIter>().unwrap();
                let input = values[2].get::<String>().unwrap();
                let b = e.text.buffer();
                let a = b.iter_at_offset(z.offset() - input.chars().count() as i32);
                let names = e.styling.typing.borrow().clone();
                b.tag_table().foreach(|tag| {
                    if tag
                        .name()
                        .is_some_and(|n| inline(&n) || n.starts_with("paragraph:"))
                    {
                        b.remove_tag(tag, &a, &z);
                    }
                });
                for name in names {
                    if let Some(t) = resolve(&e, &name) {
                        b.apply_tag(&t, &a, &z);
                    }
                }
            }
            None
        });
    let weak = Rc::downgrade(e);
    e.text.buffer().connect_mark_set(move |b, at, mark| {
        if let Some(e) = weak.upgrade()
            && mark.name().as_deref() == Some("insert")
            && !e.font_inserting.get()
            && !e.history.restoring.get()
        {
            let i = b.selection_bounds().map(|(a, _)| a).unwrap_or_else(|| {
                let mut i = *at;
                if i.is_end() {
                    i.backward_char();
                }
                i
            });
            *e.styling.typing.borrow_mut() = i
                .tags()
                .iter()
                .filter_map(|t| t.name().map(|s| s.to_string()))
                .filter(|n| inline(n) || n.starts_with("paragraph:"))
                .collect();
            sync(&e);
        }
    });
}
pub(super) fn rescale(e: &Editor) {
    let scale = crate::presentation::scale(&e.state);
    e.text.buffer().tag_table().foreach(|t| {
        if let Some(n) = t.name() {
            if let Some(v) = n
                .strip_prefix("paragraph:indent:")
                .and_then(|n| n.parse::<f64>().ok())
            {
                t.set_left_margin((v * 24. * scale) as i32);
            }
            if let Some(v) = n.strip_prefix("style:script:") {
                t.set_rise(
                    ((if v == "super" { 5. } else { -3. }) * gtk::pango::SCALE as f64 * scale)
                        as i32,
                );
            }
        }
    });
}
#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    let b = e.text.buffer();
    b.set_text("Colour and paragraph");
    b.select_range(&b.start_iter(), &b.end_iter());
    set(e, "style:color:#d92d20");
    set(e, "style:highlight:#fef08a");
    set(e, "paragraph:align:center");
    set(e, "paragraph:spacing:150");
    set(e, "paragraph:indent:2");
    let html = formatted_html(&b);
    assert!(
        html.contains("color:#d92d20")
            && html.contains("background-color:#fef08a")
            && html.contains("text-align:center")
            && html.contains("line-height:1.5")
            && html.contains("padding-left:48px")
    );
    let r = luma_core::rich_draft::parse(&html).expect("rich styles restore visually");
    assert_eq!(r.text, "Colour and paragraph");
    assert!(r.spans.iter().any(|s| s.tag == "style:color:#d92d20"));
    b.place_cursor(&b.end_iter());
    b.insert_at_cursor(" next");
    assert!(formatted_html(&b).contains("#d92d20"));
    let before = formatted_html(&b);
    e.state.settings.borrow_mut().ui_scale_percent = 150;
    crate::presentation::apply(&e.state);
    assert_eq!(formatted_html(&b), before);
    e.state.settings.borrow_mut().ui_scale_percent = 100;
    crate::presentation::apply(&e.state);
    b.set_text("");
    set(e, "paragraph:align:center");
    set(e, "paragraph:spacing:150");
    b.insert_at_cursor("Centred from the first character");
    let html = formatted_html(&b);
    assert!(html.contains("text-align:center") && html.contains("line-height:1.5"));
    println!(
        "PASS native composer colours/highlights, paragraph alignment/spacing/indent, further typing, rich draft restoration and scale-independent MIME"
    );
}
