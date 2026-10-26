//! Explicit Gmail draft deletion; never a thread Trash or permanent mail delete.
use super::*;

pub(super) fn delete(s: &Rc<State>, keys: Vec<ThreadKey>) {
    if *s.folder.borrow() != Folder::Drafts || keys.is_empty() {
        return;
    }
    if s.editor.borrow().is_some() {
        s.info("Close the composer before deleting drafts so an autosave cannot recreate one.");
        return;
    }
    let dialog = adw::AlertDialog::builder()
        .heading(if keys.len() == 1 { "Delete draft?".into() } else { format!("Delete drafts in {} conversations?", keys.len()) })
        .body("Permanently discard the Gmail drafts in this selection. Sent and received messages in these conversations are kept. This cannot be undone.")
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete draft")]);
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let state = s.clone();
    glib::spawn_future_local(async move {
        if dialog.choose_future(Some(&state.window)).await != "delete"
            || *state.folder.borrow() != Folder::Drafts
            || state.editor.borrow().is_some()
        {
            return;
        }
        run(&state, keys);
    });
}

fn run(s: &Rc<State>, keys: Vec<ThreadKey>) {
    if s.operation_busy.replace(true) {
        s.info("Finishing the current Gmail change…");
        return;
    }
    let generation = s.generation.get();
    let jobs: Vec<_> = keys
        .into_iter()
        .filter_map(|key| {
            s.clients
                .borrow()
                .get(&key.account)
                .cloned()
                .map(|client| (key, client))
        })
        .collect();
    s.info("Deleting selected Gmail drafts…");
    async_work(
        s,
        async move {
            let mut outcomes = Vec::new();
            for (key, client) in jobs {
                let result = client.discard_thread_drafts(&key.id).await;
                outcomes.push((key, result));
            }
            Ok(outcomes)
        },
        move |s, outcomes| {
            s.operation_busy.set(false);
            let mut deleted = 0;
            let mut failure = None;
            for (key, result) in outcomes {
                match result {
                    Ok(count) => {
                        deleted += count;
                        if s.generation.get() == generation && *s.folder.borrow() == Folder::Drafts
                        {
                            rows::apply(
                                &s,
                                rows::Update {
                                    key,
                                    row: None,
                                    visible: Some(false),
                                    selected_labels: None,
                                },
                            );
                        }
                    }
                    Err(error) => failure = Some(error),
                }
            }
            if let Some(error) = failure {
                s.refresh_button.add_css_class("suggested-action");
                s.error(&Error::Invalid(format!("{deleted} drafts deleted. {error} Refresh Drafts to reconcile any partial change.")));
            } else {
                s.info(&format!(
                    "Deleted {deleted} Gmail draft(s). Other messages were kept."
                ));
            }
        },
    );
}
