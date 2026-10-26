use adw::prelude::*;
use gtk::{gio, glib};
use luma_core::{
    Error, Result,
    auth::{ClientConfig, SystemVault, TokenManager},
    cache::Cache,
    composer::readable,
    config::{Paths, Settings},
    gmail::Gmail,
    http::Http,
    model::*,
    pagination::UnifiedPager,
    provider::MailProvider,
    theme::Palette,
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    future::Future,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

#[path = "ui_actions.rs"]
mod actions;
#[path = "ui_rows.rs"]
mod rows;
#[path = "ui_selection.rs"]
mod selection;

pub struct State {
    pub app: adw::Application,
    pub window: adw::ApplicationWindow,
    pub settings: RefCell<Settings>,
    pub paths: Paths,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub http: Http,
    pub cache: Arc<Mutex<Cache>>,
    pub local_compose: luma_core::local_compose::Store,
    pub clients: RefCell<HashMap<String, Arc<Gmail>>>,
    pub status: gtk::Label,
    pub sidebar: gtk::Box,
    pub sidebar_top: gtk::Box,
    pub list: gtk::ListBox,
    row_widgets: RefCell<HashMap<ThreadKey, rows::MailRow>>,
    changing_selection: Cell<bool>,
    selection_anchor: RefCell<Option<ThreadKey>>,
    selection_explicit: Cell<bool>,
    select_button: gtk::ToggleButton,
    selection_tools: gtk::Box,
    selection_count: gtk::Label,
    pub attachment_window: RefCell<glib::WeakRef<adw::Window>>,
    mutation_serial: Cell<u64>,
    row_revisions: RefCell<HashMap<ThreadKey, u64>>,
    toasts: adw::ToastOverlay,
    list_scroll: gtk::ScrolledWindow,
    refresh_button: gtk::Button,
    page_activity: gtk::ProgressBar,
    page_pulse: RefCell<Option<glib::SourceId>>,
    reader_star: RefCell<glib::WeakRef<gtk::Button>>,
    pub reader: gtk::Box,
    pub compose_stack: gtk::Stack,
    pub(crate) editor: RefCell<Option<Rc<crate::compose::Editor>>>,
    pub(crate) compose_editors: RefCell<Vec<std::rc::Weak<crate::compose::Editor>>>,
    pub reply_slot: RefCell<glib::WeakRef<gtk::Box>>,
    pub title: gtk::Label,
    pub more: gtk::Button,
    pub folder: RefCell<Folder>,
    pub filter: RefCell<Option<String>>,
    pub account_subset: RefCell<Option<HashSet<String>>>,
    scope: gtk::Label,
    pub search: gtk::SearchEntry,
    pub rows: RefCell<Vec<Conversation>>,
    pub selected: RefCell<Option<ThreadKey>>,
    pub pager: Arc<tokio::sync::Mutex<Option<UnifiedPager>>>,
    pub unread: RefCell<HashMap<String, u64>>,
    labels_cache: RefCell<HashMap<String, Vec<Label>>>,
    operation_busy: Cell<bool>,
    undo: RefCell<Vec<actions::Undo>>,
    undo_button: gtk::Button,
    generation: Cell<u64>,
    reading: Cell<u64>,
    pub selected_message: RefCell<Option<Message>>,
    page_request: RefCell<Option<tokio::task::AbortHandle>>,
    reader_request: RefCell<Option<tokio::task::AbortHandle>>,
    body_requests: RefCell<HashMap<ThreadKey, (u64, tokio::task::AbortHandle)>>,
    body_serial: Cell<u64>,
    pub theme_provider: gtk::CssProvider,
    pub style_provider: gtk::CssProvider,
    pub(crate) html_refresh: RefCell<Vec<std::rc::Weak<dyn Fn()>>>,
    pub(crate) html_refresh_pending: Cell<bool>,
    theme_monitor: RefCell<Option<gio::FileMonitor>>,
    pub last_error: RefCell<Option<String>>,
    pub tray_available: Cell<bool>,
    pub quitting: Cell<bool>,
    pub background_hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    pub tray_task: RefCell<Option<tokio::task::AbortHandle>>,
    pub poller: crate::notifications::Poller,
    pub signin_window: RefCell<Option<adw::Window>>,
    _cursor_settings: Option<gio::Settings>,
    pending_save: RefCell<Option<glib::SourceId>>,
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
    pub fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }
    pub fn save(&self) {
        if let Some(timer) = self.pending_save.borrow_mut().take() {
            timer.remove();
        }
        if let Err(e) = self.paths.save(&self.settings.borrow()) {
            self.error(&e);
        }
    }
    pub fn save_debounced(self: &Rc<Self>) {
        if let Some(timer) = self.pending_save.borrow_mut().take() {
            timer.remove();
        }
        let weak = Rc::downgrade(self);
        let timer = glib::timeout_add_local_once(Duration::from_millis(500), move || {
            if let Some(state) = weak.upgrade() {
                state.pending_save.borrow_mut().take();
                state.save();
            }
        });
        *self.pending_save.borrow_mut() = Some(timer);
    }
    pub fn client_config(&self) -> Result<ClientConfig> {
        crate::google::client_config()
    }
    pub fn rebuild_clients(&self) -> Result<()> {
        self.local_compose
            .set_accounts(self.settings.borrow().accounts.iter().map(|a| a.id.clone()));
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
    static ICONS: std::sync::Once = std::sync::Once::new();
    ICONS.call_once(|| {
        gio::resources_register_include!("luma.gresource").expect("Built-in icon resources")
    });
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_resource_path("/io/github/lumamail/Luma/icons");
    }
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
    let cursor_settings = crate::desktop::follow_cursor();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Luma Mail")
        .decorated(settings.window_chrome)
        .default_width(settings.window_width)
        .default_height(settings.window_height)
        .build();
    let root = vbox(0);
    let refresh = button(
        "view-refresh-symbolic",
        "Refresh now: fetch Gmail and check for new mail (Ctrl+R)",
    );
    let prefs = button("emblem-system-symbolic", "Settings");
    let undo_button = button(
        "edit-undo-symbolic",
        "Undo the last confirmed mail operation",
    );
    undo_button.set_sensitive(false);
    let select = gtk::ToggleButton::builder()
        .icon_name("object-select-symbolic")
        .tooltip_text("Select conversations for bulk actions")
        .build();
    let sidebar_top = vbox(8);
    sidebar_top.add_css_class("luma-sidebar");
    margins(&sidebar_top, 16);
    sidebar_top.set_margin_bottom(4);
    let sidebar = vbox(8);
    sidebar.add_css_class("luma-sidebar");
    margins(&sidebar, 16);
    sidebar.set_margin_top(4);
    let ss = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&sidebar)
        .build();
    ss.set_size_request(100, -1);
    let nav = vbox(0);
    nav.add_css_class("sidebar-shell");
    ss.set_vexpand(true);
    nav.append(&sidebar_top);
    nav.append(&ss);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    footer.add_css_class("sidebar-footer");
    let footer_name = label("Luma", "heading");
    footer_name.set_hexpand(true);
    footer.append(&footer_name);
    prefs.add_css_class("flat");
    footer.append(&prefs);
    let menu = gio::Menu::new();
    for (name, action) in [
        ("Settings", "settings"),
        ("Larger interface", "scale-up"),
        ("Smaller interface", "scale-down"),
        ("Reset interface size", "scale-reset"),
        ("Close window", "close"),
        ("Quit Luma", "quit"),
    ] {
        menu.append(Some(name), Some(&format!("win.{action}")));
    }
    let app_menu = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Luma menu · Interface size, close and quit")
        .menu_model(&menu)
        .build();
    app_menu.add_css_class("flat");
    footer.append(&app_menu);
    nav.append(&footer);
    let center = vbox(0);
    center.set_size_request(200, -1);
    let heading = vbox(8);
    margins(&heading, 16);
    let title = label("Unified Inbox", "title-2");
    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    title.set_hexpand(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_row.append(&title);
    for control in [&refresh, &undo_button] {
        control.add_css_class("flat");
        title_row.append(control);
    }
    select.add_css_class("flat");
    title_row.append(&select);
    heading.append(&title_row);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search Gmail · from: has:attachment …")
        .hexpand(true)
        .build();
    heading.append(&search);
    let scope = label("All enabled accounts", "caption");
    heading.append(&scope);
    let search_options = button("luma-filter-symbolic", "Search options and account filters");
    search_options.add_css_class("flat");
    let search_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    heading.remove(&search);
    search_bar.append(&search);
    search_bar.append(&search_options);
    heading.prepend(&search_bar);
    heading.reorder_child_after(&search_bar, Some(&title_row));
    let selection_tools = vbox(6);
    selection_tools.add_css_class("bulk-actions");
    selection_tools.set_visible(false);
    let selection_count = label("0 selected", "caption");
    selection_tools.append(&selection_count);
    let choices = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let select_all = gtk::Button::with_label("Select all loaded");
    select_all.set_tooltip_text(Some(
        "Select only the conversations already loaded here · Ctrl+A while the list is focused",
    ));
    let clear_selection = gtk::Button::with_label("Clear");
    choices.append(&select_all);
    choices.append(&clear_selection);
    selection_tools.append(&choices);
    let bulk_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    selection_tools.append(&bulk_buttons);
    heading.append(&selection_tools);
    center.append(&heading);
    let page_activity = gtk::ProgressBar::new();
    page_activity.add_css_class("mail-loading");
    page_activity.set_visible(false);
    center.append(&page_activity);
    let list = gtk::ListBox::new();
    list.set_sort_func(rows::sort);
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("navigation-sidebar");
    let list_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    center.append(&list_scroll);
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
        .shrink_start_child(true)
        .build();
    let outer = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .start_child(&nav)
        .end_child(&inner)
        .position(settings.sidebar_width)
        .resize_start_child(false)
        .shrink_start_child(true)
        .vexpand(true)
        .build();
    root.append(&outer);
    let status = label(
        "Connect a Google account to get started. Mail stays on Gmail.",
        "dim-label",
    );
    margins(&status, 10);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.add_css_class("app-status");
    root.append(&status);
    let toasts = adw::ToastOverlay::new();
    let compose_stack = gtk::Stack::new();
    compose_stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    compose_stack.set_transition_duration(160);
    compose_stack.add_named(&root, Some("mail"));
    toasts.set_child(Some(&compose_stack));
    window.set_content(Some(&toasts));
    let s = Rc::new(State {
        app: app.clone(),
        window,
        settings: RefCell::new(settings),
        local_compose: luma_core::local_compose::Store::new(&paths.config),
        paths,
        runtime: Arc::new(runtime),
        http,
        cache: Arc::new(Mutex::new(cache)),
        clients: RefCell::new(HashMap::new()),
        status,
        sidebar,
        sidebar_top,
        list,
        row_widgets: RefCell::new(HashMap::new()),
        changing_selection: Cell::new(false),
        selection_anchor: RefCell::new(None),
        selection_explicit: Cell::new(false),
        select_button: select.clone(),
        selection_tools,
        selection_count,
        compose_stack,
        editor: RefCell::new(None),
        compose_editors: RefCell::new(Vec::new()),
        reply_slot: RefCell::new(glib::WeakRef::new()),
        attachment_window: RefCell::new(glib::WeakRef::new()),
        mutation_serial: Cell::new(0),
        row_revisions: RefCell::new(HashMap::new()),
        toasts,
        list_scroll,
        refresh_button: refresh.clone(),
        page_activity,
        page_pulse: RefCell::new(None),
        reader_star: RefCell::new(glib::WeakRef::new()),
        reader,
        title,
        more,
        search,
        folder: RefCell::new(Folder::Inbox),
        filter: RefCell::new(None),
        account_subset: RefCell::new(None),
        scope,
        rows: RefCell::new(vec![]),
        unread: RefCell::new(HashMap::new()),
        labels_cache: RefCell::new(HashMap::new()),
        operation_busy: Cell::new(false),
        undo: RefCell::new(vec![]),
        undo_button,
        selected: RefCell::new(None),
        selected_message: RefCell::new(None),
        pager: Arc::new(tokio::sync::Mutex::new(None)),
        generation: Cell::new(0),
        reading: Cell::new(0),
        page_request: RefCell::new(None),
        reader_request: RefCell::new(None),
        body_requests: RefCell::new(HashMap::new()),
        body_serial: Cell::new(0),
        theme_provider: gtk::CssProvider::new(),
        style_provider: css,
        html_refresh: RefCell::new(Vec::new()),
        html_refresh_pending: Cell::new(false),
        theme_monitor: RefCell::new(None),
        last_error: RefCell::new(None),
        tray_available: Cell::new(false),
        quitting: Cell::new(false),
        background_hold: RefCell::new(None),
        tray_task: RefCell::new(None),
        poller: Default::default(),
        signin_window: RefCell::new(None),
        _cursor_settings: cursor_settings,
        pending_save: RefCell::new(None),
    });
    s.local_compose.configure(
        s.settings.borrow().local_drafts,
        s.settings.borrow().recent_recipients,
    );
    if let Err(e) = s.rebuild_clients() {
        s.error(&e);
    }
    {
        let s = s.clone();
        search_options.connect_clicked(move |_| crate::search::open(&s));
    }
    self::sidebar(&s);
    empty_reader(&s);
    apply_theme(&s);
    crate::presentation::apply(&s);
    {
        let weak = Rc::downgrade(&s);
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(s) = weak.upgrade() {
                crate::rendering::refresh_appearance(&s);
            }
        });
    }
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
        s.clone().more.connect_clicked(move |_| {
            if s.more.label().as_deref() == Some("Retry loading Gmail") {
                manual_refresh(&s);
            } else {
                load_more(&s);
            }
        });
    }
    {
        let s = s.clone();
        s.clone().search.connect_activate(move |_| reload(&s));
    }
    {
        let state = s.clone();
        select.connect_toggled(move |button| {
            if !state.changing_selection.get() {
                selection::mode(&state, button.is_active(), true);
            }
        });
    }
    selection::install(&s);
    {
        let state = s.clone();
        select_all.connect_clicked(move |_| selection::all(&state));
        let state = s.clone();
        clear_selection.connect_clicked(move |_| {
            state.changing_selection.set(true);
            state.list.unselect_all();
            state.changing_selection.set(false);
            selection::update(&state);
        });
    }
    for (icon, tip, action) in [
        (
            "luma-archive-symbolic",
            "Archive selected",
            MailAction::Archive,
        ),
        (
            "user-trash-symbolic",
            "Move selected to Trash",
            MailAction::Trash,
        ),
        (
            "mail-read-symbolic",
            "Mark selected as read",
            MailAction::Read,
        ),
        (
            "mail-unread-symbolic",
            "Mark selected as unread",
            MailAction::Unread,
        ),
        ("starred-symbolic", "Star selected", MailAction::Star),
    ] {
        let button = button(icon, tip);
        button.add_css_class("flat");
        bulk_buttons.append(&button);
        let state = s.clone();
        button.connect_clicked(move |_| act_selected(&state, action.clone()));
    }
    {
        let s = s.clone();
        s.clone().list.connect_row_activated(move |_, r| {
            if s.changing_selection.get() || s.list.selection_mode() == gtk::SelectionMode::Multiple
            {
                return;
            }
            let key = rows::key(&s, r);
            if let Some(k) = key {
                let current = s.selected.borrow().clone();
                if current.as_ref() != Some(&k)
                    || s.rows.borrow().iter().any(|r| r.key == k && r.unread)
                {
                    open_thread(&s, k);
                }
            }
        });
    }
    {
        let s = s.clone();
        s.clone().list.connect_row_selected(move |_, row| {
            if s.changing_selection.get() || s.list.selection_mode() == gtk::SelectionMode::Multiple
            {
                return;
            }
            if let Some(r) = row {
                let key = rows::key(&s, r);
                if let Some(k) = key {
                    let current = s.selected.borrow().clone();
                    if current.as_ref() != Some(&k) {
                        open_thread(&s, k);
                    }
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
        s.clone()
            .undo_button
            .connect_clicked(move |_| undo_last(&s));
    }
    crate::notifications::start(&s);
    crate::tray::start(&s);
    s.window.present();
    #[cfg(target_os = "linux")]
    if std::env::args().any(|a| a == "--render-diagnostics") {
        assert!(
            std::env::var_os("LUMA_TEST_ROOT").is_some() && s.clients.borrow().is_empty(),
            "Renderer diagnostics require a fresh isolated config"
        );
        crate::rendering::diagnostic(&s);
        return;
    }
    if std::env::args().any(|arg| arg == "--smoke-test")
        && std::env::args().any(|arg| arg == "--smoke-signin-setup")
    {
        add_account(&s, false, None);
    }
    #[cfg(all(test, target_os = "linux"))]
    if std::env::var_os("LUMA_DESKTOP_TEST_ROOT").is_some() {
        crate::desktop_test::start(&s);
    }
    if !s.clients.borrow().is_empty() {
        reload(&s);
    }
    if let Some(uri) = mailto {
        crate::compose::mailto(&s, &uri);
    }
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--design-preview")
        && std::env::var_os("LUMA_TEST_ROOT").is_some()
    {
        crate::design_preview::show(&s);
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
    clear(&s.sidebar_top);
    let brand = vbox(2);
    brand.add_css_class("sidebar-brand");
    brand.append(&label("Luma", "title-1"));
    brand.append(&label("Online-first mail", "caption"));
    s.sidebar_top.append(&brand);
    let compose = gtk::Button::new();
    compose.set_child(Some(
        &adw::ButtonContent::builder()
            .icon_name("mail-message-new-symbolic")
            .label("Compose")
            .build(),
    ));
    compose.add_css_class("suggested-action");
    compose.add_css_class("pill");
    s.sidebar_top.append(&compose);
    {
        let s = s.clone();
        compose.connect_clicked(move |_| crate::compose::open(&s, None, None));
    }
    let local = gtk::Button::with_label("Recover old local drafts");
    local.set_visible(s.local_compose.has_drafts());
    local.add_css_class("flat");
    local.set_tooltip_text(Some("Recover unsent work saved on this device"));
    s.sidebar.append(&local);
    {
        let state = s.clone();
        local.connect_clicked(move |_| crate::compose::local_drafts(&state));
    }
    for (icon, name, folder) in [
        ("luma-inbox-symbolic", "Unified Inbox", Folder::Inbox),
        ("mail-unread-symbolic", "All Mail", Folder::AllMail),
        ("mail-mark-important-symbolic", "Unread", Folder::Unread),
        ("starred-symbolic", "Starred", Folder::Starred),
        ("mail-send-symbolic", "Sent", Folder::Sent),
        ("document-edit-symbolic", "Drafts", Folder::Drafts),
        ("luma-archive-symbolic", "Archive", Folder::Archive),
        ("mail-mark-junk-symbolic", "Spam", Folder::Spam),
        ("user-trash-symbolic", "Trash", Folder::Trash),
    ] {
        let b = gtk::Button::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        content.append(&gtk::Image::from_icon_name(icon));
        let name_label = label(name, "");
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name_label.set_hexpand(true);
        content.append(&name_label);
        b.set_child(Some(&content));
        if *s.folder.borrow() == folder && s.filter.borrow().is_none() {
            b.add_css_class("active-folder");
        }
        b.add_css_class("flat");
        s.sidebar.append(&b);
        let s = s.clone();
        b.connect_clicked(move |_| {
            *s.folder.borrow_mut() = folder.clone();
            *s.filter.borrow_mut() = None;
            s.title.set_text(name);
            sidebar(&s);
            reload(&s);
        });
    }
    let heading = label("ACCOUNTS", "heading");
    heading.set_margin_top(20);
    s.sidebar.append(&heading);
    for a in s.settings.borrow().accounts.clone() {
        let b = gtk::Button::new();
        let account = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        account.append(&rows::avatar(&a.email, 30));
        let text = vbox(3);
        text.set_hexpand(true);
        let name = label(&a.name, "account-name");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&name);
        let email = label(&a.email, "account-caption");
        email.set_ellipsize(gtk::pango::EllipsizeMode::End);
        email.set_max_width_chars(20);
        text.append(&email);
        account.append(&text);
        let unread = s.unread.borrow().get(&a.id).copied().unwrap_or(0);
        if unread > 0 {
            let count = label(
                &if unread > 999 {
                    "999+".into()
                } else {
                    unread.to_string()
                },
                "thread-count",
            );
            count.set_tooltip_text(Some(&format!("{unread} unread messages")));
            account.append(&count);
        }
        b.set_child(Some(&account));
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
    let add = gtk::Button::with_label("Sign in with Google");
    add.add_css_class("flat");
    s.sidebar.append(&add);
    {
        let s = s.clone();
        add.connect_clicked(move |_| add_account(&s, false, None));
    }
    if let Some(id) = s.filter.borrow().clone()
        && let Some(labels) = s.labels_cache.borrow().get(&id).cloned()
    {
        s.sidebar.append(&label("LABELS", "heading"));
        for item in labels.into_iter().filter(|label| label.kind == "user") {
            let button = gtk::Button::with_label(&item.name);
            button.add_css_class("flat");
            s.sidebar.append(&button);
            let s = s.clone();
            let id = id.clone();
            button.connect_clicked(move |_| {
                *s.filter.borrow_mut() = Some(id.clone());
                *s.folder.borrow_mut() = Folder::Label(item.id.clone());
                s.title.set_text(&item.name);
                reload(&s);
            });
        }
    }
}
pub fn load_labels(s: &Rc<State>, account: &str) {
    let Some(c) = s.clients.borrow().get(account).cloned() else {
        return;
    };
    let id = account.to_owned();
    async_work(s, async move { c.labels().await }, move |s, labels| {
        s.labels_cache.borrow_mut().insert(id.clone(), labels);
        if s.filter.borrow().as_ref() == Some(&id) {
            sidebar(&s);
        }
    });
}
pub fn refresh_density(s: &Rc<State>) {
    rows::restyle(s);
}
pub fn reload(s: &Rc<State>) {
    reload_view(s, false);
}
pub fn manual_refresh(s: &Rc<State>) {
    if s.page_request
        .borrow()
        .as_ref()
        .is_some_and(|task| !task.is_finished())
    {
        s.info("Refresh is already running · Your current view is kept");
        return;
    }
    reload_view(s, true);
    if let Some(account) = s.filter.borrow().clone() {
        load_labels(s, &account);
    }
    crate::notifications::poll_now(s, false);
}
pub fn refresh_background(s: &Rc<State>) {
    if s.operation_busy.get()
        || s.rows.borrow().len() > 25
        || s.list.selected_rows().len() > 1
        || s.list_scroll.vadjustment().value() > 1.
    {
        s.refresh_button.add_css_class("suggested-action");
        s.refresh_button
            .set_tooltip_text(Some("Mailbox updates available · Refresh now (Ctrl+R)"));
        s.info("Mailbox updates are available. Refresh when ready; your current position and selection are kept.");
        return;
    }
    reload_view(s, true);
}
pub fn mark_updates(s: &State) {
    s.refresh_button.add_css_class("suggested-action");
    s.refresh_button
        .set_tooltip_text(Some("Mailbox updates available · Refresh now (Ctrl+R)"));
}
/// Fetch only changed conversation metadata. Existing rows, pagination, reader
/// bodies, selection and a scrolled viewport survive incoming mail.
pub fn receive_messages(s: &Rc<State>, messages: Vec<(String, Message)>) {
    use futures_util::{StreamExt, stream};
    if !s.search.text().trim().is_empty() {
        mark_updates(s);
        return;
    }
    let generation = s.generation.get();
    let revision = s.mutation_serial.get();
    let mut seen = HashSet::new();
    let jobs: Vec<_> = messages
        .into_iter()
        .filter_map(|(account, message)| {
            if s.filter.borrow().as_ref().is_some_and(|id| id != &account)
                || (s.filter.borrow().is_none()
                    && s.account_subset
                        .borrow()
                        .as_ref()
                        .is_some_and(|ids| !ids.contains(&account)))
                || !seen.insert((account.clone(), message.thread_id.clone()))
            {
                return None;
            }
            s.clients
                .borrow()
                .get(&account)
                .cloned()
                .map(|client| (account, message.thread_id, client))
        })
        .collect();
    if jobs.is_empty() {
        return;
    }
    async_work(
        s,
        async move {
            Ok(stream::iter(jobs)
                .map(|(account, id, client)| async move {
                    client
                        .thread_metadata(&id)
                        .await
                        .map(|thread| (account, thread))
                })
                .buffer_unordered(4)
                .collect::<Vec<_>>()
                .await)
        },
        move |s, results| {
            if s.generation.get() != generation {
                return;
            }
            let updates = results
                .into_iter()
                .filter_map(|result| match result {
                    Ok((account, thread)) => {
                        let row = Conversation::from_thread(&account, &thread)?;
                        if s.row_revisions
                            .borrow()
                            .get(&row.key)
                            .is_some_and(|v| *v > revision)
                        {
                            return None;
                        }
                        let visible = s.folder.borrow().matches(&thread);
                        Some(rows::Update {
                            key: row.key.clone(),
                            row: Some(row),
                            visible: Some(visible),
                            selected_labels: None,
                        })
                    }
                    Err(_) => {
                        mark_updates(&s);
                        None
                    }
                })
                .collect();
            rows::incoming(&s, updates);
        },
    );
}
fn reload_view(s: &Rc<State>, preserve_reader: bool) {
    if preserve_reader && s.operation_busy.get() {
        s.info("Gmail action in progress · Refresh again when it finishes");
        return;
    }
    s.refresh_button.remove_css_class("suggested-action");
    s.refresh_button.set_tooltip_text(Some(
        "Refresh now: fetch Gmail and check for new mail (Ctrl+R)",
    ));
    let selection: HashSet<_> = if preserve_reader {
        s.list
            .selected_rows()
            .iter()
            .filter_map(|row| rows::key(s, row))
            .collect()
    } else {
        HashSet::new()
    };
    if let Some(t) = s.page_request.borrow_mut().take() {
        t.abort();
    }
    page_loading(s, false);
    s.generation.set(s.generation.get() + 1);
    if !preserve_reader {
        crate::compose::park_inline(s);
        cancel_body_requests(s);
        s.reading.set(s.reading.get() + 1);
        if let Some(t) = s.reader_request.borrow_mut().take() {
            t.abort();
        }
        *s.selected.borrow_mut() = None;
        *s.selected_message.borrow_mut() = None;
    }
    if !preserve_reader {
        rows::clear_rows(s);
        empty_reader(s);
    }
    let filter = s.filter.borrow().clone();
    let clients: Vec<Arc<dyn MailProvider>> = s
        .clients
        .borrow()
        .iter()
        .filter(|(id, _)| filter.as_ref().is_none_or(|f| *id == f))
        .filter(|(id, _)| {
            filter.is_some()
                || s.account_subset
                    .borrow()
                    .as_ref()
                    .is_none_or(|set| set.contains(*id))
        })
        .map(|(_, c)| c.clone() as Arc<dyn MailProvider>)
        .collect();
    s.scope.set_text(&format!(
        "{} enabled account{} · server-side search",
        clients.len(),
        if clients.len() == 1 { "" } else { "s" }
    ));
    if clients.is_empty() {
        s.info("Connect or enable a Google account in Settings.");
        s.more.set_sensitive(false);
        return;
    }
    let pager = UnifiedPager::new(clients, s.folder.borrow().clone(), s.search.text().into());
    let shared = s.pager.clone();
    let generation = s.generation.get();
    let scroll = s.list_scroll.vadjustment().value();
    let revision = s.mutation_serial.get();
    s.info("Fetching current mailbox state from Gmail…");
    page_loading(s, true);
    s.more.set_sensitive(false);
    let handle = async_work(
        s,
        async move {
            Ok(async {
                let mut guard = shared.lock().await;
                *guard = Some(pager);
                let p = guard.as_mut().expect("set pager");
                let rows = p.next_page().await?;
                Ok::<_, Error>((rows, p.has_more()))
            }
            .await)
        },
        move |s, result| {
            if s.generation.get() == generation {
                page_loading(&s, false);
                match result {
                    Ok((rows, more)) => {
                        let rows = rows::merge_read(&s, rows, revision, true);
                        let selection: HashSet<_> = if preserve_reader {
                            s.list
                                .selected_rows()
                                .iter()
                                .filter_map(|row| rows::key(&s, row))
                                .collect()
                        } else {
                            selection.clone()
                        };
                        rows::reconcile(&s, rows, more);
                        s.changing_selection.set(true);
                        for key in &selection {
                            if let Some(row) = s.row_widgets.borrow().get(key) {
                                s.list.select_row(Some(&row.row));
                            }
                        }
                        s.changing_selection.set(false);
                        if preserve_reader {
                            let weak = Rc::downgrade(&s);
                            glib::idle_add_local_once(move || {
                                if let Some(s) = weak.upgrade() {
                                    s.list_scroll.vadjustment().set_value(scroll);
                                }
                            });
                        }
                    }
                    Err(error) => {
                        s.more.set_label("Retry loading Gmail");
                        s.more.set_sensitive(true);
                        s.error(&error);
                    }
                }
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
    page_loading(s, true);
    let shared = s.pager.clone();
    let generation = s.generation.get();
    let revision = s.mutation_serial.get();
    let handle = async_work(
        s,
        async move {
            Ok(async {
                let mut guard = shared.lock().await;
                let p = guard
                    .as_mut()
                    .ok_or_else(|| Error::Invalid("Refresh this mailbox first".into()))?;
                let rows = p.next_page().await?;
                Ok::<_, Error>((rows, p.has_more()))
            }
            .await)
        },
        move |s, result| {
            if s.generation.get() == generation {
                page_loading(&s, false);
                match result {
                    Ok((rows, more)) => {
                        let rows = rows::merge_read(&s, rows, revision, false);
                        display_rows(&s, rows, more);
                    }
                    Err(error) => {
                        s.more.set_label("Retry loading more");
                        s.more.set_sensitive(true);
                        s.error(&error);
                    }
                }
            }
        },
    );
    *s.page_request.borrow_mut() = Some(handle);
}
pub(crate) fn display_rows(s: &Rc<State>, items: Vec<Conversation>, more: bool) {
    rows::display(s, items, more);
}
fn page_loading(s: &Rc<State>, active: bool) {
    if let Some(timer) = s.page_pulse.borrow_mut().take() {
        timer.remove();
    }
    s.page_activity.set_visible(active);
    if !active {
        return;
    }
    s.more.set_label("Loading conversations…");
    if gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations()) {
        let weak = Rc::downgrade(s);
        let timer = glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(s) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            s.page_activity.pulse();
            glib::ControlFlow::Continue
        });
        *s.page_pulse.borrow_mut() = Some(timer);
    } else {
        s.page_activity.set_fraction(0.5);
    }
}
fn loading_placeholder(body: &gtk::Box, text: &str) {
    let loading = vbox(12);
    loading.add_css_class("loading-placeholder");
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let spinner = gtk::Spinner::new();
    spinner.start();
    line.append(&spinner);
    line.append(&label(text, "caption"));
    loading.append(&line);
    for width in [240, 360, 190] {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("skeleton-line");
        bar.set_size_request(width, 12);
        bar.set_halign(gtk::Align::Start);
        loading.append(&bar);
    }
    body.append(&loading);
}
pub(super) fn update_reader_star(s: &State) {
    let selected = s.selected.borrow();
    let starred = s
        .rows
        .borrow()
        .iter()
        .find(|row| Some(&row.key) == selected.as_ref())
        .is_some_and(|row| row.starred);
    if let Some(button) = s.reader_star.borrow().upgrade() {
        button.set_icon_name(if starred {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        });
        button.set_tooltip_text(Some(if starred {
            "Remove star"
        } else {
            "Star conversation"
        }));
    }
}
pub fn open_thread(s: &Rc<State>, key: ThreadKey) {
    if s.list.selection_mode() == gtk::SelectionMode::Multiple {
        selection::mode(s, false, false);
    }
    s.changing_selection.set(true);
    s.list.unselect_all();
    if let Some(row) = s.row_widgets.borrow().get(&key) {
        s.list.select_row(Some(&row.row));
    }
    s.changing_selection.set(false);
    *s.selection_anchor.borrow_mut() = Some(key.clone());
    #[cfg(debug_assertions)]
    if s.clients.borrow().is_empty() && std::env::args().any(|a| a == "--design-preview") {
        *s.selected.borrow_mut() = Some(key);
        selection::update(s);
        s.info("Synthetic preview · No remote mailbox connected");
        return;
    }
    crate::compose::park_inline(s);
    cancel_body_requests(s);
    if let Some(t) = s.reader_request.borrow_mut().take() {
        t.abort();
    }
    s.reading.set(s.reading.get() + 1);
    let reading = s.reading.get();
    *s.selected.borrow_mut() = Some(key.clone());
    *s.selected_message.borrow_mut() = None;
    clear(&s.reader);
    loading_placeholder(&s.reader, "Opening conversation…");
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
                "reader-subject",
            );
            title.set_wrap(true);
            s.reader.append(&title);
            let tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            tools.add_css_class("reader-toolbar");
            let starred = thread.messages.iter().any(|m| m.has("STARRED"));
            let trash = thread.messages.iter().any(|m| m.has("TRASH"));
            for (icon, tip, action) in [
                (
                    "luma-archive-symbolic",
                    "Archive conversation",
                    MailAction::Archive,
                ),
                (
                    if trash {
                        "edit-undo-symbolic"
                    } else {
                        "user-trash-symbolic"
                    },
                    if trash {
                        "Restore from Trash"
                    } else {
                        "Move to Trash"
                    },
                    if trash {
                        MailAction::Restore
                    } else {
                        MailAction::Trash
                    },
                ),
                ("mail-unread-symbolic", "Mark unread", MailAction::Unread),
                (
                    if starred {
                        "starred-symbolic"
                    } else {
                        "non-starred-symbolic"
                    },
                    if starred {
                        "Remove star"
                    } else {
                        "Star conversation"
                    },
                    if starred {
                        MailAction::Unstar
                    } else {
                        MailAction::Star
                    },
                ),
                ("luma-inbox-symbolic", "Move to Inbox", MailAction::Inbox),
            ] {
                let b = button(icon, tip);
                b.add_css_class("flat");
                tools.append(&b);
                let s = s.clone();
                if matches!(action, MailAction::Star | MailAction::Unstar) {
                    s.reader_star.borrow_mut().set(Some(&b));
                    b.connect_clicked(move |_| {
                        let key = s.selected.borrow().clone();
                        let starred = s
                            .rows
                            .borrow()
                            .iter()
                            .find(|row| Some(&row.key) == key.as_ref())
                            .is_some_and(|row| row.starred);
                        act_selected(
                            &s,
                            if starred {
                                MailAction::Unstar
                            } else {
                                MailAction::Star
                            },
                        );
                    });
                } else {
                    b.connect_clicked(move |_| act_selected(&s, action.clone()));
                }
            }
            let labels = button("luma-tag-symbolic", "Manage labels");
            labels.add_css_class("flat");
            labels.add_css_class("selected-label-action");
            tools.append(&labels);
            let reply = button("mail-reply-sender-symbolic", "Reply inline");
            reply.add_css_class("flat");
            reply.add_css_class("single-message-action");
            tools.append(&reply);
            {
                let state = s.clone();
                reply.connect_clicked(move |_| {
                    let message = state.selected_message.borrow().clone();
                    if let Some(message) = message {
                        crate::compose::open(&state, Some((message, 0)), None);
                    } else {
                        state.info("Open a message before replying.");
                    }
                });
            }
            s.reader.append(&tools);
            {
                let s = s.clone();
                labels.connect_clicked(move |_| crate::settings::label_dialog(&s));
            }
            let last = thread.messages.len().saturating_sub(1);
            for (index, message) in thread.messages.into_iter().enumerate() {
                let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                heading.append(&rows::avatar(message.payload.header("From"), 38));
                let who = vbox(3);
                who.set_hexpand(true);
                who.append(&label(
                    &rows::name(message.payload.header("From")),
                    "heading",
                ));
                let recipients = label(&format!("to {}", message.payload.header("To")), "caption");
                recipients.set_ellipsize(gtk::pango::EllipsizeMode::End);
                recipients.set_max_width_chars(36);
                who.append(&recipients);
                heading.append(&who);
                if message.has("UNREAD") {
                    let unread = label("●", "unread-dot");
                    unread.set_widget_name(&format!("unread-{}", message.id));
                    unread.set_tooltip_text(Some("Unread reply"));
                    unread.update_property(&[gtk::accessible::Property::Label("Unread message")]);
                    heading.append(&unread);
                }
                let date = chrono::DateTime::from_timestamp_millis(message.date())
                    .map(|t| {
                        t.with_timezone(&chrono::Local)
                            .format("%d %b · %H:%M")
                            .to_string()
                    })
                    .unwrap_or_default();
                heading.append(&label(&date, "caption"));
                if let Some(key) = s.selected.borrow().as_ref() {
                    heading.append(&crate::sender_controls::menu(&s, &message, &key.account));
                }
                let e = gtk::Expander::new(None);
                e.set_label_widget(Some(&heading));
                let body = vbox(10);
                margins(&body, 12);
                e.set_child(Some(&body));
                s.reader.append(&e);
                {
                    let s = s.clone();
                    let body = body.clone();
                    let m = message.clone();
                    let request_key = ThreadKey {
                        account: s
                            .selected
                            .borrow()
                            .as_ref()
                            .map(|key| key.account.clone())
                            .unwrap_or_default(),
                        id: m.id.clone(),
                    };
                    e.connect_expanded_notify(move |e| {
                        if e.is_expanded() && !s.body_requests.borrow().contains_key(&request_key) {
                            load_message(&s, &body, m.clone(), reading);
                        } else if !e.is_expanded() {
                            crate::compose::park_inline(&s);
                            if let Some((_, request)) =
                                s.body_requests.borrow_mut().remove(&request_key)
                            {
                                request.abort();
                            }
                            clear(&body);
                            if s.selected_message
                                .borrow()
                                .as_ref()
                                .is_some_and(|message| message.id == m.id)
                            {
                                s.selected_message.borrow_mut().take();
                            }
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
fn cancel_body_requests(s: &State) {
    for (_, (_, request)) in s.body_requests.borrow_mut().drain() {
        request.abort();
    }
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
    clear(&target);
    loading_placeholder(&target, "Opening message…");
    let retry_message = message.clone();
    let request_key = ThreadKey {
        account: key.account.clone(),
        id: message.id.clone(),
    };
    let completed_key = request_key.clone();
    let serial = s.body_serial.get().wrapping_add(1);
    s.body_serial.set(serial);
    let handle = async_work(
        s,
        async move {
            Ok(async {
                let cached = cache
                    .lock()
                    .expect("cache mutex")
                    .get(&key.account, &message.id)?;
                let full = if let Some(data) = cached {
                    let mut full = serde_json::from_slice::<Message>(&data)?;
                    full.label_ids = message.label_ids.clone();
                    full
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
                let text = readable(&full.payload)?;
                Ok::<_, Error>((full, text))
            }
            .await)
        },
        move |s, result| {
            if s.reading.get() != reading
                || s.body_requests
                    .borrow()
                    .get(&completed_key)
                    .is_none_or(|(epoch, _)| *epoch != serial)
            {
                return;
            }
            let (message, text) = match result {
                Ok(value) => value,
                Err(error) => {
                    s.body_requests.borrow_mut().remove(&completed_key);
                    clear(&target);
                    let text = label(&error.to_string(), "error");
                    text.set_wrap(true);
                    target.append(&text);
                    let retry = gtk::Button::with_label("Retry opening message");
                    target.append(&retry);
                    let state = s.clone();
                    let body = target.clone();
                    retry.connect_clicked(move |_| {
                        if state.reading.get() == reading
                            && !state.body_requests.borrow().contains_key(&completed_key)
                        {
                            load_message(&state, &body, retry_message.clone(), reading);
                        }
                    });
                    return;
                }
            };
            clear(&target);
            *s.selected_message.borrow_mut() = Some(message.clone());
            s.reply_slot.borrow_mut().set(Some(&target));
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
            let recipients = gtk::Expander::builder()
                .label("Message details")
                .child(&details)
                .build();
            recipients.add_css_class("message-details");
            target.append(&recipients);
            if let Some(strip) =
                crate::attachments::remote_strip(&s, &message, &completed_key.account)
            {
                target.append(&strip);
            }
            target.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            crate::rendering::append(&s, &target, &message, &text, &completed_key.account);
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            actions.add_css_class("message-replies");
            actions.add_css_class("single-message-action");
            for (name, mode) in [("Reply", 0), ("Reply all", 1), ("Forward", 2)] {
                let b = gtk::Button::new();
                b.add_css_class("flat");
                b.set_child(Some(
                    &adw::ButtonContent::builder()
                        .icon_name(if mode == 2 {
                            "mail-forward-symbolic"
                        } else {
                            "mail-reply-sender-symbolic"
                        })
                        .label(name)
                        .build(),
                ));
                actions.append(&b);
                let s = s.clone();
                let m = message.clone();
                let slot = target.downgrade();
                b.connect_clicked(move |_| {
                    s.reply_slot.borrow_mut().set(slot.upgrade().as_ref());
                    crate::compose::open(&s, Some((m.clone(), mode)), None);
                });
            }
            target.append(&actions);
            if message.has("DRAFT") {
                let b = gtk::Button::with_label("Edit Gmail draft");
                b.add_css_class("single-message-action");
                target.append(&b);
                let s = s.clone();
                let id = message.id.clone();
                b.connect_clicked(move |_| {
                    if single_message_actions(&s) {
                        crate::compose::find_draft(&s, &id);
                    }
                });
            }
            selection::update(&s);
            crate::presentation::enter(&s, &target);
            s.info("Message opened");
            if message.has("UNREAD") {
                mark_opened_read(&s, &completed_key.account, &message);
            }
        },
    );
    s.body_requests
        .borrow_mut()
        .insert(request_key, (serial, handle));
}
fn mark_opened_read(s: &Rc<State>, account: &str, message: &Message) {
    let Some(client) = s.clients.borrow().get(account).cloned() else {
        return;
    };
    let key = ThreadKey {
        account: account.into(),
        id: message.thread_id.clone(),
    };
    let id = message.id.clone();
    let folder = s.folder.borrow().clone();
    let search = s.search.text().to_string();
    let generation = s.generation.get();
    let revision = s.mutation_serial.get();
    let attached = s
        .rows
        .borrow()
        .iter()
        .find(|r| r.key == key)
        .is_some_and(|r| r.attachments);
    async_work(
        s,
        async move {
            Ok(async {
                client.act(&id, true, &MailAction::Read).await?;
                let thread = client.thread_metadata(&key.id).await?;
                let mut row = Conversation::from_thread(&key.account, &thread);
                if let Some(row) = row.as_mut() {
                    row.attachments = attached;
                }
                let visible = client.matches_view(&thread, &folder, &search).await?;
                let labels = thread
                    .messages
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| (m.id.clone(), m.label_ids.clone()));
                Ok::<_, Error>(rows::Update {
                    key,
                    row,
                    visible,
                    selected_labels: labels,
                })
            }
            .await)
        },
        move |s, result| match result {
            Ok(update)
                if s.generation.get() == generation && s.mutation_serial.get() == revision =>
            {
                rows::apply(&s, update)
            }
            Err(error) => s.info(&format!("Message displayed; marking read failed: {error}")),
            _ => (),
        },
    );
}
pub(super) fn hide_unread_marker(s: &State, id: &str) {
    fn visit(widget: &gtk::Widget, name: &str) {
        if widget.widget_name() == name {
            widget.set_visible(false);
            return;
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            visit(&widget, name);
        }
    }
    visit(s.reader.upcast_ref(), &format!("unread-{id}"));
}
pub(super) fn new_reply(s: &Rc<State>, key: &ThreadKey) {
    let mut child = s.reader.first_child();
    while let Some(widget) = child {
        if widget.widget_name() == "new-reply" {
            return;
        }
        child = widget.next_sibling();
    }
    let button = gtk::Button::builder()
        .label("New reply · Open")
        .halign(gtk::Align::Start)
        .build();
    button.set_widget_name("new-reply");
    button.add_css_class("suggested-action");
    let state = s.clone();
    let key = key.clone();
    button.connect_clicked(move |_| open_thread(&state, key.clone()));
    s.reader.prepend(&button);
}
pub fn act_selected(s: &Rc<State>, action: MailAction) {
    if s.operation_busy.get() {
        s.info("Wait for the current Gmail operation to finish.");
        return;
    }
    let mut keys = selection::keys(s);
    if keys.is_empty()
        && s.list.selection_mode() != gtk::SelectionMode::Multiple
        && let Some(k) = s.selected.borrow().clone()
    {
        keys.push(k);
    }
    dispatch_selection(s, keys, action);
}
fn dispatch_selection(s: &Rc<State>, keys: Vec<ThreadKey>, action: MailAction) {
    if keys.is_empty() {
        s.info("Select conversations first.");
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
#[cfg(all(test, target_os = "linux"))]
pub fn verify_view_behaviour(s: &Rc<State>) {
    assert_eq!(s.list.selection_mode(), gtk::SelectionMode::Single);
    s.list.set_selection_mode(gtk::SelectionMode::Multiple);
    let rows = (0..26)
        .map(|i| Conversation {
            key: ThreadKey {
                account: "fixture".into(),
                id: format!("thread-{i}"),
            },
            subject: "Synthetic acceptance fixture".into(),
            sender: "Test".into(),
            snippet: String::new(),
            date: i,
            unread: false,
            starred: false,
            attachments: false,
            count: 1,
        })
        .collect();
    display_rows(s, rows, true);
    s.list.select_row(s.list.row_at_index(0).as_ref());
    s.list.select_row(s.list.row_at_index(1).as_ref());
    assert_eq!(s.list.selected_rows().len(), 2);
    refresh_background(s);
    assert_eq!(
        s.rows.borrow().len(),
        26,
        "background refresh cannot discard loaded pages"
    );
    assert_eq!(
        s.list.selected_rows().len(),
        2,
        "background refresh cannot discard a bulk selection"
    );
    assert!(s.refresh_button.has_css_class("suggested-action"));
    let before = s.rows.borrow()[1].clone();
    let unaffected = s.list.row_at_index(0).unwrap();
    let changed = s.list.row_at_index(1).unwrap();
    let mut starred = before.clone();
    starred.starred = true;
    let start = s.mutation_serial.get();
    rows::apply(
        s,
        rows::Update {
            key: before.key.clone(),
            row: Some(starred),
            visible: Some(true),
            selected_labels: None,
        },
    );
    assert_eq!(
        s.list.row_at_index(0).unwrap(),
        unaffected,
        "unaffected widgets must stay in place"
    );
    assert_eq!(
        s.list.row_at_index(1).unwrap(),
        changed,
        "star updates retain the selected row widget"
    );
    assert_eq!(s.list.selected_rows().len(), 2);
    assert!(rows::merge_read(s, vec![before.clone()], start, false)[0].starred);
    rows::apply(
        s,
        rows::Update {
            key: before.key.clone(),
            row: None,
            visible: Some(false),
            selected_labels: None,
        },
    );
    assert!(
        rows::merge_read(s, vec![before.clone()], start, false).is_empty(),
        "late pages cannot resurrect archived rows"
    );
    rows::apply(
        s,
        rows::Update {
            key: before.key.clone(),
            row: Some(before),
            visible: Some(true),
            selected_labels: None,
        },
    );
    assert_eq!(
        s.rows.borrow().len(),
        26,
        "Undo restores only the removed row"
    );
    let mut reply = s.rows.borrow()[3].clone();
    let original_widget = s.row_widgets.borrow().get(&reply.key).unwrap().row.clone();
    *s.selected.borrow_mut() = Some(reply.key.clone());
    let reading = s.reading.get();
    reply.count += 1;
    reply.date = 10000;
    reply.unread = true;
    rows::incoming(
        s,
        vec![rows::Update {
            key: reply.key.clone(),
            row: Some(reply.clone()),
            visible: Some(true),
            selected_labels: None,
        }],
    );
    assert_eq!(
        s.rows.borrow().len(),
        26,
        "a reply updates one thread without duplicating or replacing pages"
    );
    assert!(
        s.rows.borrow()[0].unread,
        "new reply restores unread marker on an already selected thread"
    );
    assert_eq!(s.rows.borrow()[0].count, reply.count);
    assert_eq!(
        s.row_widgets.borrow().get(&reply.key).unwrap().row,
        original_widget
    );
    assert_eq!(
        s.reading.get(),
        reading,
        "incoming reply does not reload the reader"
    );
    assert_eq!(s.selected.borrow().as_ref(), Some(&reply.key));
    assert_eq!(s.reader.first_child().unwrap().widget_name(), "new-reply");
    rows::clear_rows(s);
    s.selected.borrow_mut().take();
    empty_reader(s);
    s.refresh_button.remove_css_class("suggested-action");
    let label: Label = serde_json::from_value(
        serde_json::json!({"id":"Label_test","name":"Fixture label","type":"user"}),
    )
    .unwrap();
    s.labels_cache
        .borrow_mut()
        .insert("fixture".into(), vec![label]);
    *s.filter.borrow_mut() = Some("fixture".into());
    for _ in 0..2 {
        sidebar(s);
        let mut child = s.sidebar.first_child();
        let mut found = false;
        while let Some(widget) = child {
            found |= widget
                .downcast_ref::<gtk::Button>()
                .is_some_and(|button| button.label().as_deref() == Some("Fixture label"));
            child = widget.next_sibling();
        }
        assert!(found, "unread/sidebar updates must retain label navigation");
    }
    s.filter.borrow_mut().take();
    s.labels_cache.borrow_mut().clear();
    sidebar(s);
    s.list.set_selection_mode(gtk::SelectionMode::Single);
    crate::compose::verify_formatting();
}
fn perform_actions(s: &Rc<State>, keys: Vec<ThreadKey>, action: MailAction) {
    perform_targets(s, keys.into_iter().map(|k| (k, false)).collect(), action);
}
fn perform_targets(s: &Rc<State>, keys: Vec<(ThreadKey, bool)>, action: MailAction) {
    actions::perform(s, keys, action);
}
pub fn single_message_actions(s: &State) -> bool {
    s.selected
        .borrow()
        .as_ref()
        .is_some_and(|key| selection::single_target(s, key))
}
pub fn single_target(s: &State, target: &ThreadKey) -> bool {
    selection::single_target(s, target)
}
pub fn multiple_selected(s: &State) -> bool {
    selection::keys(s).len() > 1
}
#[cfg(debug_assertions)]
pub fn verify_selection(s: &Rc<State>) {
    selection::diagnostic(s);
}
pub fn valid_single_selection(s: &State) -> bool {
    let keys = selection::keys(s);
    keys.len() == 1 || (keys.is_empty() && s.list.selection_mode() != gtk::SelectionMode::Multiple)
}
pub fn act_message(s: &Rc<State>, account: &str, id: &str, action: MailAction) {
    if !single_message_actions(s) {
        s.info("Select one conversation for message-specific actions.");
        return;
    }
    perform_targets(
        s,
        vec![(
            ThreadKey {
                account: account.into(),
                id: id.into(),
            },
            true,
        )],
        action,
    );
}
fn undo_last(s: &Rc<State>) {
    actions::undo(s);
}
pub fn add_account(s: &Rc<State>, drive: bool, expected: Option<String>) {
    crate::signin::open(s, drive, expected);
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
    crate::rendering::refresh_appearance(s);
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
        ("settings", vec!["<Primary>comma"]),
        ("quit", vec!["<Primary>q"]),
        ("close", vec!["<Primary>w"]),
        ("scale-up", vec!["<Primary>plus", "<Primary>equal"]),
        ("scale-down", vec!["<Primary>minus"]),
        ("scale-reset", vec!["<Primary>0"]),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let state = s.clone();
        action.connect_activate(move |_, _| match name {
            "settings" => crate::settings::open(&state),
            "quit" => crate::tray::quit(&state),
            "close" => {
                state.window.close();
            }
            "scale-up" => crate::presentation::change_scale(&state, 1),
            "scale-down" => crate::presentation::change_scale(&state, -1),
            _ => {
                state.settings.borrow_mut().ui_scale_percent = 100;
                state.save();
                crate::presentation::apply(&state);
            }
        });
        s.window.add_action(&action);
        s.app.set_accels_for_action(&format!("win.{name}"), &accels);
    }
    for index in 0..9 {
        let action = gio::SimpleAction::new(&format!("account-{}", index + 1), None);
        let state = s.clone();
        action.connect_activate(move |_, _| {
            let account = state
                .settings
                .borrow()
                .accounts
                .iter()
                .filter(|a| a.enabled)
                .nth(index)
                .cloned();
            if let Some(account) = account {
                *state.filter.borrow_mut() = Some(account.id.clone());
                *state.folder.borrow_mut() = Folder::Inbox;
                state.title.set_text(&account.name);
                sidebar(&state);
                reload(&state);
                load_labels(&state, &account.id);
            }
        });
        s.window.add_action(&action);
        s.app.set_accels_for_action(
            &format!("win.account-{}", index + 1),
            &[&format!("<Primary>{}", index + 1)],
        );
    }
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
