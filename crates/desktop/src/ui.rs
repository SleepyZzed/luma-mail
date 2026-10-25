use adw::prelude::*;
use gtk::{gio, glib};
use luma_core::{
    Error, Result,
    auth::{Authorization, ClientConfig, Credentials, SystemVault, TokenManager, Vault},
    cache::Cache,
    composer::readable,
    config::{Account, Paths, Settings},
    gmail::Gmail,
    http::Http,
    model::*,
    pagination::UnifiedPager,
    provider::MailProvider,
    theme::Palette,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct State {
    pub app: adw::Application,
    pub window: adw::ApplicationWindow,
    pub settings: RefCell<Settings>,
    pub paths: Paths,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub http: Http,
    pub cache: Arc<Mutex<Cache>>,
    pub clients: RefCell<HashMap<String, Arc<Gmail>>>,
    pub status: gtk::Label,
    pub sidebar: gtk::Box,
    pub list: gtk::ListBox,
    pub reader: gtk::Box,
    pub title: gtk::Label,
    pub more: gtk::Button,
    pub folder: RefCell<Folder>,
    pub filter: RefCell<Option<String>>,
    pub search: gtk::SearchEntry,
    pub rows: RefCell<Vec<Conversation>>,
    pub selected: RefCell<Option<ThreadKey>>,
    pub pager: Arc<tokio::sync::Mutex<Option<UnifiedPager>>>,
    pub unread: RefCell<HashMap<String, u64>>,
    undo: RefCell<Vec<(String, String, Vec<MailAction>)>>,
    undo_button: gtk::Button,
    generation: Cell<u64>,
    reading: Cell<u64>,
    pub selected_message: RefCell<Option<Message>>,
    page_request: RefCell<Option<tokio::task::AbortHandle>>,
    reader_request: RefCell<Option<tokio::task::AbortHandle>>,
    pub theme_provider: gtk::CssProvider,
    theme_monitor: RefCell<Option<gio::FileMonitor>>,
    pub last_error: RefCell<Option<String>>,
    pub tray_available: Cell<bool>,
    pub quitting: Cell<bool>,
    pub background_hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    pub tray_task: RefCell<Option<tokio::task::AbortHandle>>,
    pub poller: crate::notifications::Poller,
}
pub fn async_work<T: Send + 'static>(
    state: &Rc<State>,
    future: impl Future<Output = Result<T>> + Send + 'static,
    done: impl FnOnce(Rc<State>, T) + 'static,
) -> tokio::task::AbortHandle {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let task = state.runtime.spawn(async move {
        let _ = tx.send(future.await);
    });
    let weak = Rc::downgrade(state);
    glib::MainContext::default().spawn_local(async move {
        if let (Some(state), Ok(result)) = (weak.upgrade(), rx.await) {
            match result {
                Ok(value) => done(state, value),
                Err(e) => state.error(&e),
            }
        }
    });
    task.abort_handle()
}
impl State {
    pub fn error(&self, e: &Error) {
        self.status.set_text(&e.to_string());
        self.status.add_css_class("error");
        *self.last_error.borrow_mut() = Some(e.to_string());
    }
    pub fn info(&self, text: &str) {
        self.status.remove_css_class("error");
        self.status.set_text(text);
    }
    pub fn save(&self) {
        if let Err(e) = self.paths.save(&self.settings.borrow()) {
            self.error(&e);
        }
    }
    pub fn client_config(&self) -> Result<ClientConfig> {
        ClientConfig::load(
            &std::env::var_os("LUMA_OAUTH_CLIENT")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| self.paths.config.join("oauth-client.json")),
        )
    }
    pub fn rebuild_clients(&self) -> Result<()> {
        self.clients.borrow_mut().clear();
        if self.settings.borrow().accounts.is_empty() {
            return Ok(());
        }
        let config = self.client_config()?;
        for a in self.settings.borrow().accounts.iter().filter(|a| a.enabled) {
            let manager = Arc::new(TokenManager::new(
                a.id.clone(),
                config.clone(),
                self.http.clone(),
                Arc::new(SystemVault),
            ));
            self.clients.borrow_mut().insert(
                a.id.clone(),
                Arc::new(Gmail::new(a.id.clone(), self.http.clone(), manager)),
            );
        }
        Ok(())
    }
}
pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.);
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l
}
pub fn vbox(n: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, n)
}
pub fn margins(w: &impl IsA<gtk::Widget>, n: i32) {
    w.set_margin_top(n);
    w.set_margin_bottom(n);
    w.set_margin_start(n);
    w.set_margin_end(n);
}
pub fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}
pub fn button(icon: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tip)
        .build()
}

