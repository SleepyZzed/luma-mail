//! Resource reclamation at lifecycle boundaries, never a busy idle loop.
pub fn trim() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(2), || {
        // Return already-free allocator arenas after views/buffers are dropped.
        // This neither frees live memory nor imposes an allocation/renderer cap.
        unsafe {
            libc::malloc_trim(0);
        }
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
                crate::rendering::performance_fixture(&state, 1);
                println!("PERF newsletter");
                stage = 1;
            }
            1 if seconds >= 25 => {
                crate::rendering::performance_fixture(&state, 3);
                println!("PERF three_html_messages");
                stage = 2;
            }
            2 if seconds >= 40 => {
                crate::ui::clear(&state.reader);
                trim();
                println!("PERF released");
                stage = 3;
            }
            3 if seconds >= 55 => {
                crate::rendering::performance_fixture(&state, 1);
                state.window.set_visible(false);
                println!("PERF background");
                stage = 4;
            }
            4 if seconds >= 105 => {
                println!("PERF reclaimed_background");
                stage = 5;
            }
            5 if seconds >= 115 => {
                state.app.quit();
                return gtk::glib::ControlFlow::Break;
            }
            _ => (),
        }
        gtk::glib::ControlFlow::Continue
    });
}
