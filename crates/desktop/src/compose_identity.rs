//! Per-editor verified identities, in RAM. Account and From address stay distinct.
use super::*;
use luma_core::send_as::SendAs;
use std::collections::{HashMap, HashSet};
#[derive(Clone, PartialEq, Eq)]
struct Row {
    account: usize,
    sender: SendAs,
    allowed: bool,
}
pub(super) struct Choices {
    rows: RefCell<Vec<Row>>,
    cache: RefCell<HashMap<usize, Vec<SendAs>>>,
    requested: RefCell<HashSet<usize>>,
    current: RefCell<(usize, String)>,
    locked: Cell<Option<usize>>,
    sync: Cell<bool>,
}
impl Choices {
    pub(super) fn new(accounts: &[luma_core::config::Account], c: &Compose, draft: bool) -> Self {
        let address = luma_core::sender_rules::address(&c.from).unwrap_or_else(|_| c.from.clone());
        let account = c
            .account
            .as_ref()
            .and_then(|id| accounts.iter().position(|a| &a.id == id))
            .or_else(|| {
                accounts
                    .iter()
                    .position(|a| a.email.eq_ignore_ascii_case(&address))
            })
            .unwrap_or(0);
        let rows = accounts
            .iter()
            .enumerate()
            .map(|(i, a)| Row {
                account: i,
                sender: SendAs::primary(&a.email),
                allowed: true,
            })
            .collect();
        Self {
            rows: RefCell::new(rows),
            cache: Default::default(),
            requested: Default::default(),
            current: RefCell::new((account, address)),
            locked: Cell::new(
                (draft || c.thread.is_some() || !c.drive_files.is_empty()).then_some(account),
            ),
            sync: Cell::new(false),
        }
    }
    pub(super) fn account_index(&self) -> usize {
        self.current.borrow().0
    }
}
fn label(row: &Row, e: &Editor) -> String {
    if !row.allowed {
        format!("{} · checking sender", row.sender.send_as_email)
    } else if row.sender.is_primary {
        row.sender.header()
    } else {
        format!(
            "{} · {}",
            row.sender.header(),
            e.accounts[row.account].email
        )
    }
}
pub(super) fn sender(e: &Editor) -> Result<SendAs> {
    let rows = e.identities.rows.borrow();
    rows.iter()
        .find(|r| {
            r.account == e.identities.current.borrow().0
                && r.sender
                    .send_as_email
                    .eq_ignore_ascii_case(&e.identities.current.borrow().1)
                && r.allowed
        })
        .map(|r| r.sender.clone())
        .ok_or_else(|| {
            Error::Invalid(
                "Choose a verified From address before saving or sending this draft".into(),
            )
        })
}
fn controls(e: &Editor) {
    let editable = !e.busy.get() && !e.closing.get() && !e.sent.get() && !e.uncertain.get();
    e.from
        .set_sensitive(editable && e.identities.rows.borrow().len() > 1);
    e.inline_from
        .set_sensitive(editable && e.identities.rows.borrow().len() > 1);
    if !e.busy.get() && !e.uncertain.get() {
        e.send
            .set_sensitive(editable && !e.conflict.get() && sender(e).is_ok());
        e.save
            .set_sensitive(editable && (e.conflict.get() || sender(e).is_ok()));
    }
}
pub(super) fn refresh(e: &Rc<Editor>) {
    if e.closing.get() || e.sent.get() {
        return;
    }
    if e.busy.get() {
        controls(e);
        return;
    }
    if e.draft.borrow().is_some() || !e.drive_files.borrow().is_empty() {
        e.identities.locked.set(Some(e.identities.account_index()));
    }
    let key = e.identities.current.borrow().clone();
    let cache = e.identities.cache.borrow();
    let mut rows = Vec::new();
    for (index, account) in e.accounts.iter().enumerate() {
        if e.identities
            .locked
            .get()
            .is_some_and(|locked| locked != index)
        {
            continue;
        }
        if let Some(senders) = cache.get(&index) {
            rows.extend(senders.iter().map(|s| Row {
                account: index,
                sender: s.clone(),
                allowed: true,
            }));
        } else {
            rows.push(Row {
                account: index,
                sender: SendAs::primary(&account.email),
                allowed: true,
            });
        }
    }
    if !rows
        .iter()
        .any(|r| r.account == key.0 && r.sender.send_as_email.eq_ignore_ascii_case(&key.1))
    {
        rows.push(Row {
            account: key.0,
            sender: SendAs::primary(&key.1),
            allowed: false,
        });
    }
    drop(cache);
    if *e.identities.rows.borrow() != rows || e.from.model().is_none() {
        let names: Vec<_> = rows.iter().map(|r| label(r, e)).collect();
        let names: Vec<_> = names.iter().map(String::as_str).collect();
        let model = gtk::StringList::new(&names);
        let selected = rows
            .iter()
            .position(|r| r.account == key.0 && r.sender.send_as_email.eq_ignore_ascii_case(&key.1))
            .unwrap_or(0) as u32;
        e.identities.sync.set(true);
        *e.identities.rows.borrow_mut() = rows;
        e.from.set_model(Some(&model));
        e.inline_from.set_model(Some(&model));
        e.from.set_selected(selected);
        e.inline_from.set_selected(selected);
        e.identities.sync.set(false);
    }
    controls(e);
}
fn load(e: &Rc<Editor>, retry: bool) {
    let index = e.identities.account_index();
    if e.busy.get()
        || e.closing.get()
        || (!retry && e.identities.requested.borrow().contains(&index))
    {
        return;
    }
    let Some(a) = e.accounts.get(index) else {
        return;
    };
    let Some(client) = e.state.clients.borrow().get(&a.id).cloned() else {
        return;
    };
    e.identities.requested.borrow_mut().insert(index);
    let email = a.email.clone();
    let weak = Rc::downgrade(e);
    ui::async_work(
        &e.state,
        async move { Ok(client.send_as(&email).await) },
        move |_, result| {
            let Some(e) = weak.upgrade().filter(|e| !e.closing.get() && !e.sent.get()) else {
                return;
            };
            match result {
                Ok(rows) => {
                    e.identities.cache.borrow_mut().insert(index, rows);
                }
                Err(error) => {
                    e.status.set_text(&format!("Could not load sender addresses: {error}. Use Retry senders or choose the primary address."));
                }
            }
            refresh(&e);
            if sender(&e).is_ok() && e.revision.get() > e.saved.get() && !e.busy.get() {
                changed(&e);
            }
        },
    );
}
fn factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let text = gtk::Label::new(None);
        text.set_xalign(0.);
        text.set_max_width_chars(48);
        text.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        item.set_child(Some(&text));
    });
    factory.connect_bind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let Some(value) = item.item().and_downcast::<gtk::StringObject>() else {
            return;
        };
        let Some(text) = item.child().and_downcast::<gtk::Label>() else {
            return;
        };
        text.set_text(&value.string());
        text.set_tooltip_text(Some(&value.string()));
    });
    factory
}
pub(super) fn install(e: &Rc<Editor>, retry: &gtk::Button) {
    // Force the initial model to include a preserved draft alias, even before its
    // authorization lookup completes. Never silently replace it with primary.
    e.from.set_model(None::<&gtk::StringList>);
    refresh(e);
    for input in [&e.from, &e.inline_from] {
        input.set_factory(Some(&factory()));
        input.set_list_factory(Some(&factory()));
        let weak = Rc::downgrade(e);
        input.connect_selected_notify(move |input| {
            let Some(e) = weak.upgrade() else {
                return;
            };
            if e.identities.sync.get() || e.busy.get() || e.closing.get() {
                return;
            }
            let Some(row) = e
                .identities
                .rows
                .borrow()
                .get(input.selected() as usize)
                .cloned()
            else {
                return;
            };
            *e.identities.current.borrow_mut() = (row.account, row.sender.send_as_email.clone());
            e.identities.sync.set(true);
            e.from.set_selected(input.selected());
            e.inline_from.set_selected(input.selected());
            e.identities.sync.set(false);
            controls(&e);
            changed(&e);
            load(&e, false);
        });
    }
    let weak = Rc::downgrade(e);
    retry.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            load(&e, true)
        }
    });
    load(e, false);
}

