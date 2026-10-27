//! Exercise the real GTK-free child, not a second implementation of its routes.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
struct Client {
    child: Child,
    input: ChildStdin,
    events: mpsc::Receiver<Value>,
    root: tempfile::TempDir,
}
impl Client {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_luma-engine"))
            .arg("--design-preview")
            .env("LUMA_TEST_ROOT", root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, events) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(event) = serde_json::from_str(&line) else {
                    break;
                };
                if tx.send(event).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            events,
            root,
        }
    }
    fn send(&mut self, id: u64, action: &str, data: Value) {
        writeln!(
            self.input,
            "{}",
            json!({"id":id,"action":action,"data":data})
        )
        .unwrap();
        self.input.flush().unwrap();
    }
    fn until(&self, kind: &str) -> Value {
        for _ in 0..24 {
            let e = self
                .events
                .recv_timeout(Duration::from_secs(5))
                .expect("engine response deadline");
            assert_ne!(e["event"], "error", "{}", e["data"]);
            if e["event"] == kind {
                return e["data"].clone();
            }
        }
        panic!("expected {kind}")
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn real_pipe_navigation_read_actions_and_zero_cache_do_not_write_a_mailbox() {
    let mut c = Client::new();
    let ready = c.until("ready");
    assert!(ready["preview"].as_bool().unwrap());
    assert_eq!(ready["settings"]["cache_mib"], 0);
    assert_eq!(ready["settings"]["draft_mode"], "ask");
    let initial = c.until("rows");
    assert_eq!(initial["items"].as_array().unwrap().len(), 8);
    c.send(
        1,
        "view",
        json!({"folder":"inbox","accounts":["preview"],"search":""}),
    );
    let rows = c.until("rows");
    assert_eq!(rows["generation"], 1);
    let key = rows["items"][0]["key"].clone();
    c.send(2, "select", key.clone());
    let thread = c.until("thread");
    assert_eq!(thread["key"], key);
    let body = c.until("body");
    assert!(body["text"].as_str().unwrap().contains("synthetic"));
    assert_eq!(body["account"], "preview");
    assert_eq!(body["reading"], 2);
    c.send(
        3,
        "mutate",
        json!({"keys":[key.clone()],"action":"archive"}),
    );
    c.send(4, "cancel", json!({"request":3}));
    let changed = c.until("mutation");
    assert_eq!(changed["removed"][0], key);
    c.send(
        5,
        "settingsUpdate",
        json!({"cache_mib":0,"reduced_motion":true}),
    );
    let settings = c.until("settings");
    assert_eq!(settings["cache_mib"], 0);
    assert_eq!(settings["reduced_motion"], true);
    c.send(6, "cacheUsage", json!({}));
    assert_eq!(c.until("cacheUsage")["bytes"], 0);
    assert!(!c.root.path().join("config/compose").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}

#[test]
fn a_large_or_unterminated_pipe_frame_is_rejected_before_json_allocation() {
    let mut c = Client::new();
    c.until("ready");
    c.until("rows");
    let frame = vec![b'x'; 4 * 1024 * 1024 + 1];
    let _ = c.input.write_all(&frame);
    let _ = c.input.flush();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = c.child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!c.root.path().join("cache/bodies").exists());
}
