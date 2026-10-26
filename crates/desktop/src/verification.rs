//! Explicit debug-only real-flow recording guard. No private mailbox list is shown.
use crate::ui::State;
use adw::prelude::*;
pub fn active() -> bool {
    std::env::args().any(|arg| arg == "--verification-session")
}
pub fn configure(s: &State) {
    if !active() {
        return;
    }
    assert!(
        std::env::var_os("LUMA_TEST_ROOT").is_some(),
        "verification uses isolated configuration"
    );
    s.window.set_title(Some(concat!(
        "Luma Mail ",
        env!("CARGO_PKG_VERSION"),
        " · Verification session"
    )));
    s.search.set_text("subject:Luma-Review-20261026");
    s.search.set_editable(false);
    s.search.set_tooltip_text(Some(
        "Recording guard: only this session's synthetic review mail is queried",
    ));
}
pub fn browser_url(s: &State, url: &str) -> bool {
    if !active() {
        return false;
    }
    let path = s.paths.config.join("review-oauth-url.txt");
    // This is the public authorization request, never a callback code or token.
    if let Err(error) = std::fs::write(&path, url).and_then(|_| {
        luma_core::config::private_file(&path).map_err(|e| std::io::Error::other(e.to_string()))
    }) {
        s.info(&format!(
            "Cannot prepare controlled review browser: {error}"
        ));
    }
    true
}
