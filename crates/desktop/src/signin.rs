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
    drive: bool,
    expected: Option<String>,
}

pub fn open(s: &Rc<State>, drive: bool, expected: Option<String>) {
    open_with_config(s, drive, expected, s.client_config());
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn open_unconfigured(s: &Rc<State>) {
    open_with_config(s, false, None, ClientConfig::from_registration("", ""));
}

fn open_with_config(
    s: &Rc<State>,
    drive: bool,
    expected: Option<String>,
    config: Result<ClientConfig>,
) {
    if let Some(window) = s.signin_window.borrow().as_ref() {
        window.present();
        return;
    }
    let window = adw::Window::builder()
        .application(&s.app)
        .transient_for(&s.window)
        .title("Sign in with Google")
        .default_width(560)
        .default_height(420)
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
        drive,
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
        flow.window.connect_destroy(move |_| {
            lifetime.borrow_mut().take();
        });
    }
    flow.window.present();
    match config {
        Ok(config) => begin(&flow, config),
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
    let drive = flow.drive;
    let weak = Rc::downgrade(flow);
    let task = ui::async_work(
        &flow.state,
        async move { Ok(Authorization::begin(&c, drive).await) },
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
        Ok::<_, Error>((id, profile.email_address))
    }.await)
        },
        move |s, result| {
            let Some(flow) = weak.upgrade() else { return };
            flow.task.borrow_mut().take();
            match result {
                Ok((id, email)) => {
                    let mut settings = s.settings.borrow_mut();
                    if let Some(account) = settings.accounts.iter_mut().find(|a| a.id == id) {
                        account.enabled = true;
                        account.drive_enabled = flow.drive;
                    } else {
                        settings.accounts.push(Account {
                            id: id.clone(),
                            email: email.clone(),
                            name: email.split('@').next().unwrap_or(&email).into(),
                            enabled: true,
                            notifications: true,
                            signature: String::new(),
                            drive_enabled: flow.drive,
                        });
                    }
                    if settings.default_account.is_none() {
                        settings.default_account = Some(id);
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
    flow.spinner.stop();
    flow.browser
        .set_sensitive(flow.state.client_config().is_ok());
    flow.browser.set_label("Try Google sign-in again");
    flow.copy.set_sensitive(false);
    flow.url.borrow_mut().take();
    flow.status.set_text(&error.to_string());
}
