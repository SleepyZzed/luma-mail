//! Application preferences only: no desktop DPI changes or mail downloads.
use crate::ui::State;
use adw::prelude::*;
use std::rc::Rc;

pub const SCALES: [u16; 8] = [75, 85, 100, 110, 125, 150, 175, 200];
pub fn window_chrome(preference: bool) -> bool {
    cfg!(target_os = "windows") || (preference && !cfg!(target_os = "linux"))
}
pub fn configure_window(window: &impl IsA<gtk::Window>, preference: bool) {
    window.set_resizable(true);
    window.set_decorated(window_chrome(preference));
    if cfg!(target_os = "linux") {
        window.as_ref().add_css_class("luma-compositor-managed");
    }
}
pub fn scale(s: &State) -> f64 {
    s.settings.borrow().ui_scale_percent.clamp(75, 200) as f64 / 100.
}
pub fn duration(s: &State, millis: u32) -> u32 {
    if s.settings.borrow().reduced_motion
        || gtk::Settings::default().is_some_and(|p| !p.is_gtk_enable_animations())
    {
        0
    } else {
        #[cfg(debug_assertions)]
        if std::env::args().any(|a| a == "--motion-preview") {
            return millis.saturating_mul(6);
        }
        millis
    }
}
pub fn enter(s: &State, widget: &impl IsA<gtk::Widget>) {
    let time = duration(s, 280);
    if time == 0 || !widget.is_mapped() {
        widget.set_opacity(1.);
        return;
    }
    widget.set_opacity(0.35);
    let target = adw::PropertyAnimationTarget::new(widget.as_ref(), "opacity");
    let animation = adw::TimedAnimation::new(widget, 0.35, 1., time, target);
    animation.set_easing(adw::Easing::EaseOutCubic);
    animation.play();
}
fn stylesheet(percent: u16, reduced: bool) -> String {
    let percent = percent.clamp(75, 200);
    let factor = percent as f64 / 100.;
    let source = include_str!("../../../data/style.css");
    let mut out = format!("window {{ font-size: {percent}%; }}\n");
    // App-owned pixel dimensions scale with text. Keep hairline borders crisp.
    let bytes = source.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at].is_ascii_digit() && (at == 0 || !bytes[at - 1].is_ascii_alphanumeric()) {
            let begin = at;
            while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                at += 1;
            }
            if source[at..].starts_with("px") {
                let value = source[begin..at].parse::<f64>().unwrap_or(1.);
                out.push_str(&format!(
                    "{:.2}px",
                    if value > 1. { value * factor } else { value }
                ));
                at += 2;
            } else {
                out.push_str(&source[begin..at]);
            }
        } else {
            let ch = source[at..].chars().next().unwrap();
            out.push(ch);
            at += ch.len_utf8();
        }
    }
    if reduced {
        out.push_str("\nwindow .navigation-sidebar row.mail-row, window button, window button:hover, window button:active, window .row-star, window .attachment-card, window .luma-sidebar button, window .context-menu button, window .reader-toolbar button, window .compose-toolbar button { transition-duration: 0ms; }\n");
    }
    out
}
fn motion_tree(s: &State, widget: &gtk::Widget) {
    if let Some(reveal) = widget.downcast_ref::<gtk::Revealer>() {
        reveal.set_transition_duration(duration(s, 260));
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        motion_tree(s, &current);
    }
}
pub fn apply(s: &Rc<State>) {
    let prefs = s.settings.borrow();
    s.style_provider
        .load_from_string(&stylesheet(prefs.ui_scale_percent, prefs.reduced_motion));
    configure_window(&s.window, prefs.window_chrome);
    drop(prefs);
    s.compose_stack.set_transition_duration(duration(s, 240));
    motion_tree(s, s.window.upcast_ref());
    crate::compose::rescale(s);
    crate::rendering::refresh_appearance(s);
}
pub fn change_scale(s: &Rc<State>, direction: i32) {
    let current = s.settings.borrow().ui_scale_percent;
    let next = if direction > 0 {
        SCALES.iter().copied().find(|v| *v > current)
    } else {
        SCALES.iter().copied().rev().find(|v| *v < current)
    };
    if let Some(next) = next {
        s.settings.borrow_mut().ui_scale_percent = next;
        s.save();
        apply(s);
    }
}

#[cfg(debug_assertions)]
pub fn diagnostic(s: &Rc<State>) {
    assert!(s.clients.borrow().is_empty() && std::env::var_os("LUMA_TEST_ROOT").is_some());
    crate::ui::verify_selection(s);
    crate::ui::scroll_navigation_fixture(s);
    s.window.set_default_size(1000, 620);
    let mut ancestor = s.sidebar.parent();
    let scroll = loop {
        let current = ancestor.expect("navigation has a scroll viewport");
        if let Ok(scroll) = current.clone().downcast::<gtk::ScrolledWindow>() {
            break scroll;
        }
        ancestor = current.parent();
    };
    let footer = s.sidebar_top.parent().unwrap().last_child().unwrap();
    let state = s.clone();
    let mut stage = 0;
    let mut ticks = 0;
    let mut pinned = None::<(gtk::graphene::Rect, gtk::graphene::Rect)>;
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
        ticks += 1;
        assert!(ticks < 60, "pinned navigation diagnostics timed out");
        let header = state.sidebar_top.compute_bounds(&state.window).unwrap();
        let bottom = footer.compute_bounds(&state.window).unwrap();
        let adjustment = scroll.vadjustment();
        match stage {
            0 | 3 => {
                if adjustment.upper() - adjustment.page_size() < 20. {
                    return gtk::glib::ControlFlow::Continue;
                }
                pinned = Some((header, bottom));
                adjustment.set_value(if stage == 0 {
                    adjustment.upper() - adjustment.page_size()
                } else {
                    0.
                });
                stage += 1;
            }
            1 | 4 => {
                let (before, old_footer) = pinned.unwrap();
                assert!(
                    (header.y() - before.y()).abs() < 1.
                        && (header.height() - before.height()).abs() < 1.,
                    "Luma/Compose must stay fixed while navigation scrolls"
                );
                assert!(
                    (bottom.y() - old_footer.y()).abs() < 1.,
                    "sidebar footer must stay fixed"
                );
                assert!(state.sidebar_top.is_mapped());
                if stage == 1 {
                    assert!(adjustment.value() > 1.);
                    state.settings.borrow_mut().ui_scale_percent = 150;
                    apply(&state);
                    stage = 3;
                } else {
                    assert!(adjustment.value() < 1.);
                    println!(
                        "PASS fixed Luma/Compose and Settings/menu with independently scrolling folders/accounts at 100% and 150%"
                    );
                    crate::ui::verify_navigation(&state);
                    return gtk::glib::ControlFlow::Break;
                }
            }
            _ => (),
        }
        gtk::glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scale_changes_dimensions_but_keeps_hairlines_and_respects_reduced_motion() {
        let css = stylesheet(150, true);
        assert!(css.contains("font-size: 150%"));
        assert!(css.contains("border-radius: 21.00px"));
        assert!(css.contains("1.00px solid"));
        assert!(css.contains("transition-duration: 0ms"));
    }
}
