//! The selected sender's online signature stays separate from authored text.
use super::*;
use luma_core::signatures::SignatureStore;
pub(super) struct SignatureState {
    pub root: gtk::Box,
    include: gtk::CheckButton,
    summary: gtk::Label,
    edit: gtk::Button,
    retry: gtk::Button,
    html: RefCell<String>,
    key: RefCell<Option<(String, String)>>,
    preserved: Cell<bool>,
    sync: Cell<bool>,
    opted_out: Cell<bool>,
    loading: Cell<bool>,
    error: RefCell<Option<String>>,
    epoch: Cell<u64>,
    task: RefCell<Option<tokio::task::AbortHandle>>,
}
impl SignatureState {
    pub(super) fn new(saved: &str, draft: bool) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        root.add_css_class("signature-summary");
        let include = gtk::CheckButton::with_label("Signature");
        include.set_active(!draft || !saved.is_empty());
        root.append(&include);
        let summary = ui::label("", "caption");
        summary.set_hexpand(true);
        summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
        root.append(&summary);
        let edit = ui::button("luma-compose-symbolic", "Edit the online signature");
        root.append(&edit);
        let retry = ui::button(
            "luma-refresh-symbolic",
            "Reload this sender's signature from Gmail",
        );
        root.append(&retry);
        Self {
            root,
            include,
            summary,
            edit,
            retry,
            html: RefCell::new(saved.into()),
            key: RefCell::new(None),
            preserved: Cell::new(draft),
            sync: Cell::new(false),
            opted_out: Cell::new(false),
            loading: Cell::new(false),
            error: RefCell::new(None),
            epoch: Cell::new(0),
            task: RefCell::new(None),
        }
    }
}
pub(super) fn cancel(e: &Editor) {
    e.signature.loading.set(false);
    e.signature
        .epoch
        .set(e.signature.epoch.get().wrapping_add(1));
    if let Some(task) = e.signature.task.borrow_mut().take() {
        task.abort();
    }
}
pub(super) fn load(e: &Rc<Editor>, force: bool) {
    if e.closing.get() || e.sent.get() {
        return;
    }
    let Ok(sender) = identity::sender(e) else {
        return;
    };
    let Some(account) = e.account().cloned() else {
        return;
    };
    let key = (account.id.clone(), sender.send_as_email.clone());
    if !force && e.signature.key.borrow().as_ref() == Some(&key) {
        return;
    }
    cancel(e);
    *e.signature.key.borrow_mut() = Some(key.clone());
    let Some(provider) = e.state.clients.borrow().get(&account.id).cloned() else {
        e.signature
            .root
            .set_visible(!e.signature.html.borrow().is_empty());
        if e.signature.html.borrow().is_empty() {
            e.signature.sync.set(true);
            e.signature.include.set_active(false);
            e.signature.sync.set(false);
        }
        return;
    };
    e.signature.root.set_visible(true);
    if !force && e.signature.preserved.replace(false) {
        e.signature
            .summary
            .set_text("Signature preserved in this draft");
        return;
    }
    e.signature.loading.set(true);
    e.signature.error.borrow_mut().take();
    e.signature.summary.set_text("Loading online signature…");
    let epoch = e.signature.epoch.get();
    let weak = Rc::downgrade(e);
    let legacy = account.signature.clone();
    let primary = account.email;
    let task = ui::async_work(
        &e.state,
        async move { Ok(provider.signature(&sender.send_as_email).await) },
        move |_, result| {
            let Some(e) = weak.upgrade().filter(|e| {
                !e.closing.get()
                    && !e.sent.get()
                    && e.signature.epoch.get() == epoch
                    && e.signature.key.borrow().as_ref() == Some(&key)
            }) else {
                return;
            };
            e.signature.task.borrow_mut().take();
            e.signature.loading.set(false);
            match result {
                Ok(signature) => {
                    let html = if signature.html.is_empty()
                        && signature.identity.eq_ignore_ascii_case(&primary)
                        && !legacy.is_empty()
                    {
                        format!("<div>{}</div>", escape(&legacy).replace('\n', "<br>"))
                    } else {
                        signature.html
                    };
                    *e.signature.html.borrow_mut() = html;
                    e.signature.sync.set(true);
                    e.signature.include.set_active(
                        !e.signature.opted_out.get() && !e.signature.html.borrow().is_empty(),
                    );
                    e.signature.sync.set(false);
                    e.signature
                        .summary
                        .set_text(if e.signature.html.borrow().is_empty() {
                            "No Gmail signature · Edit to add one"
                        } else {
                            "Online signature included below your message"
                        });
                }
                Err(error) => {
                    *e.signature.error.borrow_mut() = Some(error.to_string());
                    e.signature
                        .summary
                        .set_text("Could not load signature · Retry or switch it off");
                }
            }
        },
    );
    *e.signature.task.borrow_mut() = Some(task);
}
pub(super) fn selected(e: &Editor) -> Result<Option<String>> {
    if !e.signature.include.is_active() {
        return Ok(None);
    }
    if e.signature.loading.get() {
        return Err(Error::Invalid(
            "The online signature is still loading. Wait or switch Signature off to continue."
                .into(),
        ));
    }
    if let Some(error) = e.signature.error.borrow().as_ref() {
        return Err(Error::Invalid(format!(
            "Signature unavailable: {error}. Retry or switch Signature off to continue."
        )));
    }
    let html = e.signature.html.borrow();
    Ok((!html.trim().is_empty()).then(|| html.clone()))
}
pub(super) fn install(e: &Rc<Editor>) {
    let weak = Rc::downgrade(e);
    e.signature.include.connect_toggled(move |input| {
        if let Some(e) = weak.upgrade()
            && !e.signature.sync.get()
        {
            e.signature.opted_out.set(!input.is_active());
            changed(&e);
            if input.is_active() && e.signature.error.borrow().is_some() {
                load(&e, true);
            }
        }
    });
    let weak = Rc::downgrade(e);
    e.signature.retry.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            load(&e, true);
            changed(&e);
        }
    });
    let weak = Rc::downgrade(e);
    e.signature.edit.connect_clicked(move |_| {
        let Some(e) = weak.upgrade() else {
            return;
        };
        let Some(account) = e.account() else {
            return;
        };
        let Ok(sender) = identity::sender(&e) else {
            return;
        };
        let weak = Rc::downgrade(&e);
        crate::signatures::open(
            &e.state,
            &account.id,
            &sender.send_as_email,
            Some(Box::new(move || {
                if let Some(e) = weak.upgrade() {
                    load(&e, true);
                    changed(&e);
                }
            })),
        );
    });
    load(e, false);
}
