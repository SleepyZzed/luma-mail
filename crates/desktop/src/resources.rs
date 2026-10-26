//! Resource reclamation at lifecycle boundaries, never a busy idle loop.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
thread_local! {
    static TRIM: std::cell::RefCell<Option<gtk::glib::SourceId>> = const { std::cell::RefCell::new(None) };
}
pub fn trim() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    TRIM.with(|pending| {
        if let Some(old) = pending.borrow_mut().take() {
            old.remove();
        }
        let source = gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(2), || {
            TRIM.with(|pending| {
                pending.borrow_mut().take();
            });
            // Return already-free allocator arenas after views/buffers are dropped.
            // This neither frees live memory nor imposes an allocation/renderer cap.
            unsafe {
                libc::malloc_trim(0);
            }
        });
        *pending.borrow_mut() = Some(source);
    });
}

#[cfg(target_os = "linux")]
pub fn diagnostic(s: &std::rc::Rc<crate::ui::State>) {
    use adw::prelude::*;
    assert!(std::env::var_os("LUMA_TEST_ROOT").is_some() && s.clients.borrow().is_empty());
    let state = s.clone();
    let start = std::time::Instant::now();
    let mut stage = 0;
    println!("PERF baseline");
    gtk::glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let seconds = start.elapsed().as_secs();
        match stage {
            0 if seconds >= 10 => {
                crate::ui::resource_rows(&state, 1000);
                println!("PERF inbox_1000");
                stage = 1;
            }
            1 if seconds >= 18 => {
                crate::ui::resource_scroll(&state, true);
                println!("PERF scrolled_1000");
                stage = 2;
            }
            2 if seconds >= 28 => {
                crate::ui::resource_scroll(&state, false);
                crate::rendering::performance_fixture(&state, 1);
                println!("PERF newsletter");
                stage = 3;
            }
            3 if seconds >= 43 => {
                crate::rendering::performance_fixture(&state, 3);
                println!("PERF three_html_messages");
                stage = 4;
            }
            4 if seconds >= 58 => {
                crate::ui::clear(&state.reader);
                trim();
                println!("PERF released");
                stage = 5;
            }
            5 if seconds >= 73 => {
                crate::rendering::performance_fixture(&state, 1);
                state.window.set_visible(false);
                println!("PERF background");
                stage = 6;
            }
            6 if seconds >= 123 => {
                println!("PERF reclaimed_background");
                stage = 7;
            }
            7 if seconds >= 133 => {
                println!("PERF bound_rows {}", state.list.bound_count());
                state.app.quit();
                return gtk::glib::ControlFlow::Break;
            }
            _ => (),
        }
        gtk::glib::ControlFlow::Continue
    });
}
