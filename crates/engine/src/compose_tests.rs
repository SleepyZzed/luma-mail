use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct Remote {
    saves: AtomicUsize,
    sends: AtomicUsize,
    raw: Mutex<Vec<Vec<u8>>>,
    ids: Mutex<Vec<Option<String>>>,
    gate: Option<Arc<tokio::sync::Notify>>,
    fail_save: bool,
    conflict: bool,
    fail_send: bool,
}
#[async_trait]
impl DraftTransport for Remote {
    async fn save(&self, existing: Option<&Draft>, raw: &[u8], _: Option<&str>) -> Result<Draft> {
        let n = self.saves.fetch_add(1, Ordering::SeqCst) + 1;
        self.ids
            .lock()
            .unwrap()
            .push(existing.map(|d| d.id.clone()));
        self.raw.lock().unwrap().push(raw.to_vec());
        if let Some(gate) = &self.gate
            && n == 1
        {
            gate.notified().await;
        }
        if self.fail_save {
            return Err(Error::ResponseLimit);
        }
        if self.conflict {
            return Err(Error::DraftConflict);
        }
        Ok(Draft {
            id: existing
                .map(|d| d.id.clone())
                .unwrap_or_else(|| "draft1".into()),
            message: Message {
                id: format!("message{n}"),
                thread_id: "thread1".into(),
                ..Default::default()
            },
        })
    }
    async fn send(&self, _: &Draft) -> Result<Message> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        if self.fail_send {
            return Err(Error::ResponseLimit);
        }
        Ok(Message {
            id: "sent1".into(),
            ..Default::default()
        })
    }
}
fn editor() -> Arc<Session> {
    let mut form = Compose::new("alex@example.com".into());
    form.account = Some("account1".into());
    Session::new(Init {
        form,
        primary: "alex@example.com".into(),
        mode: "new".into(),
        key: None,
        draft: None,
        quote: String::new(),
        signature: String::new(),
        original_html: None,
        source_mode: false,
        document: json!({"kind":"plain","text":""}),
    })
}
fn type_body(s: &Session, id: u64, body: &str) {
    s.update(
        &json!({"to":"person@example.com","text":body,"html":false}),
        id,
    )
    .unwrap();
}
#[test]
fn upload_progress_is_retained_without_an_active_ui_poll() {
    let s = editor();
    assert!(
        s.progress
            .send(luma_core::compose_drive::Progress {
                name: "large.zip".into(),
                done: 123,
                total: 1000,
                sharing: false
            })
            .is_ok()
    );
    assert_eq!(s.snapshot(false)["progress"]["done"], 123);
    assert_eq!(s.snapshot(false)["progress"]["total"], 1000);
}
async fn started(remote: &Remote) {
    for _ in 0..1000 {
        if remote.saves.load(Ordering::SeqCst) > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    panic!("save did not start")
}

#[tokio::test]
async fn saves_reuse_one_remote_draft_and_send_once_without_late_recreation() {
    let s = editor();
    let remote = Remote::default();
    type_body(&s, 1, "First");
    s.queue(2, Intent::Save).unwrap();
    s.persist(&remote, None, false, 2).await.unwrap();
    type_body(&s, 3, "Latest");
    s.queue(4, Intent::Save).unwrap();
    s.persist(&remote, None, false, 4).await.unwrap();
    s.queue(5, Intent::Send).unwrap();
    assert!(!s.queue(6, Intent::Send).unwrap());
    s.persist(&remote, None, false, 5).await.unwrap();
    assert_eq!(
        remote.ids.lock().unwrap().as_slice(),
        [None, Some("draft1".into()), Some("draft1".into())]
    );
    assert_eq!(remote.sends.load(Ordering::SeqCst), 1);
    assert_eq!(s.data.lock().unwrap().phase, Phase::Sent);
    assert!(s.data.lock().unwrap().form.text.is_empty());
    assert!(s.queue(7, Intent::Send).is_err());
    assert!(s.update(&json!({"text":"late callback"}), 8).is_err());
    s.persist(&remote, None, false, 5).await.unwrap();
    assert_eq!(remote.saves.load(Ordering::SeqCst), 3);
}
#[tokio::test]
async fn save_in_flight_preserves_newer_text_and_queued_send_uses_it() {
    let s = editor();
    let gate = Arc::new(tokio::sync::Notify::new());
    let remote = Arc::new(Remote {
        gate: Some(gate.clone()),
        ..Default::default()
    });
    type_body(&s, 1, "Old");
    s.queue(2, Intent::Save).unwrap();
    let task = {
        let s = s.clone();
        let r = remote.clone();
        tokio::spawn(async move { s.persist(&*r, None, false, 2).await })
    };
    started(&remote).await;
    type_body(&s, 3, "Newer message");
    s.queue(4, Intent::Send).unwrap();
    let send = {
        let s = s.clone();
        let r = remote.clone();
        tokio::spawn(async move { s.persist(&*r, None, false, 4).await })
    };
    gate.notify_one();
    task.await.unwrap().unwrap();
    send.await.unwrap().unwrap();
    use base64::Engine;
    let expected = base64::engine::general_purpose::STANDARD.encode("Newer message");
    assert!(
        String::from_utf8_lossy(remote.raw.lock().unwrap().last().unwrap()).contains(&expected)
    );
    assert_eq!(remote.saves.load(Ordering::SeqCst), 2);
    assert_eq!(remote.sends.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn close_after_save_does_not_drop_edits_that_arrived_during_the_save() {
    let s = editor();
    let gate = Arc::new(tokio::sync::Notify::new());
    let remote = Arc::new(Remote {
        gate: Some(gate.clone()),
        ..Default::default()
    });
    type_body(&s, 1, "Before save");
    s.queue(2, Intent::SaveAndClose).unwrap();
    let task = {
        let s = s.clone();
        let r = remote.clone();
        tokio::spawn(async move { s.persist(&*r, None, false, 2).await })
    };
    started(&remote).await;
    type_body(&s, 3, "After save started");
    gate.notify_one();
    task.await.unwrap().unwrap();
    let d = s.data.lock().unwrap();
    assert_eq!(d.phase, Phase::Editing);
    assert_eq!(d.form.text, "After save started");
    assert!(d.revision > d.saved_revision);
}
#[tokio::test]
async fn uncertain_writes_lock_retries_and_keep_the_composer_for_reconciliation() {
    let s = editor();
    let remote = Remote {
        fail_send: true,
        ..Default::default()
    };
    type_body(&s, 1, "Keep this");
    s.queue(2, Intent::Send).unwrap();
    assert!(s.persist(&remote, None, false, 2).await.is_err());
    assert_eq!(s.data.lock().unwrap().phase, Phase::Uncertain);
    assert_eq!(s.data.lock().unwrap().draft.as_ref().unwrap().id, "draft1");
    assert_eq!(s.data.lock().unwrap().form.text, "Keep this");
    assert!(s.queue(3, Intent::Send).is_err());
    assert_eq!(remote.sends.load(Ordering::SeqCst), 1);
    assert!(
        matches!(s.close(DraftMode::Manual), Close::Ask(_)),
        "known uncertain delivery must be acknowledged even in Manual mode"
    );
}
#[tokio::test]
async fn conflicts_never_overwrite_the_changed_draft() {
    let s = editor();
    let remote = Remote {
        conflict: true,
        ..Default::default()
    };
    type_body(&s, 1, "Unsent local work");
    s.queue(2, Intent::Save).unwrap();
    assert!(s.persist(&remote, None, false, 2).await.is_err());
    assert_eq!(s.data.lock().unwrap().phase, Phase::Conflict);
    assert_eq!(s.data.lock().unwrap().form.text, "Unsent local work");
    assert!(s.queue(3, Intent::Send).is_err());
    type_body(&s, 4, "Can keep editing while choosing recovery");
    assert_eq!(s.data.lock().unwrap().phase, Phase::Conflict);
}
#[test]
fn typing_close_policies_and_stale_dialog_responses_do_not_autosave() {
    let s = editor();
    type_body(&s, 10, "Unsent");
    s.update(&json!({"text":"stale"}), 9).unwrap();
    assert_eq!(s.data.lock().unwrap().form.text, "Unsent");
    let Close::Ask(revision) = s.close(DraftMode::Ask) else {
        panic!("expected Ask")
    };
    type_body(&s, 11, "Newer");
    assert!(s.close_answer(revision, "close").is_err());
    assert!(s.active());
    let Close::Ask(revision) = s.close(DraftMode::Ask) else {
        panic!()
    };
    s.close_answer(revision, "keep").unwrap();
    assert!(s.active());
    assert!(matches!(s.close(DraftMode::OnClose), Close::Save));
    assert!(matches!(s.close(DraftMode::Manual), Close::Closed));
    assert!(s.data.lock().unwrap().form.text.is_empty());
    let s = editor();
    s.data.lock().unwrap().quote = "Only old history".into();
    s.data.lock().unwrap().signature = "Only a signature".into();
    s.update(&json!({"to":"person@example.com"}), 1).unwrap();
    assert!(matches!(s.close(DraftMode::OnClose), Close::Closed));
}
#[test]
fn reply_history_stays_outside_authored_input_and_can_be_excluded() {
    let s = editor();
    type_body(&s, 1, "My reply");
    {
        let mut d = s.data.lock().unwrap();
        d.quote = "Earlier message".into();
        d.signature = "<b>My signature</b>".into();
    }
    let form = delivery(&s.data.lock().unwrap()).unwrap();
    let parts = document::sections(&form.text).unwrap();
    assert!(parts.body.contains("My reply"));
    assert!(!parts.body.contains("Earlier message"));
    assert_eq!(parts.quote, "Earlier message");
    s.update(&json!({"includeQuote":false,"includeSignature":false}), 2)
        .unwrap();
    let form = delivery(&s.data.lock().unwrap()).unwrap();
    assert_eq!(form.text, "My reply");
    assert!(!form.html);
}
#[tokio::test]
async fn unverified_draft_sender_is_rejected_before_a_provider_write() {
    let s = editor();
    type_body(&s, 1, "No spoofing");
    s.data.lock().unwrap().form.from = "unverified@example.com".into();
    let remote = Remote::default();
    s.queue(2, Intent::Send).unwrap();
    assert!(s.persist(&remote, None, false, 2).await.is_err());
    assert_eq!(remote.saves.load(Ordering::SeqCst), 0);
    assert_eq!(remote.sends.load(Ordering::SeqCst), 0);
}
