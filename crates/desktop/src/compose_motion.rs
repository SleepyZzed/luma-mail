//! Finite, in-memory composer transitions. No disk copies or frame timer at rest.
use super::*;

pub(super) fn controls(e: &Editor) {
    let expanded = e.placement.get() == 0 || (e.placement.get() == 2 && e.detached_expanded.get());
    e.expand.set_icon_name(if expanded {
        "luma-restore-symbolic"
    } else {
        "luma-expand-symbolic"
    });
    e.expand.set_tooltip_text(Some(if expanded {
        "Restore compact composer"
    } else {
        "Expand composer"
    }));
    e.popout.set_icon_name(if e.placement.get() == 2 {
        "luma-restore-symbolic"
    } else {
        "luma-popout-symbolic"
    });
    e.popout.set_tooltip_text(Some(if e.placement.get() == 2 {
        "Move composer to main window"
    } else {
        "Pop out into a separate window"
    }));
}
pub(super) fn reset_layout(e: &Editor) {
    e.header.set_visible(true);
    e.inline_header.set_visible(false);
    e.from_row.set_visible(true);
    e.subject.set_visible(true);
    e.field_separator.set_visible(true);
    e.form.set_vexpand(true);
    e.send_reveal.set_vexpand(true);
    e.body_scroll.set_max_content_height(-1);
    e.body_scroll.set_propagate_natural_height(false);
    e.body_scroll.set_min_content_height(240);
    e.body_scroll.set_vexpand(true);
    e.root.remove_css_class("inline-composer");
    e.root.remove_css_class("compact-composer");
    e.root.set_size_request(-1, -1);
    e.text.set_height_request(-1);
    e.root.set_halign(gtk::Align::Fill);
    e.root.set_valign(gtk::Align::Fill);
    e.root.set_hexpand(true);
    e.root.set_vexpand(true);
    e.surface.set_vexpand(true);
    ui::margins(&e.root, 0);
}
pub(super) fn inline(e: &Editor) {
    reset_layout(e);
    e.root.add_css_class("inline-composer");
    e.root.set_vexpand(false);
    e.surface.set_vexpand(false);
    e.header.set_visible(false);
    e.inline_header.set_visible(true);
    e.from_row.set_visible(false);
    e.subject.set_visible(false);
    e.field_separator.set_visible(false);
    e.form.set_vexpand(false);
    e.send_reveal.set_vexpand(false);
    e.body_scroll.set_vexpand(false);
    e.body_scroll.set_min_content_height(120);
    e.body_scroll.set_max_content_height(320);
    e.body_scroll.set_propagate_natural_height(true);
}
pub(super) fn resize(e: &Editor, width: i32, height: i32) {
    if e.placement.get() != 3 {
        return;
    }
    let scale = crate::presentation::scale(&e.state);
    let margin = (20. * scale).round() as i32;
    ui::margins(&e.root, margin);
    e.root.set_size_request(
        (width - margin * 2).max(1).min((760. * scale) as i32),
        (height - margin * 2).max(1).min((600. * scale) as i32),
    );
}
pub(super) fn reveal_inline(e: &Rc<Editor>) {
    if e.placement.get() != 1 {
        return;
    }
    let weak = Rc::downgrade(e);
    let ticks = Cell::new(0);
    e.root.add_tick_callback(move |_, _| {
        ticks.set(ticks.get() + 1);
        if ticks.get() < 3 {
            return glib::ControlFlow::Continue;
        }
        let Some(e) = weak
            .upgrade()
            .filter(|e| !e.closing.get() && e.placement.get() == 1)
        else {
            return glib::ControlFlow::Break;
        };
        if let Some(bounds) = e.root.compute_bounds(&e.state.reader) {
            let mut parent = e.state.reader.parent();
            while let Some(widget) = parent {
                if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
                    let adjustment = scroll.vadjustment();
                    let bottom = bounds.y() as f64
                        + bounds.height() as f64
                        + e.state.reader.margin_top() as f64
                        + 16.;
                    let target = (bottom - adjustment.page_size())
                        .max(0.)
                        .min((adjustment.upper() - adjustment.page_size()).max(0.));
                    adjustment.set_value(target);
                    break;
                }
                parent = widget.parent();
            }
        }
        glib::ControlFlow::Break
    });
}
pub(super) fn compact(e: &Rc<Editor>) {
    detach(e);
    e.placement.set(3);
    reset_layout(e);
    e.root.add_css_class("compact-composer");
    e.root.set_halign(gtk::Align::End);
    e.root.set_valign(gtk::Align::End);
    e.root.set_hexpand(false);
    e.root.set_vexpand(false);
    resize(e, e.state.window.width(), e.state.window.height());
    e.state
        .compose_stack
        .set_visible_child_full("mail", gtk::StackTransitionType::None);
    e.state.compose_overlay.add_overlay(&e.root);
    *e.state.editor.borrow_mut() = Some(e.clone());
}