pub fn create(app: &adw::Application, mailto: Option<String>) {
    let setup = || -> Result<_> {
        let paths = Paths::discover()?;
        let settings = paths.load()?;
        let cache = Cache::new(
            paths.cache.join("bodies"),
            settings.cache_mib.saturating_mul(1024 * 1024),
            Duration::from_secs(settings.cache_ttl_hours.saturating_mul(3600)),
        )?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        Ok((paths, settings, cache, runtime, Http::new()?))
    };
    let (paths, settings, cache, runtime, http) = match setup() {
        Ok(v) => v,
        Err(e) => {
            let page = adw::StatusPage::builder()
                .title("Luma could not start")
                .description(e.to_string())
                .build();
            let win = adw::ApplicationWindow::builder()
                .application(app)
                .content(&page)
                .build();
            win.present();
            return;
        }
    };
    let css = gtk::CssProvider::new();
    css.load_from_string(include_str!("../../../data/style.css"));
    if let Some(d) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &d,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Luma Mail")
        .default_width(settings.window_width)
        .default_height(settings.window_height)
        .build();
    let root = vbox(0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Luma", "Online-first mail")));
    let refresh = button(
        "view-refresh-symbolic",
        "Refresh now: fetch Gmail and check for new mail (Ctrl+R)",
    );
    let prefs = button("emblem-system-symbolic", "Settings");
    header.pack_start(&refresh);
    let undo_button = button(
        "edit-undo-symbolic",
        "Undo the last confirmed mail operation",
    );
    undo_button.set_sensitive(false);
    header.pack_start(&undo_button);
    header.pack_end(&prefs);
    let quit_button = button("application-exit-symbolic", "Quit Luma completely");
    header.pack_end(&quit_button);
    root.append(&header);
    let sidebar = vbox(8);
    sidebar.add_css_class("luma-sidebar");
    margins(&sidebar, 16);
    let ss = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&sidebar)
        .build();
    ss.set_size_request(190, -1);
    let center = vbox(0);
    center.set_size_request(300, -1);
    let heading = vbox(8);
    margins(&heading, 16);
    let title = label("Unified Inbox", "title-2");
    heading.append(&title);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search Gmail · from: has:attachment …")
        .hexpand(true)
        .build();
    heading.append(&search);
    center.append(&heading);
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Multiple);
    list.add_css_class("navigation-sidebar");
    center.append(
        &gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build(),
    );
    let more = gtk::Button::with_label("Load 25 more conversations");
    margins(&more, 12);
    more.set_sensitive(false);
    center.append(&more);
    let reader = vbox(16);
    margins(&reader, 28);
    let rs = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&reader)
        .build();
    let inner = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .start_child(&center)
        .end_child(&rs)
        .position(settings.list_width)
        .resize_start_child(false)
        .shrink_start_child(false)
        .build();
    let outer = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .start_child(&ss)
        .end_child(&inner)
        .position(settings.sidebar_width)
        .resize_start_child(false)
        .shrink_start_child(false)
        .vexpand(true)
        .build();
    root.append(&outer);
    let status = label(
        "Connect a Google account to get started. Mail stays on Gmail.",
        "dim-label",
    );
    margins(&status, 10);
    status.set_wrap(true);
    root.append(&status);
    window.set_content(Some(&root));
    let s = Rc::new(State {
        app: app.clone(),
        window,
        settings: RefCell::new(settings),
        paths,
        runtime: Arc::new(runtime),
        http,
        cache: Arc::new(Mutex::new(cache)),
        clients: RefCell::new(HashMap::new()),
        status,
        sidebar,
        list,
        reader,
        title,
        more,
        search,
        folder: RefCell::new(Folder::Inbox),
        filter: RefCell::new(None),
        rows: RefCell::new(vec![]),
        unread: RefCell::new(HashMap::new()),
        undo: RefCell::new(vec![]),
        undo_button,
        selected: RefCell::new(None),
        selected_message: RefCell::new(None),
        pager: Arc::new(tokio::sync::Mutex::new(None)),
        generation: Cell::new(0),
        reading: Cell::new(0),
        page_request: RefCell::new(None),
        reader_request: RefCell::new(None),
        theme_provider: gtk::CssProvider::new(),
        theme_monitor: RefCell::new(None),
        last_error: RefCell::new(None),
        tray_available: Cell::new(false),
        quitting: Cell::new(false),
        background_hold: RefCell::new(None),
        tray_task: RefCell::new(None),
        poller: Default::default(),
    });
    if let Err(e) = s.rebuild_clients() {
        s.error(&e);
    }
    self::sidebar(&s);
    empty_reader(&s);
    apply_theme(&s);
    watch_theme(&s);
    {
        let s = s.clone();
        refresh.connect_clicked(move |_| manual_refresh(&s));
    }
    {
        let s = s.clone();
        prefs.connect_clicked(move |_| crate::settings::open(&s));
    }
    {
        let s = s.clone();
        s.clone().more.connect_clicked(move |_| load_more(&s));
    }
    {
        let s = s.clone();
        s.clone().search.connect_activate(move |_| reload(&s));
    }
    {
        let s = s.clone();
        s.clone().list.connect_row_activated(move |_, r| {
            let key = s
                .rows
                .borrow()
                .get(r.index() as usize)
                .map(|r| r.key.clone());
            if let Some(k) = key {
                open_thread(&s, k);
            }
        });
    }
    {
        let s = s.clone();
        s.clone().list.connect_row_selected(move |_, row| {
            if let Some(r) = row {
                let key = s
                    .rows
                    .borrow()
                    .get(r.index() as usize)
                    .map(|r| r.key.clone());
                if let Some(k) = key {
                    open_thread(&s, k);
                }
            }
        });
    }
    {
        let s = s.clone();
        s.clone().window.connect_close_request(move |w| {
            let mut p = s.settings.borrow_mut();
            p.window_width = w.width();
            p.window_height = w.height();
            p.sidebar_width = outer.position();
            p.list_width = inner.position();
            drop(p);
            s.save();
            if s.settings
                .borrow()
                .hides_on_close(s.tray_available.get(), s.quitting.get())
            {
                if s.background_hold.borrow().is_none() {
                    *s.background_hold.borrow_mut() = Some(s.app.hold());
                }
                w.set_visible(false);
                return glib::Propagation::Stop;
            }
            crate::tray::quit(&s);
            glib::Propagation::Stop
        });
    }
    install_shortcuts(&s);
    {
        let s = s.clone();
        quit_button.connect_clicked(move |_| crate::tray::quit(&s));
    }
    {
        let s = s.clone();
        s.clone()
            .undo_button
            .connect_clicked(move |_| undo_last(&s));
    }
    crate::notifications::start(&s);
    crate::tray::start(&s);
    s.window.present();
    if !s.clients.borrow().is_empty() {
        reload(&s);
    }
    if let Some(uri) = mailto {
        crate::compose::mailto(&s, &uri);
    }
}
fn empty_reader(s: &State) {
    clear(&s.reader);
    s.reader.append(
        &adw::StatusPage::builder()
            .icon_name("mail-unread-symbolic")
            .title("A little room to focus")
            .description("Choose a conversation. Luma only downloads the message you open.")
            .vexpand(true)
            .build(),
    );
}
pub fn sidebar(s: &Rc<State>) {
    clear(&s.sidebar);
    let compose = gtk::Button::with_label("Compose");
    compose.add_css_class("suggested-action");
    compose.add_css_class("pill");
    s.sidebar.append(&compose);
    {
        let s = s.clone();
        compose.connect_clicked(move |_| crate::compose::open(&s, None, None));
    }
    for (name, folder) in [
        ("Unified Inbox", Folder::Inbox),
        ("All Mail", Folder::AllMail),
        ("Unread", Folder::Unread),
        ("Starred", Folder::Starred),
        ("Sent", Folder::Sent),
        ("Drafts", Folder::Drafts),
        ("Archive", Folder::Archive),
        ("Trash", Folder::Trash),
    ] {
        let b = gtk::Button::with_label(name);
        b.add_css_class("flat");
        s.sidebar.append(&b);
        let s = s.clone();
        b.connect_clicked(move |_| {
            *s.folder.borrow_mut() = folder.clone();
            *s.filter.borrow_mut() = None;
            s.title.set_text(name);
            reload(&s);
        });
    }
    let heading = label("ACCOUNTS", "heading");
    heading.set_margin_top(20);
    s.sidebar.append(&heading);
    for a in s.settings.borrow().accounts.clone() {
        let b = gtk::Button::with_label(&format!(
            "{}\n{}{}",
            format_args!(
                "{} · {} unread",
                a.name,
                s.unread.borrow().get(&a.id).copied().unwrap_or(0)
            ),
            a.email,
            if a.enabled { "" } else { " · disabled" }
        ));
        b.add_css_class("flat");
        b.set_sensitive(a.enabled);
        s.sidebar.append(&b);
        let s = s.clone();
        b.connect_clicked(move |_| {
            *s.filter.borrow_mut() = Some(a.id.clone());
            *s.folder.borrow_mut() = Folder::Inbox;
            s.title.set_text(&a.name);
            reload(&s);
            load_labels(&s, &a.id);
        });
    }
    let add = gtk::Button::with_label("＋ Add Google account");
    add.add_css_class("flat");
    s.sidebar.append(&add);
    {
        let s = s.clone();
        add.connect_clicked(move |_| add_account(&s, false, None));
    }
    let setup = gtk::Button::with_label("Settings & OAuth setup");
    setup.add_css_class("flat");
    s.sidebar.append(&setup);
    {
        let s = s.clone();
        setup.connect_clicked(move |_| crate::settings::open(&s));
    }
}
fn load_labels(s: &Rc<State>, account: &str) {
    let Some(c) = s.clients.borrow().get(account).cloned() else {
        return;
    };
    let id = account.to_owned();
    async_work(s, async move { c.labels().await }, move |s, labels| {
        if s.filter.borrow().as_ref() != Some(&id) {
            return;
        }
        sidebar(&s);
        s.sidebar.append(&label("LABELS", "heading"));
        for l in labels.into_iter().filter(|l| l.kind == "user") {
            let b = gtk::Button::with_label(&l.name);
            b.add_css_class("flat");
            s.sidebar.append(&b);
            let s = s.clone();
            let id = id.clone();
            b.connect_clicked(move |_| {
                *s.filter.borrow_mut() = Some(id.clone());
                *s.folder.borrow_mut() = Folder::Label(l.id.clone());
                s.title.set_text(&l.name);
                reload(&s);
            });
        }
    });
}
pub fn reload(s: &Rc<State>) {
    reload_view(s, false);
}
pub fn manual_refresh(s: &Rc<State>) {
    reload(s);
    crate::notifications::poll_now(s, false);
}
pub fn refresh_background(s: &Rc<State>) {
    reload_view(s, true);
}
fn reload_view(s: &Rc<State>, preserve_reader: bool) {
    if let Some(t) = s.page_request.borrow_mut().take() {
        t.abort();
    }
    s.generation.set(s.generation.get() + 1);
    if !preserve_reader {
        s.reading.set(s.reading.get() + 1);
        if let Some(t) = s.reader_request.borrow_mut().take() {
            t.abort();
        }
        *s.selected.borrow_mut() = None;
        *s.selected_message.borrow_mut() = None;
    }
    s.rows.borrow_mut().clear();
    while let Some(c) = s.list.first_child() {
        s.list.remove(&c);
    }
    if !preserve_reader {
        empty_reader(s);
    }
    let filter = s.filter.borrow().clone();
    let clients: Vec<Arc<dyn MailProvider>> = s
        .clients
        .borrow()
        .iter()
        .filter(|(id, _)| filter.as_ref().is_none_or(|f| *id == f))
        .map(|(_, c)| c.clone() as Arc<dyn MailProvider>)
        .collect();
    if clients.is_empty() {
        s.info("Connect or enable a Google account in Settings.");
        s.more.set_sensitive(false);
        return;
    }
    let pager = UnifiedPager::new(clients, s.folder.borrow().clone(), s.search.text().into());
    let shared = s.pager.clone();
    let generation = s.generation.get();
    s.info("Fetching current mailbox state from Gmail…");
    s.more.set_sensitive(false);
    let handle = async_work(
        s,
        async move {
            let mut guard = shared.lock().await;
            *guard = Some(pager);
            let p = guard.as_mut().expect("set pager");
            let rows = p.next_page().await?;
            Ok((rows, p.has_more()))
        },
        move |s, (rows, more)| {
            if s.generation.get() == generation {
                display_rows(&s, rows, more);
            }
        },
    );
    *s.page_request.borrow_mut() = Some(handle);
}
fn load_more(s: &Rc<State>) {
    if s.rows.borrow().len() >= 1000 {
        s.info("1,000 conversations loaded. Narrow your search or refresh to keep memory bounded.");
        s.more.set_sensitive(false);
        return;
    }
    s.more.set_sensitive(false);
    s.info("Loading the next page…");
    let shared = s.pager.clone();
    let generation = s.generation.get();
    let handle = async_work(
        s,
        async move {
            let mut guard = shared.lock().await;
            let p = guard
                .as_mut()
                .ok_or_else(|| Error::Invalid("Refresh this mailbox first".into()))?;
            let rows = p.next_page().await?;
            Ok((rows, p.has_more()))
        },
        move |s, (rows, more)| {
            if s.generation.get() == generation {
                display_rows(&s, rows, more);
            }
        },
    );
    *s.page_request.borrow_mut() = Some(handle);
}
fn display_rows(s: &Rc<State>, rows: Vec<Conversation>, more: bool) {
    let accounts = s.settings.borrow().accounts.clone();
    for item in &rows {
        let row = vbox(if s.settings.borrow().compact { 3 } else { 7 });
        margins(&row, 14);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let sender = label(&item.sender, if item.unread { "heading" } else { "" });
        sender.set_ellipsize(gtk::pango::EllipsizeMode::End);
        sender.set_hexpand(true);
        top.append(&sender);
        if item.starred {
            top.append(&gtk::Image::from_icon_name("starred-symbolic"));
        }
        if item.attachments {
            top.append(&gtk::Image::from_icon_name("mail-attachment-symbolic"));
        }
        let date = chrono::DateTime::from_timestamp_millis(item.date)
            .map(|t| t.with_timezone(&chrono::Local).format("%d %b").to_string())
            .unwrap_or_default();
        top.append(&label(&date, "dim-label"));
        row.append(&top);
        let subject = label(
            if item.subject.is_empty() {
                "(No subject)"
            } else {
                &item.subject
            },
            if item.unread { "heading" } else { "" },
        );
        subject.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&subject);
        let snippet = label(&item.snippet, "dim-label");
        snippet.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&snippet);
        let source = accounts
            .iter()
            .find(|a| a.id == item.key.account)
            .map(|a| a.email.as_str())
            .unwrap_or("Unknown account");
        row.append(&label(
            &format!(
                "{}{} · {}",
                if item.unread { "● " } else { "" },
                source,
                item.count
            ),
            "caption",
        ));
        s.list.append(&row);
    }
    s.rows.borrow_mut().extend(rows);
    s.more.set_sensitive(more && s.rows.borrow().len() < 1000);
    s.info(&format!(
        "{} conversations loaded · Online · Bodies fetched on demand",
        s.rows.borrow().len()
    ));
    if s.rows.borrow().is_empty() {
        s.info("No conversations match this view. Try another folder or Gmail search.");
    }
}
pub fn open_thread(s: &Rc<State>, key: ThreadKey) {
    if let Some(t) = s.reader_request.borrow_mut().take() {
        t.abort();
    }
    s.reading.set(s.reading.get() + 1);
    let reading = s.reading.get();
    *s.selected.borrow_mut() = Some(key.clone());
    *s.selected_message.borrow_mut() = None;
    clear(&s.reader);
    s.reader
        .append(&label("Loading conversation metadata…", "dim-label"));
    let Some(client) = s.clients.borrow().get(&key.account).cloned() else {
        return;
    };
    let handle = async_work(
        s,
        async move { client.thread(&key.id).await },
        move |s, mut thread| {
            if s.reading.get() != reading {
                return;
            }
            thread.messages.sort_by_key(Message::date);
            clear(&s.reader);
            let title = label(
                thread
                    .messages
                    .last()
                    .map(|m| m.payload.header("Subject"))
                    .unwrap_or("Conversation"),
                "title-1",
            );
            title.set_wrap(true);
            s.reader.append(&title);
            let tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for (icon, tip, action) in [
                (
                    "mail-archive-symbolic",
                    "Archive on Gmail",
                    MailAction::Archive,
                ),
                (
                    "user-trash-symbolic",
                    "Move to Gmail Trash",
                    MailAction::Trash,
                ),
                (
                    "mail-mark-unread-symbolic",
                    "Mark unread",
                    MailAction::Unread,
                ),
                ("starred-symbolic", "Star", MailAction::Star),
                ("non-starred-symbolic", "Unstar", MailAction::Unstar),
                ("mail-inbox-symbolic", "Move to inbox", MailAction::Inbox),
                (
                    "edit-undo-symbolic",
                    "Restore from Trash",
                    MailAction::Restore,
                ),
            ] {
                let b = button(icon, tip);
                tools.append(&b);
                let s = s.clone();
                b.connect_clicked(move |_| act_selected(&s, action.clone()));
            }
            s.reader.append(&tools);
            let labels = gtk::Button::with_label("Labels…");
            s.reader.append(&labels);
            {
                let s = s.clone();
                labels.connect_clicked(move |_| crate::settings::label_dialog(&s));
            }
            let last = thread.messages.len().saturating_sub(1);
            for (index, message) in thread.messages.into_iter().enumerate() {
                let e = gtk::Expander::builder()
                    .label(format!(
                        "{}  ·  {}{}",
                        message.payload.header("From"),
                        chrono::DateTime::from_timestamp_millis(message.date())
                            .map(|t| t
                                .with_timezone(&chrono::Local)
                                .format("%d %b %Y %H:%M")
                                .to_string())
                            .unwrap_or_default(),
                        if message.has("UNREAD") {
                            " · unread"
                        } else {
                            ""
                        }
                    ))
                    .build();
                let body = vbox(10);
                margins(&body, 12);
                e.set_child(Some(&body));
                s.reader.append(&e);
                let loaded = Rc::new(Cell::new(false));
                {
                    let s = s.clone();
                    let body = body.clone();
                    let m = message.clone();
                    e.connect_expanded_notify(move |e| {
                        if e.is_expanded() && !loaded.replace(true) {
                            load_message(&s, &body, m.clone(), reading);
                        }
                    });
                }
                if index == last {
                    e.set_expanded(true);
                }
            }
        },
    );
    *s.reader_request.borrow_mut() = Some(handle);
}
fn load_message(s: &Rc<State>, body: &gtk::Box, message: Message, reading: u64) {
    let Some(key) = s.selected.borrow().clone() else {
        return;
    };
    let Some(client) = s.clients.borrow().get(&key.account).cloned() else {
        return;
    };
    let cache = s.cache.clone();
    let target = body.clone();
    target.append(&label("Fetching message…", "dim-label"));
    async_work(
        s,
        async move {
            let cached = cache
                .lock()
                .expect("cache mutex")
                .get(&key.account, &message.id)?;
            let full = if let Some(data) = cached {
                serde_json::from_slice::<Message>(&data)?
            } else {
                let full = client.message(&message.id).await?;
                // Gmail can include small MIME attachments inline. Never persist
                // attachment bytes as a side effect of caching an opened body.
                if !full
                    .payload
                    .attachments()
                    .iter()
                    .any(|p| p.body.data.is_some())
                {
                    cache.lock().expect("cache mutex").put(
                        &key.account,
                        &message.id,
                        &serde_json::to_vec(&full)?,
                    )?;
                }
                full
            };
            let read_error = client
                .act(&message.id, true, &MailAction::Read)
                .await
                .err()
                .map(|e| e.to_string());
            let text = readable(&full.payload)?;
            Ok((full, text, read_error))
        },
        move |s, (message, text, read_error)| {
            if s.reading.get() != reading {
                return;
            }
            clear(&target);
            *s.selected_message.borrow_mut() = Some(message.clone());
            let details = label(
                &format!(
                    "From: {}\nTo: {}\nCc: {}",
                    message.payload.header("From"),
                    message.payload.header("To"),
                    message.payload.header("Cc")
                ),
                "dim-label",
            );
            details.set_wrap(true);
            details.set_selectable(true);
            target.append(&details);
            let text = label(&text, "");
            text.set_wrap(true);
            text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            text.set_selectable(true);
            target.append(&text);
            for part in message.payload.attachments() {
                let b = gtk::Button::with_label(&format!(
                    "Save {} · {} bytes",
                    part.filename, part.body.size
                ));
                target.append(&b);
                let s = s.clone();
                let m = message.clone();
                let p = part.clone();
                b.connect_clicked(move |_| save_attachment(&s, &m, &p));
            }
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            for (name, mode) in [("Reply", 0), ("Reply all", 1), ("Forward", 2)] {
                let b = gtk::Button::with_label(name);
                actions.append(&b);
                let s = s.clone();
                let m = message.clone();
                b.connect_clicked(move |_| crate::compose::open(&s, Some((m.clone(), mode)), None));
            }
            target.append(&actions);
            let operations = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for (name, action) in [
                ("Mark unread", MailAction::Unread),
                ("Star", MailAction::Star),
                ("Unstar", MailAction::Unstar),
                ("Important", MailAction::Important),
                ("Not important", MailAction::Unimportant),
                ("Trash message", MailAction::Trash),
            ] {
                let b = gtk::Button::with_label(name);
                operations.append(&b);
                let s = s.clone();
                let id = message.id.clone();
                let account = s
                    .selected
                    .borrow()
                    .as_ref()
                    .map(|k| k.account.clone())
                    .unwrap_or_default();
                b.connect_clicked(move |_| {
                    perform_targets(
                        &s,
                        vec![(
                            ThreadKey {
                                account: account.clone(),
                                id: id.clone(),
                            },
                            true,
                        )],
                        action.clone(),
                    )
                });
            }
            target.append(&operations);
            if message.has("DRAFT") {
                let b = gtk::Button::with_label("Edit Gmail draft");
                target.append(&b);
                let s = s.clone();
                let id = message.id.clone();
                b.connect_clicked(move |_| crate::compose::find_draft(&s, &id));
            }
            if let Some(error) = read_error {
                s.info(&format!("Message displayed; marking read failed: {error}"));
            } else {
                s.info("Message fetched · Read status confirmed by Gmail · Remote images blocked");
            }
        },
    );
}
fn save_attachment(s: &Rc<State>, message: &Message, part: &Part) {
    let dialog = gtk::FileDialog::builder()
        .title("Save attachment")
        .initial_name(
            std::path::Path::new(&part.filename)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
        )
        .build();
    let s = s.clone();
    let m = message.clone();
    let p = part.clone();
    let key = s.selected.borrow().clone();
    glib::MainContext::default().spawn_local(async move {
        let Ok(file) = dialog.save_future(Some(&s.window)).await else {
            return;
        };
        let Some(path) = file.path() else { return };
        let Some(key) = key else { return };
        let Some(c) = s.clients.borrow().get(&key.account).cloned() else {
            return;
        };
        async_work(
            &s,
            async move {
                let data = if let Some(id) = &p.body.attachment_id {
                    c.attachment(&m.id, id).await?
                } else {
                    luma_core::composer::decode_part(&p)?
                };
                tokio::fs::write(path, data).await?;
                Ok(())
            },
            |s, ()| s.info("Attachment saved to your chosen location"),
        );
    });
}
pub fn act_selected(s: &Rc<State>, action: MailAction) {
    let mut keys: Vec<_> = s
        .list
        .selected_rows()
        .iter()
        .filter_map(|r| {
            s.rows
                .borrow()
                .get(r.index() as usize)
                .map(|r| r.key.clone())
        })
        .collect();
    if keys.is_empty()
        && let Some(k) = s.selected.borrow().clone()
    {
        keys.push(k);
    }
    if keys.is_empty() {
        return;
    }
    if action == MailAction::Trash && keys.len() > 1 {
        let d=adw::AlertDialog::builder().heading(format!("Move {} conversations to Trash?",keys.len())).body("This changes Gmail for each source account. Messages remain recoverable in Gmail Trash.").build();
        d.add_responses(&[("cancel", "Cancel"), ("trash", "Move to Trash")]);
        d.set_response_appearance("trash", adw::ResponseAppearance::Destructive);
        d.set_default_response(Some("cancel"));
        let s = s.clone();
        glib::MainContext::default().spawn_local(async move {
            if d.choose_future(Some(&s.window)).await == "trash" {
                perform_actions(&s, keys, action);
            }
        });
    } else {
        perform_actions(s, keys, action);
    }
}
fn perform_actions(s: &Rc<State>, keys: Vec<ThreadKey>, action: MailAction) {
    perform_targets(s, keys.into_iter().map(|k| (k, false)).collect(), action);
}
fn perform_targets(s: &Rc<State>, keys: Vec<(ThreadKey, bool)>, action: MailAction) {
    let jobs: Vec<_> = keys
        .into_iter()
        .filter_map(|(k, message)| {
            s.clients
                .borrow()
                .get(&k.account)
                .cloned()
                .map(|c| (k, c, message))
        })
        .collect();
    s.info("Applying changes on Gmail…");
    async_work(
        s,
        async move {
            let mut success = 0;
            let mut failures = Vec::new();
            let mut undo = Vec::new();
            for (k, c, message) in jobs {
                let before = if message {
                    c.message_metadata(&k.id).await.map(|m| vec![m])
                } else {
                    c.thread(&k.id).await.map(|t| t.messages)
                };
                let result = async {
                    let before = before?;
                    c.act(&k.id, message, &action).await?;
                    Ok::<_, Error>(before)
                }
                .await;
                match result {
                    Ok(before) => {
                        success += 1;
                        for m in before {
                            let actions = action.undo_for(&m);
                            if !actions.is_empty() {
                                undo.push((k.account.clone(), m.id, actions));
                            }
                        }
                    }
                    Err(e) => failures.push(format!("{}: {e}", k.account)),
                }
            }
            Ok((success, failures, undo))
        },
        |s, (success, failures, undo)| {
            s.undo_button.set_sensitive(!undo.is_empty());
            *s.undo.borrow_mut() = undo;
            reload(&s);
            s.info(&format!(
                "Gmail confirmed {success} operations; {} failed. {}",
                failures.len(),
                failures.first().cloned().unwrap_or_default()
            ));
        },
    );
}
fn undo_last(s: &Rc<State>) {
    let records = std::mem::take(&mut *s.undo.borrow_mut());
    s.undo_button.set_sensitive(false);
    let jobs: Vec<_> = records
        .into_iter()
        .filter_map(|(account, message, actions)| {
            s.clients
                .borrow()
                .get(&account)
                .cloned()
                .map(|c| (c, account, message, actions))
        })
        .collect();
    async_work(
        s,
        async move {
            let mut failed = vec![];
            for (c, a, m, actions) in jobs {
                let mut remaining = vec![];
                for action in actions {
                    if c.act(&m, true, &action).await.is_err() {
                        remaining.push(action);
                    }
                }
                if !remaining.is_empty() {
                    failed.push((a, m, remaining));
                }
            }
            Ok(failed)
        },
        |s, failed| {
            let count = failed.len();
            s.undo_button.set_sensitive(!failed.is_empty());
            *s.undo.borrow_mut() = failed;
            reload(&s);
            s.info(&format!(
                "Undo confirmed on Gmail; {count} messages need a retry"
            ));
        },
    );
}
pub fn add_account(s: &Rc<State>, drive: bool, expected: Option<String>) {
    let config = match s.client_config() {
        Ok(c) => c,
        Err(e) => {
            s.error(&e);
            crate::settings::open(s);
            return;
        }
    };
    let c = config.clone();
    s.info("Opening Google sign-in in your browser…");
    async_work(
        s,
        async move { Authorization::begin(&c, drive).await },
        move |s, auth| {
            if let Err(e) = gio::AppInfo::launch_default_for_uri(
                auth.url.as_str(),
                None::<&gio::AppLaunchContext>,
            ) {
                s.info(&format!("Could not launch your default browser: {e}"));
                return;
            }
            let http = s.http.clone();
            let existing = s.settings.borrow().accounts.clone();
            async_work(
                &s,
                async move {
                    let token = auth.complete(&http, &config).await?;
                    let p = Gmail::profile_with_token(&http, &token.access_token).await?;
                    if let Some(email) = expected
                        && !p.email_address.eq_ignore_ascii_case(&email)
                    {
                        return Err(Error::Invalid(
                            "Sign in with the same Google account when reconnecting".into(),
                        ));
                    }
                    let id = existing
                        .iter()
                        .find(|a| a.email.eq_ignore_ascii_case(&p.email_address))
                        .map(|a| a.id.clone())
                        .unwrap_or_else(luma_core::composer::new_id);
                    let refresh=token.refresh_token.as_ref().ok_or_else(||Error::Invalid("Google did not return a refresh token. Revoke the previous grant and reconnect.".into()))?;
                    SystemVault
                        .store(
                            &id,
                            &Credentials {
                                refresh_token: refresh.clone(),
                            },
                        )
                        .await?;
                    Ok((id, p.email_address))
                },
                move |s, (id, email)| {
                    let mut p = s.settings.borrow_mut();
                    if let Some(a) = p.accounts.iter_mut().find(|a| a.id == id) {
                        a.enabled = true;
                        a.drive_enabled = drive;
                    } else {
                        p.accounts.push(Account {
                            id: id.clone(),
                            email: email.clone(),
                            name: email.split('@').next().unwrap_or(&email).into(),
                            enabled: true,
                            notifications: true,
                            signature: String::new(),
                            drive_enabled: drive,
                        });
                    }
                    if p.default_account.is_none() {
                        p.default_account = Some(id);
                    }
                    drop(p);
                    s.save();
                    if let Err(e) = s.rebuild_clients() {
                        s.error(&e);
                        return;
                    }
                    sidebar(&s);
                    reload(&s);
                },
            );
        },
    );
}
pub fn apply_theme(s: &Rc<State>) {
    let style = adw::StyleManager::default();
    let mode = s.settings.borrow().theme.clone();
    style.set_color_scheme(match mode.as_str() {
        "dark" => adw::ColorScheme::ForceDark,
        "light" => adw::ColorScheme::ForceLight,
        _ => adw::ColorScheme::Default,
    });
    s.theme_provider.load_from_string("");
    if mode == "noctalia" {
        match std::fs::read_to_string(s.paths.config.join("noctalia.toml"))
            .map_err(Error::from)
            .and_then(|v| Palette::parse(&v))
        {
            Ok(p) => {
                style.set_color_scheme(if p.mode == "dark" {
                    adw::ColorScheme::ForceDark
                } else {
                    adw::ColorScheme::ForceLight
                });
                s.theme_provider.load_from_string(&p.css());
            }
            Err(_) => {
                s.info("Noctalia palette unavailable or invalid; following system appearance")
            }
        }
    }
    if let Some(d) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &d,
            &s.theme_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }
}
fn watch_theme(s: &Rc<State>) {
    if luma_core::config::private_dir(&s.paths.config).is_err() {
        return;
    }
    if let Ok(m) = gio::File::for_path(&s.paths.config)
        .monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)
    {
        let weak = Rc::downgrade(s);
        m.connect_changed(move |_, file, _, _| {
            if file
                .basename()
                .is_some_and(|p| p == std::path::Path::new("noctalia.toml"))
                && let Some(s) = weak.upgrade()
            {
                apply_theme(&s);
            }
        });
        *s.theme_monitor.borrow_mut() = Some(m);
    }
}
fn install_shortcuts(s: &Rc<State>) {
    for (name, accels) in [
        ("refresh", vec!["<Primary>r"]),
        ("compose", vec!["<Primary>n"]),
        ("search", vec!["<Primary>f"]),
        ("archive", vec!["<Primary>e"]),
        ("trash", vec!["Delete"]),
        ("unread", vec!["<Primary><Shift>u"]),
    ] {
        let a = gio::SimpleAction::new(name, None);
        let state = s.clone();
        a.connect_activate(move |_, _| match name {
            "refresh" => manual_refresh(&state),
            "compose" => crate::compose::open(&state, None, None),
            "search" => {
                state.search.grab_focus();
            }
            "archive" => act_selected(&state, MailAction::Archive),
            "trash" => act_selected(&state, MailAction::Trash),
            "unread" => act_selected(&state, MailAction::Unread),
            _ => (),
        });
        s.window.add_action(&a);
        s.app.set_accels_for_action(&format!("win.{name}"), &accels);
    }
}
