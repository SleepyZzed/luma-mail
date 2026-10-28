//! Native component conformance, no Qt, Google account or OS input automation.
use super::*;
use std::{
    net::TcpListener,
    time::{Duration, Instant},
};
fn spin_until(mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "asynchronous native component timed out"
        );
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
extern "C" fn receive(context: *mut c_void, bytes: *const u8, size: usize) {
    unsafe {
        if let Ok(event) = serde_json::from_slice(std::slice::from_raw_parts(bytes, size)) {
            (&*(context as *const RefCell<Vec<serde_json::Value>>))
                .borrow_mut()
                .push(event);
        }
    }
}
unsafe fn create(
    parent: HWND,
    runtime: &std::path::Path,
    page: &str,
    events: &RefCell<Vec<serde_json::Value>>,
) -> *mut Surface {
    let path = runtime.to_str().unwrap();
    unsafe {
        luma_webview_create(
            parent.0 as isize,
            page.as_ptr(),
            page.len(),
            path.as_ptr(),
            path.len(),
            receive,
            events as *const _ as *mut c_void,
        )
    }
}
#[test]
fn async_private_reader_keeps_images_blocks_content_and_cancels_cleanly() {
    unsafe {
        assert!(APARTMENT.with(|a| a.0));
        let parent = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Luma component fixture"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            800,
            600,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let runtime = tempfile::tempdir().unwrap();
        let cancelled = RefCell::new(Vec::new());
        let start = Instant::now();
        let surface = create(
            parent,
            runtime.path(),
            "<p>Cancelled before initialization</p>",
            &cancelled,
        );
        assert!(!surface.is_null());
        // Cold SDK bootstrap/VM scheduling is measured, not treated as a
        // portable 250-ms guarantee. The completion-order assertion below
        // detects a nested blocking pump deterministically.
        println!(
            "WebView2 factory caller time: {} ms",
            start.elapsed().as_millis()
        );
        assert!(
            surface.as_ref().unwrap().0.controller.borrow().is_none(),
            "initialization must be asynchronous"
        );
        let child = surface.as_ref().unwrap().0.child;
        luma_webview_destroy(surface);
        spin_until(|| !IsWindow(Some(child)).as_bool());
        assert!(
            cancelled.borrow().is_empty(),
            "cancelled construction delivered late events"
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let page = format!(
            r#"<!doctype html><html><body style="margin:0"><script>window.senderRan=true</script><img id="inline" src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8XQAAAAASUVORK5CYII="><img src="http://127.0.0.1:{port}/must-not-fetch"><div style="height:900px">Latest document &amp; image</div></body></html>"#
        );
        let events = RefCell::new(Vec::new());
        let surface = create(
            parent,
            runtime.path(),
            "<p>Superseded during startup</p>",
            &events,
        );
        assert!(!surface.is_null());
        luma_webview_document(surface, page.as_ptr(), page.len());
        let rects = [0, 0, 600, 1000, 0, 0, 600, 300];
        luma_webview_geometry(surface, rects.as_ptr(), 2, 1., 1.);
        luma_webview_visible(surface, true);
        spin_until(|| {
            events
                .borrow()
                .iter()
                .any(|e| e["kind"] == "loaded" || e["kind"] == "error")
        });
        assert!(
            !events.borrow().iter().any(|e| e["kind"] == "error"),
            "{events:?}"
        );
        let web = surface
            .as_ref()
            .unwrap()
            .0
            .web
            .borrow()
            .as_ref()
            .unwrap()
            .clone();
        let mut private = Default::default();
        web.cast::<ICoreWebView2_13>()
            .unwrap()
            .Profile()
            .unwrap()
            .IsInPrivateModeEnabled(&mut private)
            .unwrap();
        assert!(private.as_bool());
        let settings = web.Settings().unwrap();
        let mut script = Default::default();
        settings.IsScriptEnabled(&mut script).unwrap();
        assert!(!script.as_bool());
        let mut hosts = Default::default();
        settings.AreHostObjectsAllowed(&mut hosts).unwrap();
        assert!(!hosts.as_bool());
        let inspection = Rc::new(RefCell::new(None));
        let result = inspection.clone();
        web.ExecuteScript(w!("JSON.stringify({inline:document.getElementById('inline').complete&&document.getElementById('inline').naturalWidth===1,injected:!!window.senderRan,latest:document.body.textContent.includes('Latest document'),height:document.body.getBoundingClientRect().height})"),&ExecuteScriptCompletedHandler::create(Box::new(move|status,value|{status?;*result.borrow_mut()=Some(value);Ok(())}))).unwrap();
        spin_until(|| inspection.borrow().is_some());
        let raw = inspection.borrow_mut().take().unwrap();
        let decoded: String = serde_json::from_str(&raw).unwrap();
        let result: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(result["inline"], true, "{result}");
        assert_eq!(result["injected"], false);
        assert_eq!(result["latest"], true);
        assert!(result["height"].as_f64().unwrap() > 900.);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "renderer attempted a network subresource"
        );
        luma_webview_measure(surface, 600, 73);
        spin_until(|| events.borrow().iter().any(|e| e["kind"] == "height"));
        assert!(events.borrow().iter().any(|e| e["kind"] == "height"
            && e["generation"] == 73
            && e["height"].as_f64().unwrap() > 900.));
        web.Navigate(w!("https://example.invalid/must-not-navigate"))
            .unwrap();
        spin_until(|| events.borrow().iter().any(|e| e["kind"] == "link"));
        let mut current = PWSTR::null();
        web.Source(&mut current).unwrap();
        assert_eq!(take_string(current), URL);
        let child = surface.as_ref().unwrap().0.child;
        drop(web);
        luma_webview_destroy(surface);
        spin_until(|| !IsWindow(Some(child)).as_bool());
        assert!(
            !runtime.path().read_dir().unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("initializing-")),
            "initialization lease leaked"
        );
        DestroyWindow(parent).unwrap();
    }
}
