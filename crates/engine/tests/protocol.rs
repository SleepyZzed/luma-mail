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
        Self::configured(false)
    }
    fn configured(two_accounts: bool) -> Self {
        Self::options(two_accounts, false)
    }
    fn options(two_accounts: bool, photos: bool) -> Self {
        Self::features(two_accounts, photos, false)
    }
    fn features(two_accounts: bool, photos: bool, html: bool) -> Self {
        Self::fixture(two_accounts, photos, html, false)
    }
    fn fixture(two_accounts: bool, photos: bool, html: bool, conflict: bool) -> Self {
        Self::fixture_count(two_accounts, photos, html, conflict, 1, false)
    }
    fn fixture_count(
        two_accounts: bool,
        photos: bool,
        html: bool,
        conflict: bool,
        count: usize,
        auth: bool,
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_luma-engine"))
            .arg("--design-preview")
            .env("LUMA_TEST_ROOT", root.path())
            .env("LUMA_PREVIEW_AUTH", if auth { "1" } else { "0" })
            .env("LUMA_PREVIEW_THREAD_COUNT", count.to_string())
            .env("LUMA_PREVIEW_PHOTOS", if photos { "1" } else { "0" })
            .env("LUMA_PREVIEW_HTML", if html { "1" } else { "0" })
            .env("LUMA_PREVIEW_CONFLICT", if conflict { "1" } else { "0" })
            .env(
                "LUMA_PREVIEW_TWO_ACCOUNTS",
                if two_accounts { "1" } else { "0" },
            )
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
    fn expect_error(&self, id: u64) -> Value {
        for _ in 0..24 {
            let event = self.events.recv_timeout(Duration::from_secs(5)).unwrap();
            if event["event"] == "error" {
                assert_eq!(event["id"], id);
                return event["data"].clone();
            }
        }
        panic!("expected error for request {id}")
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
    assert!(!c.root.path().join("config/composer").exists());
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

#[test]
fn composer_input_and_close_use_ram_only_and_quit_can_be_cancelled() {
    let mut c = Client::new();
    c.until("ready");
    c.until("rows");
    c.send(1, "composeStart", json!({"mode":"new","account":"preview"}));
    let opened = c.until("composeOpened");
    let session = opened["session"].clone();
    assert_eq!(opened["document"]["text"], "");
    c.send(
        2,
        "composeUpdate",
        json!({"session":session,"to":"person@example.com","text":"RAM only","html":false}),
    );
    assert_eq!(c.until("compose")["revision"], 1);
    c.send(3, "composeStart", json!({"mode":"reply"}));
    assert_eq!(c.until("composeFocus")["session"], session);
    c.send(4, "composePrepareQuit", json!({}));
    let q = c.until("composeCloseQuestion");
    c.send(
        5,
        "composeCloseAnswer",
        json!({"session":session,"revision":q["revision"],"answer":"keep"}),
    );
    c.until("quitCancelled");
    c.until("compose");
    c.send(6, "settingsUpdate", json!({"draft_mode":"manual"}));
    c.until("settings");
    c.send(7, "composeClose", json!({"session":session}));
    c.until("composeClosed");
    c.send(8, "composeStart", json!({"mode":"new"}));
    let fresh = c.until("composeOpened");
    assert_ne!(fresh["session"], session);
    assert_eq!(fresh["document"]["text"], "");
    c.send(9, "composePrepareQuit", json!({}));
    c.until("quitReady");
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}
#[test]
fn clipboard_transfer_rejects_bad_data_and_creates_only_a_ram_attachment() {
    use base64::Engine;
    let mut c = Client::new();
    c.until("ready");
    c.until("rows");
    c.send(1, "composeStart", json!({"mode":"new"}));
    let s = c.until("composeOpened")["session"].clone();
    c.send(
        2,
        "composeClipboardBegin",
        json!({"session":s,"token":"bad"}),
    );
    c.until("clipboardAck");
    c.send(
        3,
        "composeClipboardChunk",
        json!({"session":s,"token":"bad","size":3,"offset":0,"bytes":"!!!"}),
    );
    assert_eq!(c.until("clipboardComplete")["ok"], false);
    c.until("compose");
    let png=base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNg+P//PwAF/wL+Xg47rQAAAABJRU5ErkJggg==").unwrap();
    c.send(
        4,
        "composeClipboardBegin",
        json!({"session":s,"token":"good"}),
    );
    c.until("clipboardAck");
    c.send(5,"composeClipboardChunk",json!({"session":s,"token":"good","size":png.len(),"offset":0,"bytes":base64::engine::general_purpose::STANDARD.encode(&png)}));
    assert_eq!(c.until("clipboardAck")["next"], png.len());
    c.send(
        6,
        "composeClipboardFinish",
        json!({"session":s,"token":"good"}),
    );
    assert_eq!(c.until("clipboardComplete")["ok"], true);
    let state = c.until("compose");
    assert_eq!(state["attachments"].as_array().unwrap().len(), 1);
    assert!(state["draftId"].is_null());
    assert_eq!(state["inputBusy"], false);
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}
#[test]
fn account_switch_and_recent_recipients_are_account_scoped_and_respect_opt_out() {
    let mut c = Client::configured(true);
    c.until("ready");
    c.until("rows");
    let store = luma_core::local_compose::Store::new(&c.root.path().join("config"));
    store.configure(false, true);
    store.set_accounts(["preview".into(), "preview-b".into()]);
    for (account, to) in [
        ("preview", "Maya Chen <maya@example.com>"),
        ("preview-b", "Morgan <morgan@example.com>"),
    ] {
        store
            .remember(
                account,
                &luma_core::composer::Compose {
                    to: to.into(),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    c.send(1, "composeStart", json!({"mode":"new","account":"preview"}));
    let s = c.until("composeOpened")["session"].clone();
    c.send(2, "composeRecipients", json!({"session":s,"prefix":"Ma"}));
    let a = c.until("recipients");
    assert_eq!(a["request"], 2);
    assert_eq!(a["account"], "preview");
    assert_eq!(a["items"][0], "Maya Chen <maya@example.com>");
    c.send(
        3,
        "composeAccount",
        json!({"session":s,"account":"preview-b"}),
    );
    let b = c.until("compose");
    assert_eq!(b["account"], "preview-b");
    assert_eq!(b["from"], "jamie@example.com");
    c.send(4, "composeRecipients", json!({"session":s,"prefix":"Mo"}));
    assert_eq!(
        c.until("recipients")["items"][0],
        "Morgan <morgan@example.com>"
    );
    c.send(5, "settingsUpdate", json!({"recent_recipients":false}));
    c.until("settings");
    c.send(6, "composeRecipients", json!({"session":s,"prefix":"Mo"}));
    assert!(
        c.until("recipients")["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(!c.root.path().join("cache/bodies").exists());
    assert!(
        std::fs::read_dir(c.root.path().join("config/composer"))
            .unwrap()
            .all(|e| e.unwrap().path().extension().is_none_or(|s| s != "draft"))
    );
}
#[test]
fn photos_use_bounded_pixel_frames_without_a_disk_cache() {
    use base64::Engine;
    let mut c = Client::options(false, true);
    c.until("ready");
    c.until("rows");
    c.send(
        1,
        "photoRequest",
        json!({"account":"preview","sender":"Maya Chen <maya@example.com>","token":"photo1"}),
    );
    let photo = c.until("photo");
    assert_eq!(photo["state"], "available");
    assert_eq!(photo["requests"][0], "photo1");
    assert_eq!(photo["width"], 96);
    let pixels = base64::engine::general_purpose::STANDARD
        .decode(photo["rgba"].as_str().unwrap())
        .unwrap();
    assert_eq!(pixels.len(), 96 * 96 * 4);
    c.send(2, "photosReset", json!({}));
    c.until("photosReset");
    assert!(!c.root.path().join("cache/bodies").exists());
    assert!(!c.root.path().join("config/composer").exists());
}
#[test]
fn html_documents_preserve_layout_theme_and_privacy_without_mail_files() {
    let mut c = Client::features(false, false, true);
    c.until("ready");
    c.until("rows");
    c.send(1, "select", json!({"account":"preview","id":"preview0"}));
    c.until("thread");
    assert_eq!(c.until("body")["hasHtml"], true);
    let mut params = json!({"account":"preview","id":"preview0","background":[25,24,31],"foreground":[242,239,247],"dark":true});
    c.send(2, "renderMessage", params.clone());
    let doc = c.until("readerDocument");
    let html = doc["html"].as_str().unwrap();
    assert!(
        html.contains("<table")
            && html.contains("color-scheme:dark")
            && html.contains("#19181f")
            && html.contains("https://example.com/review")
    );
    assert!(
        !html.contains("<script")
            && !html.contains("<iframe")
            && !html.contains("src=\"https://")
            && !html.contains("Hidden title")
    );
    params["original"] = json!(true);
    c.send(3, "renderMessage", params);
    assert!(
        c.until("readerDocument")["html"]
            .as_str()
            .unwrap()
            .contains("color-scheme:light")
    );
    c.send(4, "releaseReader", json!({}));
    c.send(5, "cacheUsage", json!({}));
    c.until("cacheUsage");
    assert!(!c.root.path().join("cache/bodies").exists());
    assert!(!c.root.path().join("config/composer").exists());
}
#[test]
fn attachment_previews_and_media_chunks_are_memory_only_and_generation_checked() {
    use base64::Engine;
    let mut c = Client::new();
    c.until("ready");
    c.until("rows");
    let inputs = c.root.path().join("inputs");
    std::fs::create_dir(&inputs).unwrap();
    let note = inputs.join("notes.txt");
    std::fs::write(&note, b"Plain attachment preview").unwrap();
    let wav = inputs.join("tone.wav");
    let samples = 320000u32;
    let mut sound = b"RIFF".to_vec();
    sound.extend_from_slice(&(samples + 36).to_le_bytes());
    sound.extend_from_slice(b"WAVEfmt ");
    sound.extend_from_slice(&16u32.to_le_bytes());
    sound.extend_from_slice(&1u16.to_le_bytes());
    sound.extend_from_slice(&1u16.to_le_bytes());
    sound.extend_from_slice(&8000u32.to_le_bytes());
    sound.extend_from_slice(&8000u32.to_le_bytes());
    sound.extend_from_slice(&1u16.to_le_bytes());
    sound.extend_from_slice(&8u16.to_le_bytes());
    sound.extend_from_slice(b"data");
    sound.extend_from_slice(&samples.to_le_bytes());
    sound.resize(samples as usize + 44, 128);
    std::fs::write(&wav, &sound).unwrap();
    let png = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNg+P//PwAF/wL+Xg47rQAAAABJRU5ErkJggg==").unwrap();
    let image = inputs.join("image.png");
    std::fs::write(&image, png).unwrap();
    c.send(1, "composeStart", json!({"mode":"new","account":"preview"}));
    let session = c.until("composeOpened")["session"].clone();
    c.send(
        2,
        "composeAttach",
        json!({"session":session,"paths":[note,wav,image]}),
    );
    let state = loop {
        let state = c.until("compose");
        if state["attachments"]
            .as_array()
            .is_some_and(|a| a.len() == 3)
        {
            break state;
        }
    };
    let generation = state["attachmentGeneration"].clone();
    let attachment = state["attachments"][0]["id"].clone();
    let media = state["attachments"][1]["id"].clone();
    c.send(30,"attachmentThumbnail",json!({"session":session,"generation":generation,"attachment":state["attachments"][2]["id"],"token":"thumb1"}));
    let thumbnail = c.until("attachmentThumbnail");
    assert_eq!(thumbnail["width"], 1);
    assert_eq!(thumbnail["height"], 1);
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(thumbnail["rgba"].as_str().unwrap())
            .unwrap()
            .len(),
        4
    );
    c.send(
        3,
        "attachmentPreview",
        json!({"session":session,"generation":generation,"attachment":attachment,"token":"text1"}),
    );
    let preview = c.until("attachmentPreview");
    assert_eq!(preview["kind"], "Text");
    assert_eq!(preview["text"], "Plain attachment preview");
    c.send(
        4,
        "attachmentPreview",
        json!({"session":session,"generation":generation,"attachment":media,"token":"media1"}),
    );
    let preview = c.until("attachmentPreview");
    assert_eq!(preview["kind"], "Audio");
    assert_eq!(preview["size"], sound.len());
    let mut received = Vec::new();
    let mut id = 5;
    while received.len() < sound.len() {
        c.send(
            id,
            "attachmentRead",
            json!({"token":"media1","offset":received.len()}),
        );
        let chunk = c.until("attachmentChunk");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(chunk["data"].as_str().unwrap())
            .unwrap();
        assert!(bytes.len() <= 256 * 1024);
        received.extend_from_slice(&bytes);
        assert_eq!(chunk["done"], received.len());
        id += 1;
    }
    assert_eq!(received, sound);
    c.send(id, "attachmentRelease", json!({"token":"media1"}));
    id += 1;
    c.send(id, "attachmentRead", json!({"token":"media1","offset":0}));
    let error = c.events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(error["event"], "error");
    id += 1;
    c.send(
        id,
        "attachmentPreview",
        json!({"session":session,"generation":0,"attachment":attachment,"token":"stale1"}),
    );
    let error = c.events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(error["event"], "error");
    assert!(
        error["data"]["message"]
            .as_str()
            .unwrap()
            .contains("changed")
    );
    id += 1;
    let exports = c.root.path().join("exports");
    std::fs::create_dir(&exports).unwrap();
    c.send(id,"attachmentSave",json!({"session":session,"generation":generation,"attachment":attachment,"directory":exports}));
    c.until("attachmentSaved");
    assert_eq!(
        std::fs::read(exports.join("notes.txt")).unwrap(),
        b"Plain attachment preview"
    );
    id += 1;
    c.send(
        id,
        "attachmentSaveAll",
        json!({"token":"batch1","directory":exports,"attachments":[
            {"session":session,"generation":generation,"attachment":attachment},
            {"session":session,"generation":generation,"attachment":attachment}
        ]}),
    );
    let saved = c.until("attachmentSaveFinished");
    assert_eq!(saved["saved"], 2);
    assert_eq!(saved["cancelled"], false);
    assert!(saved["errors"].as_array().unwrap().is_empty());
    for name in ["notes.txt", "notes (1).txt", "notes (2).txt"] {
        assert_eq!(
            std::fs::read(exports.join(name)).unwrap(),
            b"Plain attachment preview"
        );
    }
    assert!(!c.root.path().join("cache/bodies").exists());
    assert!(!c.root.path().join("config/composer").exists());
}

#[test]
fn label_crud_sender_rules_and_draft_delete_keep_their_scope_guards() {
    let mut c = Client::new();
    c.until("ready");
    c.until("rows");
    c.send(1, "labels", json!({"account":"preview","operation":"list"}));
    let labels = c.until("labels");
    assert_eq!(labels["items"].as_array().unwrap().len(), 2);
    c.send(
        2,
        "labels",
        json!({"account":"preview","operation":"create","name":"Project <literal>"}),
    );
    let labels = c.until("labels");
    let id = labels["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "Project <literal>")
        .unwrap()["id"]
        .clone();
    c.send(
        3,
        "labels",
        json!({"account":"preview","operation":"rename","label":id,"name":"Research"}),
    );
    let labels = c.until("labels");
    assert!(
        labels["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["name"] == "Research")
    );
    c.send(
        4,
        "labels",
        json!({"account":"preview","operation":"delete","label":id}),
    );
    assert!(
        c.expect_error(4)["message"]
            .as_str()
            .unwrap()
            .contains("Confirm")
    );
    c.send(
        5,
        "labels",
        json!({"account":"preview","operation":"delete","label":id,"confirmed":true}),
    );
    assert_eq!(c.until("labels")["items"].as_array().unwrap().len(), 2);
    c.send(6,"senderRule",json!({"account":"preview","operation":"block","sender":"Alex <alex@example.com>","confirmed":true}));
    let settings = c.until("settings");
    assert_eq!(
        settings["accounts"][0]["blocked_senders"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    c.until("senderRuleSaved");
    c.send(7,"senderRule",json!({"account":"preview","operation":"block","sender":"alex@example.com","confirmed":true}));
    assert_eq!(
        c.until("settings")["accounts"][0]["blocked_senders"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    c.until("senderRuleSaved");
    c.send(8,"senderRule",json!({"account":"preview","operation":"unblock","sender":"alex@example.com","confirmed":true}));
    assert!(
        c.until("settings")["accounts"][0]["blocked_senders"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    c.until("senderRuleSaved");
    let keys = json!([{"account":"preview","id":"preview0"}]);
    c.send(9, "draftDelete", json!({"keys":keys,"confirmed":true}));
    assert!(
        c.expect_error(9)["message"]
            .as_str()
            .unwrap()
            .contains("Drafts")
    );
    c.send(
        10,
        "view",
        json!({"folder":"drafts","accounts":["preview"]}),
    );
    c.until("rows");
    c.send(11, "draftDelete", json!({"keys":keys}));
    assert!(
        c.expect_error(11)["message"]
            .as_str()
            .unwrap()
            .contains("Confirm")
    );
    c.send(12, "draftDelete", json!({"keys":keys,"confirmed":true}));
    assert_eq!(c.until("draftsDeleted")["count"], 1);
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}

#[test]
fn concurrent_preferences_preserve_other_fields_and_account_controls_stay_scoped() {
    let mut c = Client::configured(true);
    c.until("ready");
    c.until("rows");
    c.send(1, "settingsUpdate", json!({"ui_scale_percent":115}));
    c.send(2, "settingsUpdate", json!({"reduced_motion":true}));
    c.send(
        3,
        "accountUpdate",
        json!({"account":"preview-b","name":"Second inbox"}),
    );
    let mut final_state = Value::Null;
    for _ in 0..3 {
        final_state = c.until("settings");
    }
    assert_eq!(final_state["ui_scale_percent"], 115);
    assert_eq!(final_state["reduced_motion"], true);
    assert_eq!(final_state["accounts"][1]["name"], "Second inbox");
    c.send(
        4,
        "accountReorder",
        json!({"accounts":["preview-b","preview"]}),
    );
    assert_eq!(c.until("settings")["accounts"][0]["id"], "preview-b");
    c.send(
        5,
        "accountUpdate",
        json!({"account":"preview-b","enabled":false}),
    );
    assert_eq!(c.until("settings")["accounts"][0]["enabled"], false);
    c.send(6, "accountRemove", json!({"account":"preview-b"}));
    assert!(
        c.expect_error(6)["message"]
            .as_str()
            .unwrap()
            .contains("Confirm")
    );
    c.send(
        7,
        "accountRemove",
        json!({"account":"preview-b","confirmed":true}),
    );
    let settings = c.until("settings");
    assert_eq!(settings["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(settings["accounts"][0]["id"], "preview");
    c.send(8, "cacheUsage", json!({}));
    let usage = c.until("cacheUsage");
    assert_eq!(usage["bytes"], 0);
    assert!(usage["accounts"].as_object().unwrap().is_empty());
    assert!(!c.root.path().join("cache").exists());
}

#[test]
fn mail_links_prefill_a_new_composer_without_autosave_or_replacing_active_input() {
    let mut c = Client::new();
    c.until("ready");
    c.until("rows");
    c.send(1,"composeStart",json!({"mode":"new","account":"preview","mailto":"mailto:alex+tag%40example.com?subject=Coffee%20plans&body=Hi%0Athere&attach=file:///private"}));
    let opened = c.until("composeOpened");
    assert_eq!(opened["to"], "alex+tag@example.com");
    assert_eq!(opened["subject"], "Coffee plans");
    assert_eq!(opened["document"]["text"], "Hi\nthere");
    assert!(opened["attachments"].as_array().unwrap().is_empty());
    c.send(
        2,
        "composeStart",
        json!({"mode":"new","account":"preview","mailto":"mailto:other@example.com"}),
    );
    assert!(
        c.expect_error(2)["message"]
            .as_str()
            .unwrap()
            .contains("already open")
    );
    c.send(3, "settingsUpdate", json!({"draft_mode":"manual"}));
    c.until("settings");
    c.send(4, "composeClose", json!({"session":opened["session"]}));
    c.until("composeClosed");
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}

#[test]
fn advanced_search_validates_dates_and_preserves_the_selected_account_subset() {
    let mut c = Client::configured(true);
    c.until("ready");
    c.until("rows");
    c.send(1,"searchBuild",json!({"folder":"all_mail","accounts":["preview-b"],"from":"Maya Chen","to":"alex@example.com","subject":"launch","words":"project","after":"2026-01-01","before":"2026-02-01","unread":true,"attachments":true}));
    let search = c.until("searchBuilt");
    let query = search["query"].as_str().unwrap();
    assert!(query.contains("from:\"Maya Chen\""));
    assert!(query.contains("after:2026/01/01"));
    assert!(query.contains("is:unread has:attachment project"));
    assert_eq!(search["accounts"], json!(["preview-b"]));
    c.send(
        2,
        "searchBuild",
        json!({"folder":"all_mail","allAccounts":true,"after":"2026-04-01","before":"2026-03-01"}),
    );
    assert!(
        c.expect_error(2)["message"]
            .as_str()
            .unwrap()
            .contains("later")
    );
    c.send(
        3,
        "searchBuild",
        json!({"folder":"all_mail","accounts":["missing"]}),
    );
    assert!(
        c.expect_error(3)["message"]
            .as_str()
            .unwrap()
            .contains("enabled")
    );
    c.send(
        4,
        "view",
        json!({"folder":"label:Label_work","accounts":["preview"]}),
    );
    c.until("rows");
    c.send(
        5,
        "searchBuild",
        json!({"folder":"label:Label_work","allAccounts":true}),
    );
    assert!(
        c.expect_error(5)["message"]
            .as_str()
            .unwrap()
            .contains("owns")
    );
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}

#[test]
fn conflict_review_requires_current_choices_and_opens_only_an_unsaved_copy() {
    let mut c = Client::fixture(false, false, false, true);
    c.until("ready");
    c.send(1, "composeStart", json!({"mode":"new"}));
    let original = c.until("composeOpened");
    let session = original["session"].clone();
    assert_eq!(original["state"], "conflict");
    c.send(2, "composeCompare", json!({"session":session}));
    let first = c.until("composeComparison");
    assert_eq!(first["fields"][4]["resolution"], "gmail");
    assert_eq!(first["fields"][5]["resolution"], "choose");
    c.send(3, "composeCompareCancel", json!({"session":session}));
    c.send(4,"composeMerge",json!({"session":session,"ticket":first["ticket"],"choices":[null,null,null,null,null,"mine"],"attachments":"mine"}));
    c.expect_error(4);
    c.send(5, "composeCompare", json!({"session":session}));
    let second = c.until("composeComparison");
    assert!(second["ticket"].as_u64() > first["ticket"].as_u64());
    c.send(6,"composeMerge",json!({"session":session,"ticket":second["ticket"],"choices":[null,null,null,null,null,"mine"],"attachments":"mine"}));
    let merged = c.until("composeOpened");
    assert_ne!(merged["session"], session);
    assert_eq!(merged["state"], "editing");
    assert!(merged["draftId"].is_null());
    assert_eq!(merged["dirty"], true);
    assert_eq!(merged["subject"], "Gmail subject");
    assert_eq!(merged["document"]["text"], "My edited message");
    c.send(7, "composeSave", json!({"session":merged["session"]}));
    c.expect_error(7);
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}

#[test]
fn online_signature_edits_are_ephemeral_and_quit_waits_for_an_explicit_choice() {
    let mut c = Client::new();
    c.until("ready");
    c.send(1, "signatureOpen", json!({"account":"preview"}));
    let opened = c.until("signatureOpened");
    let session = opened["session"].clone();
    assert_eq!(opened["editable"], true);
    c.send(
        2,
        "signatureUpdate",
        json!({"session":session,"dirty":true}),
    );
    c.send(3, "composePrepareQuit", json!({}));
    let question = c.until("signatureCloseQuestion");
    assert_eq!(question["quit"], true);
    c.send(4, "signatureKeepEditing", json!({"session":session}));
    c.until("quitCancelled");
    c.send(5,"signatureSave",json!({"session":session,"html":"<p style=\"color:#123abc\"><b>Name</b></p><script>bad()</script>"}));
    let saved = c.until("signatureSaved");
    assert!(!saved["html"].as_str().unwrap().contains("<script"));
    assert!(saved["html"].as_str().unwrap().contains("#123abc"));
    assert_eq!(saved["preview"], true);
    c.until("signatureStatus");
    c.send(
        6,
        "signatureUploadPicture",
        json!({"session":session,"path":"must-not-open-this-file","publicConfirmed":false}),
    );
    let error = c.expect_error(6);
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("Confirm public")
    );
    c.send(
        7,
        "signatureClose",
        json!({"session":session,"dirty":false}),
    );
    c.until("signatureClosed");
    c.send(
        8,
        "signatureUpdate",
        json!({"session":session,"dirty":true}),
    );
    c.expect_error(8);
    assert!(!c.root.path().join("config/signatures.json").exists());
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}

#[test]
fn long_conversations_page_headers_without_prefetching_or_dropping_the_latest() {
    let mut c = Client::fixture_count(false, false, false, false, 2503, false);
    c.until("ready");
    c.until("rows");
    let key = json!({"account":"preview","id":"preview1"});
    c.send(1, "select", key.clone());
    let first = c.until("thread");
    assert_eq!(first["messages"].as_array().unwrap().len(), 32);
    assert_eq!(first["window"]["start"], 2472);
    assert_eq!(first["window"]["end"], 2503);
    assert_eq!(c.until("body")["id"], "preview1");
    let mut seen = std::collections::HashSet::new();
    let mut current = first;
    let mut id = 2;
    loop {
        for header in current["messages"].as_array().unwrap() {
            assert!(!header.as_object().unwrap().contains_key("text"));
            seen.insert(header["id"].as_str().unwrap().to_owned());
        }
        if current["window"]["earlier"] == false {
            break;
        }
        c.send(
            id,
            "threadPage",
            json!({"key":key,"reading":1,"direction":"earlier"}),
        );
        current = c.until("threadHeaders");
        assert!(current["messages"].as_array().unwrap().len() <= 32);
        assert_eq!(current["window"]["revision"], id);
        c.until("body");
        id += 1;
    }
    assert_eq!(seen.len(), 2503);
    c.send(
        id,
        "threadPage",
        json!({"key":key,"reading":1,"direction":"latest"}),
    );
    let latest = c.until("threadHeaders");
    assert_eq!(
        latest["messages"].as_array().unwrap().last().unwrap()["id"],
        "preview1"
    );
    assert_eq!(c.until("body")["id"], "preview1");
    c.send(
        id + 1,
        "threadPage",
        json!({"key":key,"reading":1,"direction":"bad"}),
    );
    c.expect_error(id + 1);
    assert!(!c.root.path().join("cache/bodies").exists());
    assert!(!c.root.path().join("config/composer").exists());
}

#[test]
fn signin_cancel_retry_and_quit_do_not_interrupt_an_accepted_commit() {
    let mut c = Client::fixture_count(false, false, false, false, 1, true);
    c.until("ready");
    c.until("rows");
    c.send(
        1,
        "authStart",
        json!({"drive":true,"settings":true,"photos":true}),
    );
    let first = c.until("authUrl");
    assert!(
        first["url"]
            .as_str()
            .unwrap()
            .starts_with("https://accounts.google.com/")
    );
    c.send(2, "cancel", json!({"request":1}));
    c.until("authCancelled");
    c.send(3, "authStart", json!({}));
    c.until("authUrl");
    c.send(4, "previewAuthApprove", json!({"request":3}));
    c.until("authFinishing");
    c.send(5, "cancel", json!({"request":3}));
    c.until("authFinishing");
    c.send(6, "quit", json!({}));
    assert_eq!(c.until("authComplete")["synthetic"], true);
    assert!(c.child.wait().unwrap().success());
    assert!(!c.root.path().join("config/composer").exists());
    assert!(!c.root.path().join("cache/bodies").exists());
}
