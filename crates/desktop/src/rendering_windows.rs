//! Windows mail HTML shares the short-lived, event-driven private WebView2 host.
use super::*;
use gtk::glib;
pub(super) fn append(
    s: &Rc<State>,
    body: &gtk::Box,
    message: &Message,
    account: &str,
    html: String,
) {
    let weak = Rc::downgrade(s);
    let surface = crate::windows_surface::Surface::new(
        crate::windows_platform::webview_runtime(),
        String::new(),
        true,
        move |uri| {
            if let Some(s) = weak.upgrade() {
                open_link(&s, &uri);
            }
        },
    );
    let area = surface.area.clone();
    area.set_widget_name("html-mail-view");
    let handle = surface.clone();
    html_controls(
        s,
        body,
        area.upcast_ref(),
        message,
        account,
        html,
        move |page, zoom| handle.load(page, zoom),
    );
    body.append(&area);
    if std::env::args().any(|a| matches!(a.as_str(), "--html-diagnostics" | "--render-diagnostics"))
    {
        let state = s.clone();
        let weak = area.downgrade();
        let minimum = std::env::var("LUMA_EXPECT_HTML_MIN_HEIGHT")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(200);
        let mut stage = 0;
        let mut ticks = 0;
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            ticks += 1;
            assert!(ticks < 150, "native HTML diagnostics timed out");
            let area = weak.upgrade().expect("HTML surface stays mapped");
            match stage {
                0 if area.height_request() > minimum => {
                    assert!(
                        area.height_request() < 60000,
                        "synthetic newsletter fits within the bounded full-content view"
                    );
                    println!(
                        "PASS WebView2 content height {} GTK units",
                        area.height_request()
                    );
                    crate::settings::open(&state);
                    stage = 1;
                }
                1 => {
                    assert!(state.window.visible_dialog().is_some());
                    let visible = surface.visible();
                    assert!(!visible, "native HTML must not overlap Settings");
                    state.window.visible_dialog().unwrap().force_close();
                    println!("PASS native newsletter is hidden under Settings");
                    stage = 2;
                }
                2 if state.window.visible_dialog().is_none() => {
                    let visible = surface.visible();
                    assert!(visible, "newsletter restores after Settings closes");
                    println!("PASS native newsletter restores after Settings");
                    let media = crate::media::from_bytes(silent_wav()).unwrap();
                    let controls = gtk::MediaControls::new(Some(&media));
                    state.reader.append(&controls);
                    let state = state.clone();
                    glib::MainContext::default().spawn_local(async move{
                        for _ in 0..100{if media.is_prepared()||media.error().is_some(){break}glib::timeout_future(std::time::Duration::from_millis(50)).await;}
                        assert!(media.error().is_none(),"Windows media decoder: {:?}",media.error());assert!(media.is_prepared()&&media.duration()>0,"In-memory WAV must prepare without a temporary attachment");media.clear();
                        println!("PASS Windows in-memory media decoding and controls; no attachment spooling");state.app.quit();
                    });
                    return glib::ControlFlow::Break;
                }
                _ => (),
            }
            glib::ControlFlow::Continue
        });
    }
}

fn silent_wav() -> Vec<u8> {
    let len = 16000u32;
    let mut b = Vec::with_capacity(len as usize + 44);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(len + 36).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&8000u32.to_le_bytes());
    b.extend_from_slice(&16000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&len.to_le_bytes());
    b.resize(len as usize + 44, 0);
    b
}
pub(super) fn diagnostic(s: &Rc<State>) {
    ui::clear(&s.reader);
    append(
        s,
        &s.reader,
        &Message::default(),
        "windows-fixture",
        include_str!("../tests/fixtures/layout-mail.html").into(),
    );
}