#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    let index = e.identities.account_index();
    let primary = e.account().unwrap().email.clone();
    let alias = SendAs {
        send_as_email: "alias@example.invalid".into(),
        display_name: "Alias Person".into(),
        reply_to_address: "reply@example.invalid".into(),
        is_primary: false,
        is_default: false,
        verification_status: "accepted".into(),
    };
    e.identities
        .cache
        .borrow_mut()
        .insert(index, vec![SendAs::primary(&primary), alias]);
    refresh(e);
    let position = e
        .identities
        .rows
        .borrow()
        .iter()
        .position(|r| !r.sender.is_primary)
        .unwrap() as u32;
    e.from.set_selected(position);
    let compose = snapshot(e).unwrap();
    assert!(compose.from.contains("alias@example.invalid"));
    assert_eq!(compose.sender_reply_to, "reply@example.invalid");
    assert_eq!(
        compose.account.as_deref(),
        Some(e.accounts[index].id.as_str())
    );
    assert_eq!(e.inline_from.selected(), position);
    assert_eq!(e.identities.account_index(), index);
    *e.identities.current.borrow_mut() = (index, "missing@example.invalid".into());
    refresh(e);
    assert!(
        sender(e).is_err() && !e.send.is_sensitive(),
        "Unknown draft aliases cannot silently send as primary"
    );
    *e.identities.current.borrow_mut() = (index, primary);
    refresh(e);
    println!(
        "PASS verified alias MIME identity, owning Gmail account, inline/full sender sync and missing-alias guard"
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_alias_keeps_its_owning_account_independent_of_dropdown_position() {
        let account = |id: &str, email: &str| {
            serde_json::from_value::<luma_core::config::Account>(serde_json::json!({"id":id,"email":email,"name":"Test","enabled":true,"notifications":false})).unwrap()
        };
        let accounts = vec![
            account("one", "one@example.invalid"),
            account("two", "two@example.invalid"),
        ];
        let c = Compose {
            account: Some("two".into()),
            from: "Alias <alias@example.invalid>".into(),
            ..Default::default()
        };
        let choices = Choices::new(&accounts, &c, true);
        assert_eq!(choices.account_index(), 1);
        assert_eq!(choices.locked.get(), Some(1));
        assert_eq!(choices.current.borrow().1, "alias@example.invalid");
    }
}