struct Frame {
    image: gtk::gdk::Paintable,
    rect: gtk::graphene::Rect,
}
fn rect(e: &Editor) -> Option<gtk::graphene::Rect> {
    e.root
        .compute_bounds(&e.state.motion_layer)
        .filter(|r| r.width() > 1. && r.height() > 1.)
}
fn capture(e: &Editor) -> Option<Frame> {
    if !e.root.is_mapped() {
        return None;
    }
    let rect = rect(e)?;
    // Snapshot the final allocation at its native size. Never stretch a compact
    // screenshot into a full-page editor: only the clip rectangle interpolates.
    let snapshot = gtk::Snapshot::new();
    let inner = e.surface.compute_bounds(&e.root)?;
    snapshot.translate(&gtk::graphene::Point::new(inner.x(), inner.y()));
    gtk::WidgetPaintable::new(Some(&e.surface)).snapshot(
        &snapshot,
        inner.width() as f64,
        inner.height() as f64,
    );
    // Reject an unpainted frame instead of animating an empty paintable.
    let node = snapshot.to_node()?;
    let snapshot = gtk::Snapshot::new();
    snapshot.append_node(&node);
    let image = snapshot.to_paintable(Some(&gtk::graphene::Size::new(rect.width(), rect.height())));
    image.map(|image| Frame { image, rect })
}
fn interpolate(a: gtk::graphene::Rect, b: gtk::graphene::Rect, v: f32) -> gtk::graphene::Rect {
    gtk::graphene::Rect::new(
        a.x() + (b.x() - a.x()) * v,
        a.y() + (b.y() - a.y()) * v,
        a.width() + (b.width() - a.width()) * v,
        a.height() + (b.height() - a.height()) * v,
    )
}
#[allow(deprecated)]
fn animate(
    e: &Rc<Editor>,
    frame: Frame,
    from: gtk::graphene::Rect,
    to: gtk::graphene::Rect,
    done: impl FnOnce() + 'static,
) {
    let layer = &e.state.motion_layer;
    let canvas =
        gtk::graphene::Size::new(layer.width().max(1) as f32, layer.height().max(1) as f32);
    #[cfg(debug_assertions)]
    if std::env::args().any(|a| a == "--motion-preview") {
        eprintln!(
            "motion canvas {}x{} frame {:?} surface {}x{}",
            canvas.width(),
            canvas.height(),
            frame.rect,
            e.surface.width(),
            e.surface.height()
        );
    }
    let picture = gtk::Picture::new();
    picture.set_can_target(false);
    picture.set_can_focus(false);
    picture.set_size_request(canvas.width() as i32, canvas.height() as i32);
    picture.set_content_fit(gtk::ContentFit::Fill);
    layer.put(&picture, 0., 0.);
    // Keep motion above an overlay composer regardless of child insertion order.
    if let Some(last) = e.state.compose_overlay.last_child().filter(|w| w != layer) {
        layer.insert_after(&e.state.compose_overlay, Some(&last));
    }
    let background = e
        .root
        .style_context()
        .lookup_color("window_bg_color")
        .unwrap_or_else(|| gtk::gdk::RGBA::new(0.12, 0.12, 0.12, 1.));
    let output = picture.clone();
    let radius = (16. * crate::presentation::scale(&e.state)) as f32;
    let draw = move |value: f64| {
        let clip = interpolate(from, to, value as f32);
        let local = gtk::graphene::Rect::new(0., 0., clip.width(), clip.height());
        let snapshot = gtk::Snapshot::new();
        snapshot.translate(&gtk::graphene::Point::new(clip.x(), clip.y()));
        snapshot.push_rounded_clip(&gtk::gsk::RoundedRect::from_rect(local, radius));
        snapshot.append_color(&background, &local);
        // Constant image dimensions preserve glyphs, icons and border thickness.
        frame.image.snapshot(
            &snapshot,
            frame.rect.width() as f64,
            frame.rect.height() as f64,
        );
        snapshot.pop();
        output.set_paintable(snapshot.to_paintable(Some(&canvas)).as_ref());
    };
    draw(0.);
    let animation = adw::TimedAnimation::new(
        &e.state.window,
        0.,
        1.,
        crate::presentation::duration(&e.state, 280),
        adw::CallbackAnimationTarget::new(draw),
    );
    animation.set_easing(adw::Easing::EaseOutCubic);
    let done = RefCell::new(Some(done));
    let editor = e.clone();
    animation.connect_done(move |_| {
        if picture.parent().is_some() {
            editor.state.motion_layer.remove(&picture);
        }
        editor.root.set_opacity(1.);
        editor.root.set_can_target(true);
        editor.motion_busy.set(false);
        if let Some(done) = done.borrow_mut().take() {
            done();
        }
    });
    animation.play();
}
fn ready(e: &Rc<Editor>, done: impl FnOnce(&Rc<Editor>, Option<Frame>) + 'static) {
    let editor = e.clone();
    let last = Cell::new(None::<gtk::graphene::Rect>);
    let stable = Cell::new(0);
    let ticks = Cell::new(0);
    let done = Rc::new(RefCell::new(Some(done)));
    let fallback = done.clone();
    let callback = Rc::new(RefCell::new(None::<gtk::TickCallbackId>));
    let completed = callback.clone();
    *callback.borrow_mut() = Some(e.state.window.add_tick_callback(move |_, _| {
        ticks.set(ticks.get() + 1);
        let current = rect(&editor);
        if let (Some(a), Some(b)) = (last.get(), current) {
            stable.set(
                if (a.x() - b.x()).abs() < 0.5
                    && (a.y() - b.y()).abs() < 0.5
                    && (a.width() - b.width()).abs() < 0.5
                    && (a.height() - b.height()).abs() < 0.5
                {
                    stable.get() + 1
                } else {
                    0
                },
            );
        }
        last.set(current);
        if (stable.get() >= 2 && editor.root.is_mapped()) || ticks.get() >= 20 {
            let frame = capture(&editor);
            completed.borrow_mut().take();
            done.borrow_mut().take().unwrap()(&editor, frame);
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    }));
    let weak = Rc::downgrade(e);
    glib::timeout_add_local_once(Duration::from_millis(350), move || {
        if let Some(done) = fallback.borrow_mut().take() {
            if let Some(id) = callback.borrow_mut().take() {
                id.remove();
            }
            if let Some(editor) = weak.upgrade() {
                done(&editor, None);
            }
        }
    });
}
fn show_without_motion(e: &Editor) {
    e.root.set_opacity(1.);
    e.root.set_can_target(true);
    e.motion_busy.set(false);
    focus_message(e);
}
pub(super) fn enter(e: &Rc<Editor>) {
    if e.placement.get() == 2 || crate::presentation::duration(&e.state, 280) == 0 {
        show_without_motion(e);
        return;
    }
    e.motion_busy.set(true);
    // One alpha step keeps GTK painting the opaque inner surface for capture.
    e.root.set_opacity(1. / 255.);
    e.root.set_can_target(false);
    ready(e, |e, frame| {
        if let Some(frame) = frame {
            let dest = frame.rect;
            let source = gtk::graphene::Rect::new(
                dest.x(),
                e.state.motion_layer.height() as f32 + 24.,
                dest.width(),
                dest.height(),
            );
            let finished = e.clone();
            animate(e, frame, source, dest, move || focus_message(&finished));
        } else {
            show_without_motion(e);
        }
    });
}
pub(super) fn relocate(e: &Rc<Editor>, change: impl FnOnce()) {
    let from = rect(e);
    let enabled = e.placement.get() != 2
        && crate::presentation::duration(&e.state, 280) > 0
        && from.is_some();
    let cover = if enabled {
        capture(e).map(|frame| {
            let picture = gtk::Picture::for_paintable(&frame.image);
            picture.set_can_target(false);
            picture.set_size_request(frame.rect.width() as i32, frame.rect.height() as i32);
            e.state
                .motion_layer
                .put(&picture, frame.rect.x() as f64, frame.rect.y() as f64);
            picture
        })
    } else {
        None
    };
    if enabled {
        e.motion_busy.set(true);
        e.root.set_opacity(1. / 255.);
        e.root.set_can_target(false);
    }
    change();
    if !enabled {
        show_without_motion(e);
        return;
    }
    ready(e, move |e, frame| {
        if let Some(cover) = cover {
            e.state.motion_layer.remove(&cover);
        }
        if let Some(frame) = frame {
            let dest = frame.rect;
            let finished = e.clone();
            animate(e, frame, from.unwrap(), dest, move || {
                if !finished.closing.get() {
                    focus_message(&finished);
                    reveal_inline(&finished);
                }
            });
        } else {
            show_without_motion(e);
        }
    });
}
pub(super) fn dismiss(e: &Rc<Editor>) {
    if e.motion_busy.get() {
        let editor = e.clone();
        glib::timeout_add_local_once(Duration::from_millis(80), move || dismiss(&editor));
        return;
    }
    e.root.set_can_target(false);
    let frame = (e.placement.get() != 2 && crate::presentation::duration(&e.state, 260) > 0)
        .then(|| capture(e))
        .flatten();
    if let Some(frame) = frame {
        let source = frame.rect;
        e.motion_busy.set(true);
        e.root.set_opacity(0.);
        if e.placement.get() == 0 {
            e.state
                .compose_stack
                .set_visible_child_full("mail", gtk::StackTransitionType::None);
        }
        let dest = gtk::graphene::Rect::new(
            source.x(),
            e.state.motion_layer.height() as f32 + 24.,
            source.width(),
            source.height(),
        );
        let editor = e.clone();
        animate(e, frame, source, dest, move || release_editor(&editor));
    } else {
        release_editor(e);
    }
}
