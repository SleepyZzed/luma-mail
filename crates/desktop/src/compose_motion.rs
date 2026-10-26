//! Finite, in-memory composer transitions. No disk copies or frame timer at rest.
use super::*;

pub(super) fn controls(e: &Editor) {
    let expanded = e.placement.get() == 0 || (e.placement.get() == 2 && e.detached_expanded.get());
    e.expand.set_icon_name(if expanded {
        "view-restore-symbolic"
    } else {
        "view-fullscreen-symbolic"
    });
    e.expand.set_tooltip_text(Some(if expanded {
        "Restore compact composer"
    } else {
        "Expand composer"
    }));
    e.popout.set_icon_name(if e.placement.get() == 2 {
        "view-restore-symbolic"
    } else {
        "window-new-symbolic"
    });
    e.popout.set_tooltip_text(Some(if e.placement.get() == 2 {
        "Move composer to main window"
    } else {
        "Pop out into a separate window"
    }));
}
pub(super) fn reset_layout(e: &Editor) {
    e.root.remove_css_class("inline-composer");
    e.root.remove_css_class("compact-composer");
    e.root.set_size_request(-1, -1);
    e.text.set_height_request(-1);
    e.root.set_halign(gtk::Align::Fill);
    e.root.set_valign(gtk::Align::Fill);
    e.root.set_hexpand(true);
    e.root.set_vexpand(true);
    ui::margins(&e.root, 0);
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
    picture: gtk::Picture,
    rect: gtk::graphene::Rect,
}
fn capture(e: &Editor) -> Option<Frame> {
    if e.placement.get() == 2
        || !e.root.is_mapped()
        || crate::presentation::duration(&e.state, 280) == 0
    {
        return None;
    }
    let rect = e.root.compute_bounds(&e.state.motion_layer)?;
    if rect.width() < 1. || rect.height() < 1. {
        return None;
    }
    let image = gtk::WidgetPaintable::new(Some(&e.root)).current_image();
    let picture = gtk::Picture::for_paintable(&image);
    picture.set_can_target(false);
    picture.set_can_focus(false);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Fill);
    picture.set_size_request(rect.width().round() as i32, rect.height().round() as i32);
    e.state.motion_layer.put(&picture, 0., 0.);
    position(&e.state.motion_layer, &picture, &rect, &rect);
    Some(Frame { picture, rect })
}
fn position(
    layer: &gtk::Fixed,
    picture: &gtk::Picture,
    source: &gtk::graphene::Rect,
    dest: &gtk::graphene::Rect,
) {
    let transform = gtk::gsk::Transform::new()
        .translate(&gtk::graphene::Point::new(dest.x(), dest.y()))
        .scale(
            dest.width() / source.width(),
            dest.height() / source.height(),
        );
    layer.set_child_transform(picture, Some(&transform));
}
fn animate(
    e: &Rc<Editor>,
    frame: Frame,
    dest: gtk::graphene::Rect,
    fade: bool,
    done: impl FnOnce() + 'static,
) {
    let layer = e.state.motion_layer.clone();
    let picture = frame.picture.clone();
    let source = frame.rect;
    let target = adw::CallbackAnimationTarget::new(move |value| {
        let v = value as f32;
        let rect = gtk::graphene::Rect::new(
            source.x() + (dest.x() - source.x()) * v,
            source.y() + (dest.y() - source.y()) * v,
            source.width() + (dest.width() - source.width()) * v,
            source.height() + (dest.height() - source.height()) * v,
        );
        position(&layer, &picture, &source, &rect);
        if fade {
            picture.set_opacity(1. - value);
        }
    });
    let animation = adw::TimedAnimation::new(
        &e.state.window,
        0.,
        1.,
        crate::presentation::duration(&e.state, 280),
        target,
    );
    animation.set_easing(adw::Easing::EaseOutCubic);
    let done = RefCell::new(Some(done));
    let editor = e.clone();
    animation.connect_done(move |_| {
        if frame.picture.parent().is_some() {
            editor.state.motion_layer.remove(&frame.picture);
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
pub(super) fn relocate(e: &Rc<Editor>, change: impl FnOnce()) {
    let frame = capture(e);
    if frame.is_some() {
        e.motion_busy.set(true);
        e.root.set_opacity(0.);
        e.root.set_can_target(false);
    }
    change();
    let Some(frame) = frame else {
        focus_message(e);
        return;
    };
    let editor = e.clone();
    let frame = RefCell::new(Some(frame));
    let ticks = Cell::new(0);
    e.state.window.add_tick_callback(move |_, _| {
        ticks.set(ticks.get() + 1);
        if ticks.get() < 3 {
            return glib::ControlFlow::Continue;
        }
        let frame = frame.borrow_mut().take().unwrap();
        let dest = editor
            .root
            .compute_bounds(&editor.state.motion_layer)
            .unwrap_or(frame.rect);
        let finished = editor.clone();
        animate(&editor, frame, dest, false, move || {
            if !finished.closing.get() {
                focus_message(&finished);
            }
        });
        glib::ControlFlow::Break
    });
}
pub(super) fn dismiss(e: &Rc<Editor>) {
    if e.motion_busy.get() {
        let editor = e.clone();
        glib::timeout_add_local_once(Duration::from_millis(320), move || dismiss(&editor));
        return;
    }
    e.root.set_can_target(false);
    if e.placement.get() == 0
        && e.state.compose_stack.visible_child_name().as_deref() == Some("compose")
    {
        // Keep the outgoing child alive until GTK has painted its return transition.
        e.motion_busy.set(true);
        e.state
            .compose_stack
            .set_visible_child_full("mail", gtk::StackTransitionType::SlideRight);
        if e.state.compose_stack.is_transition_running() {
            let editor = e.clone();
            let signal = Rc::new(RefCell::new(None));
            let disconnect = signal.clone();
            *signal.borrow_mut() = Some(e.state.compose_stack.connect_transition_running_notify(
                move |stack| {
                    if !stack.is_transition_running() {
                        if let Some(id) = disconnect.borrow_mut().take() {
                            stack.disconnect(id);
                        }
                        editor.motion_busy.set(false);
                        release_editor(&editor);
                    }
                },
            ));
            return;
        }
        e.motion_busy.set(false);
    } else if let Some(frame) = capture(e) {
        e.motion_busy.set(true);
        e.root.set_opacity(0.);
        let source = frame.rect;
        let dest = gtk::graphene::Rect::new(
            source.x() + source.width() * 0.015,
            source.y() + 30.,
            source.width() * 0.97,
            source.height() * 0.97,
        );
        let editor = e.clone();
        animate(e, frame, dest, true, move || release_editor(&editor));
        return;
    }
    release_editor(e);
}
