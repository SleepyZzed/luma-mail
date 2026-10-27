use crate::ui::{self, State};
use adw::prelude::*;
use gtk::glib;
use luma_core::{
    Error, Result,
    auth::{Authorization, ClientConfig, Credentials, SystemVault, Vault},
    config::Account,
    gmail::Gmail,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct SignIn {
    state: Rc<State>,
    window: adw::Window,
    status: gtk::Label,
    spinner: gtk::Spinner,
    browser: gtk::Button,
    copy: gtk::Button,
    url: RefCell<Option<String>>,
    task: RefCell<Option<tokio::task::AbortHandle>>,
    active: Cell<bool>,
    permissions: [gtk::CheckButton; 3],
    ready: RefCell<Option<Ready>>,
    expected: Option<String>,
}
enum Feature {
    Drive,
    Settings,
    Photos,
}
impl Feature {
    fn granted(&self, p: luma_core::auth::Permissions) -> bool {
        match self {
            Self::Drive => p.drive,
            Self::Settings => p.settings,
            Self::Photos => p.photos,
        }
    }
}
type Ready = (Feature, Box<dyn FnOnce(Rc<State>, String)>);

pub fn open(s: &Rc<State>, drive: bool, expected: Option<String>) {
    #[cfg(debug_assertions)]
    if std::env::args().any(|a| a == "--design-preview") {
        s.info("Use the normal Luma window to connect an account. This preview contains synthetic test data.");
        return;
    }
    let account = expected.as_ref().and_then(|email| {
        s.settings
            .borrow()
            .accounts
            .iter()
            .find(|a| &a.email == email)
            .cloned()
    });
    let filters = account.as_ref().is_some_and(|a| a.sender_controls);
    let drive = drive || account.as_ref().is_some_and(|a| a.drive_enabled);
    let photos = account.as_ref().is_some_and(|a| a.contact_photos);
    open_with_config(s, drive, expected, filters, photos, None, s.client_config());
}
pub fn sender_controls(
    s: &Rc<State>,
    account: &Account,
    ready: impl FnOnce(Rc<State>, String) + 'static,
) {
    open_with_config(
        s,
        account.drive_enabled,
        Some(account.email.clone()),
        true,
        account.contact_photos,
        Some((Feature::Settings, Box::new(ready))),
        s.client_config(),
    );
}

pub fn contact_photos(
    s: &Rc<State>,
    account: &Account,
    ready: impl FnOnce(Rc<State>, String) + 'static,
) {
    open_with_config(
        s,
        account.drive_enabled,
        Some(account.email.clone()),
        account.sender_controls,
        true,
        Some((Feature::Photos, Box::new(ready))),
        s.client_config(),
    );
}
pub fn drive_files(
    s: &Rc<State>,
    account: &Account,
    ready: impl FnOnce(Rc<State>, String) + 'static,
) {
    open_with_config(
        s,
        true,
        Some(account.email.clone()),
        account.sender_controls,
        account.contact_photos,
        Some((Feature::Drive, Box::new(ready))),
        s.client_config(),
    );
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn open_unconfigured(s: &Rc<State>) {
    open_with_config(
        s,
        false,
        None,
        false,
        false,
        None,
        ClientConfig::from_registration("", ""),
    );
}

fn open_with_config(
    s: &Rc<State>,
    drive: bool,
    expected: Option<String>,
    filters: bool,
    photos: bool,
    ready: Option<Ready>,
    config: Result<ClientConfig>,
) {
    #[cfg(debug_assertions)]
    if std::env::args().any(|a| a == "--design-preview") {
        s.info("Use the normal Luma window to connect an account. This preview contains synthetic test data.");
        return;
    }
    if let Some(window) = s.signin_window.borrow().as_ref() {
        window.present();
        return;
    }
    let window = adw::Window::builder()
        .application(&s.app)
        .transient_for(&s.window)
        .title("Sign in with Google")
        .default_width(560)
        .default_height(590)
        .build();
    let root = ui::vbox(12);
    root.append(&adw::HeaderBar::new());
    let content = ui::vbox(12);
    ui::margins(&content, 24);
    content.append(&ui::label("Connect your Gmail account", "title-2"));
    let intro = ui::label(
        "Choose your Google account and allow Luma to access Gmail in your browser. Your password stays with Google. You can revoke access from your Google Account at any time.",
        "",
    );
    intro.set_wrap(true);
    content.append(&intro);
    let optional = ui::vbox(6);
    optional.append(&ui::label(
        "Optional features · one Google consent screen",
        "heading",
    ));
    let permissions = [
        gtk::CheckButton::with_label("Drive attachments · upload large files and share links"),
        gtk::CheckButton::with_label(
            "Sender controls and online signatures · manage Gmail settings",
        ),
        gtk::CheckButton::with_label("Contact photos · read Google Contacts"),
    ];
    let new_account = expected.is_none();
    for (check, requested) in permissions.iter().zip([drive, filters, photos]) {
        check.set_active(new_account || requested);
        optional.append(check);
    }
    let hint = ui::label(
        "Gmail is required. You can skip optional features here or on Google's consent screen and connect them later in Settings.",
        "caption",
    );
    hint.set_wrap(true);
    optional.append(&hint);
    content.append(&optional);
    let status = ui::label("", "");
    status.set_wrap(true);
    status.set_selectable(true);
    content.append(&status);
    let spinner = gtk::Spinner::new();
    content.append(&spinner);
    let browser = gtk::Button::with_label("Sign in with Google");
    browser.add_css_class("suggested-action");
    browser.set_sensitive(false);
    content.append(&browser);
    let copy = gtk::Button::with_label("Copy sign-in link");
    copy.set_sensitive(false);
    content.append(&copy);
    root.append(&content);
    window.set_content(Some(&root));
    let flow = Rc::new(SignIn {
        state: s.clone(),
        window,
        status,
        spinner,
        browser,
        copy,
        url: RefCell::new(None),
        task: RefCell::new(None),
        active: Cell::new(false),
        permissions,
        ready: RefCell::new(ready),
        expected,
    });
    *s.signin_window.borrow_mut() = Some(flow.window.clone());
    {
        let weak = Rc::downgrade(&flow);
        flow.browser.connect_clicked(move |_| {
            if let Some(flow) = weak.upgrade() {
                if flow.url.borrow().is_some() {
                    launch_browser(&flow);
                } else {
                    match flow.state.client_config() {
                        Ok(config) => begin(&flow, config),
                        Err(error) => failed(&flow, &error),
                    }
                }
            }
        });
    }
    {
        let weak = Rc::downgrade(&flow);
        flow.copy.connect_clicked(move |_| {
            if let Some(flow) = weak.upgrade() && let Some(url) = flow.url.borrow().as_ref() {
                flow.window.clipboard().set_text(url);
                flow.status.set_text("Sign-in link copied. Open it in your browser and finish consent within five minutes.");
            }
        });
    }
    {
        let weak = Rc::downgrade(&flow);
        flow.window.connect_close_request(move |_| {
            if let Some(flow) = weak.upgrade() {
                flow.active.set(false);
                if let Some(task) = flow.task.borrow_mut().take() {
                    task.abort();
                }
                flow.state.signin_window.borrow_mut().take();
            }
            glib::Propagation::Proceed
        });
    }
    {
        let lifetime = RefCell::new(Some(flow.clone()));
        flow.window.connect_unrealize(move |_| {
            lifetime.borrow_mut().take();
        });
    }
    flow.window.present();
    match config {
        Ok(config) => {
            if flow.expected.is_some() {
                begin(&flow, config);
            } else {
                flow.browser.set_sensitive(true);
                flow.status
                    .set_text("Choose optional features, then continue to Google once.");
            }
        }
        Err(error) => flow.status.set_text(&error.to_string()),
    }
}

fn begin(flow: &Rc<SignIn>, config: ClientConfig) {
    if flow.active.replace(true) {
        return;
    }
    flow.browser.set_sensitive(false);
    flow.spinner.start();
    flow.status.set_text("Preparing secure Google sign-in…");
    let c = config.clone();
    let [drive, filters, photos] = flow.permissions.each_ref().map(|check| check.is_active());
    for check in &flow.permissions {
        check.set_sensitive(false);
    }
    let weak = Rc::downgrade(flow);
    let task = ui::async_work(
        &flow.state,
        async move { Ok(Authorization::begin_with_photos(&c, drive, filters, photos).await) },
        move |_, result| {
            let Some(flow) = weak.upgrade() else { return };
            flow.task.borrow_mut().take();
            match result {
                Ok(auth) => wait_for_consent(&flow, auth, config),
                Err(error) => failed(&flow, &error),
            }
        },
    );
    *flow.task.borrow_mut() = Some(task);
}

fn launch_browser(flow: &Rc<SignIn>) {
    let Some(url) = flow.url.borrow().clone() else {
        return;
    };
    #[cfg(debug_assertions)]
    if crate::verification::browser_url(&flow.state, &url) {
        return;
    }
    let flow = flow.clone();
    glib::MainContext::default().spawn_local(async move {
        if gtk::UriLauncher::new(&url).launch_future(Some(&flow.window)).await.is_err() && flow.active.get() {
            flow.status.set_text("Your browser could not be opened. Use Copy sign-in link, then open that link manually in your browser. The request expires after five minutes.");
        }
    });
}

fn wait_for_consent(flow: &Rc<SignIn>, auth: Authorization, config: ClientConfig) {
    *flow.url.borrow_mut() = Some(auth.url.to_string());
    flow.browser.set_label("Open browser again");
    flow.browser.set_sensitive(true);
    flow.copy.set_sensitive(true);
    flow.status.set_text("Complete Google sign-in and consent in your browser. This window waits for Google's callback for up to five minutes. Close it to cancel.");
    let http = flow.state.http.clone();
    let existing = flow.state.settings.borrow().accounts.clone();
    let expected = flow.expected.clone();
    let weak = Rc::downgrade(flow);
    let task = ui::async_work(
        &flow.state,
        async move {
            Ok(async {
        let token = auth.complete(&http, &config).await?;
        let permissions=token.permissions()?;
        let profile = Gmail::profile_with_token(&http, &token.access_token).await?;
        if expected.is_some_and(|email| !profile.email_address.eq_ignore_ascii_case(&email)) {
            return Err(Error::Invalid("Sign in with the same Google account when reconnecting".into()));
        }
        let id = existing.iter().find(|a| a.email.eq_ignore_ascii_case(&profile.email_address))
            .map(|a| a.id.clone()).unwrap_or_else(luma_core::composer::new_id);
        if let Some(refresh) = &token.refresh_token {
            SystemVault.store(&id, &Credentials { refresh_token: refresh.clone(), client_id: config.client_id.clone() }).await?;
        } else {
            // A reconnect can legitimately omit an unchanged refresh token.
            let saved = SystemVault.load(&id).await.map_err(|_| Error::Invalid("Google did not return a refresh token. Revoke the previous grant in Google Account permissions and reconnect.".into()))?;
            if saved.client_id != config.client_id {
                return Err(Error::Invalid("Google did not return a refresh token for Luma's app registration. Revoke the previous Luma grant in Google Account permissions and reconnect.".into()));
            }
        }
        Ok::<_, Error>((id, profile.email_address,permissions))
    }.await)
        },
        move |s, result| {
            let Some(flow) = weak.upgrade() else { return };
            flow.task.borrow_mut().take();
            match result {
                Ok((id, email, permissions)) => {
                    let mut settings = s.settings.borrow_mut();
                    if let Some(account) = settings.accounts.iter_mut().find(|a| a.id == id) {
                        account.enabled = true;
                        account.drive_enabled = permissions.drive;
                        account.sender_controls = permissions.settings;
                        account.contact_photos = permissions.photos;
                    } else {
                        settings.accounts.push(Account {
                            id: id.clone(),
                            email: email.clone(),
                            name: email.split('@').next().unwrap_or(&email).into(),
                            enabled: true,
                            notifications: true,
                            signature: String::new(),
                            drive_enabled: permissions.drive,
                            sender_controls: permissions.settings,
                            contact_photos: permissions.photos,
                            blocked_senders: vec![],
                        });
                    }
                    if settings.default_account.is_none() {
                        settings.default_account = Some(id.clone());
                    }
                    drop(settings);
                    s.save();
                    if let Err(error) = s.rebuild_clients() {
                        failed(&flow, &error);
                        return;
                    }
                    ui::sidebar(&s);
                    ui::reload(&s);
                    flow.window.close();
                    let requested = flow.permissions.each_ref().map(|c| c.is_active());
                    let granted = [permissions.drive, permissions.settings, permissions.photos];
                    let all = requested.iter().zip(granted).all(|(r, g)| !*r || g);
                    if !all {
                        s.info("Gmail is connected. Some optional features were skipped; connect them later in Settings.");
                    }
                    if let Some((feature, ready)) = flow.ready.borrow_mut().take()
                        && feature.granted(permissions)
                    {
                        ready(s, id);
                    }
                }
                Err(error) => failed(&flow, &error),
            }
        },
    );
    *flow.task.borrow_mut() = Some(task);
    launch_browser(flow);
}

fn failed(flow: &SignIn, error: &Error) {
    flow.active.set(false);
    for check in &flow.permissions {
        check.set_sensitive(true);
    }
    flow.spinner.stop();
    flow.browser
        .set_sensitive(flow.state.client_config().is_ok());
    flow.browser.set_label("Try Google sign-in again");
    flow.copy.set_sensitive(false);
    flow.url.borrow_mut().take();
    flow.status.set_text(&error.to_string());
}
