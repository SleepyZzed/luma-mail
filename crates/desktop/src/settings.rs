use crate::ui::{self, State, label, margins, vbox};
use adw::prelude::*;
use gtk::glib;
use luma_core::{
    Error,
    auth::{SystemVault, TokenManager},
    model::MailAction,
};
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub fn open(s: &Rc<State>) {
    let dialog = adw::PreferencesDialog::builder().title("Settings").build();
    let accounts = adw::PreferencesPage::builder()
        .title("Accounts")
        .icon_name("system-users-symbolic")
        .build();
    let group = adw::PreferencesGroup::builder()
        .title("Google accounts")
        .description("Gmail is the source of truth. Disconnecting never deletes mail.")
        .build();
    let add = adw::ActionRow::builder()
        .title("Sign in with Google")
        .subtitle("Opens Google's consent screen in your default browser")
        .activatable(true)
        .build();
    group.add(&add);
    {
        let s = s.clone();
        add.connect_activated(move |_| ui::add_account(&s, false, None));
    }
    accounts.add(&group);
    for account in s.settings.borrow().accounts.clone() {
        let g = adw::PreferencesGroup::builder()
            .title(&account.email)
            .build();
        let name = adw::EntryRow::builder()
            .title("Local name")
            .text(&account.name)
            .build();
        g.add(&name);
        {
            let s = s.clone();
            let id = account.id.clone();
            name.connect_changed(move |e| {
                if let Some(a) = s
                    .settings
                    .borrow_mut()
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    a.name = e.text().into();
                }
                s.save_debounced();
                ui::sidebar(&s);
            });
        }
        let enabled = adw::SwitchRow::builder()
            .title("Enable account")
            .active(account.enabled)
            .build();
        g.add(&enabled);
        {
            let s = s.clone();
            let id = account.id.clone();
            enabled.connect_active_notify(move |e| {
                if let Some(a) = s
                    .settings
                    .borrow_mut()
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    a.enabled = e.is_active();
                }
                s.save();
                if let Err(e) = s.rebuild_clients() {
                    s.error(&e);
                }
                ui::sidebar(&s);
                ui::reload(&s);
            });
        }
        let notify = adw::SwitchRow::builder()
            .title("New mail notifications")
            .active(account.notifications)
            .build();
        g.add(&notify);
        {
            let s = s.clone();
            let id = account.id.clone();
            notify.connect_active_notify(move |e| {
                if let Some(a) = s
                    .settings
                    .borrow_mut()
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    a.notifications = e.is_active();
                }
                s.save();
            });
        }
        let signature = adw::EntryRow::builder()
            .title("Signature (plain text)")
            .text(&account.signature)
            .build();
        g.add(&signature);
        {
            let s = s.clone();
            let id = account.id.clone();
            signature.connect_changed(move |e| {
                if let Some(a) = s
                    .settings
                    .borrow_mut()
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    a.signature = e.text().into();
                }
                s.save_debounced();
            });
        }
        for (title, mode) in [
            ("Use as default sender", 0),
            ("Move account up", 1),
            ("Reconnect Google account", 2),
            ("Enable Drive permission", 3),
            ("Clear this account's cache", 4),
            ("Revoke authorization and remove account", 5),
        ] {
            let row = adw::ActionRow::builder()
                .title(title)
                .activatable(true)
                .build();
            g.add(&row);
            let s = s.clone();
            let a = account.clone();
            row.connect_activated(move |_|match mode{
                0=>{s.settings.borrow_mut().default_account=Some(a.id.clone());s.save();s.info("Default sending account updated");},
                1=>{let mut p=s.settings.borrow_mut();if let Some(i)=p.accounts.iter().position(|b|b.id==a.id)&& i>0{p.accounts.swap(i,i-1);}drop(p);s.save();ui::sidebar(&s);},
                2=>ui::add_account(&s,a.drive_enabled,Some(a.email.clone())),
                3=>{let d=adw::AlertDialog::builder().heading("Allow Google Drive files?").body("Google will request gmail.modify and drive.file together. Luma can manage files you select or create with this app. Sharing still requires a separate choice for each upload.").build();d.add_responses(&[("cancel","Cancel"),("allow","Continue to Google")]);let s=s.clone();let email=a.email.clone();glib::MainContext::default().spawn_local(async move{if d.choose_future(Some(&s.window)).await=="allow"{ui::add_account(&s,true,Some(email));}});},
                4=>match s.cache.lock().expect("cache mutex").clear_account(&a.id){Ok(())=>s.info("Account cache cleared"),Err(e)=>s.error(&e)},
                _=>remove_account(&s,&a.id,&a.email),
            });
        }
        let bytes = s
            .cache
            .lock()
            .expect("cache mutex")
            .account_usage(&account.id)
            .unwrap_or(0);
        g.add(
            &adw::ActionRow::builder()
                .title("Disposable cache")
                .subtitle(format!("{bytes} allocated bytes"))
                .build(),
        );
        accounts.add(&g);
    }
    dialog.add(&accounts);
    let appearance = adw::PreferencesPage::builder()
        .title("Appearance")
        .icon_name("applications-graphics-symbolic")
        .build();
    let g=adw::PreferencesGroup::builder().title("Appearance and Noctalia").description("Noctalia reads a generated palette, with live file-monitor updates. See docs/noctalia.md.").build();
    let modes = ["system", "dark", "light", "noctalia"];
    let names = gtk::StringList::new(&["Follow system", "Dark", "Light", "Follow Noctalia"]);
    let selected = modes
        .iter()
        .position(|v| *v == s.settings.borrow().theme)
        .unwrap_or(0) as u32;
    let theme = adw::ComboRow::builder()
        .title("Theme")
        .model(&names)
        .selected(selected)
        .build();
    g.add(&theme);
    {
        let s = s.clone();
        theme.connect_selected_notify(move |r| {
            s.settings.borrow_mut().theme = modes[r.selected() as usize].into();
            s.save();
            ui::apply_theme(&s);
        });
    }
    let compact = adw::SwitchRow::builder()
        .title("Compact message density")
        .active(s.settings.borrow().compact)
        .build();
    g.add(&compact);
    {
        let s = s.clone();
        compact.connect_active_notify(move |e| {
            s.settings.borrow_mut().compact = e.is_active();
            s.save();
            ui::reload(&s);
        });
    }
    appearance.add(&g);
    dialog.add(&appearance);
    let storage = adw::PreferencesPage::builder()
        .title("Storage & Cache")
        .icon_name("drive-harddisk-symbolic")
        .build();
    let g=adw::PreferencesGroup::builder().title("Enforced disposable cache").description("Message bodies only. Cache allocation includes files and conservative directory overhead. Credentials, settings, and explicitly saved files are outside this budget.").build();
    let sizes = [0, 50, 100, 250, 500];
    let size = adw::ComboRow::builder()
        .title("Cache budget")
        .model(&gtk::StringList::new(&[
            "Disabled · 0 MiB",
            "50 MiB",
            "100 MiB",
            "250 MiB",
            "500 MiB",
            "Custom",
        ]))
        .selected(
            sizes
                .iter()
                .position(|v| *v == s.settings.borrow().cache_mib)
                .unwrap_or(5) as u32,
        )
        .build();
    g.add(&size);
    let custom = adw::SpinRow::with_range(0., 16384., 1.);
    custom.set_title("Custom budget (MiB)");
    custom.set_value(s.settings.borrow().cache_mib as f64);
    g.add(&custom);
    let ttl = adw::SpinRow::with_range(0., 8760., 1.);
    ttl.set_title("Expire contents after (hours; 0 disables retention)");
    ttl.set_value(s.settings.borrow().cache_ttl_hours as f64);
    g.add(&ttl);
    let usage = adw::ActionRow::builder()
        .title("Actual allocated storage")
        .subtitle(format!(
            "{} bytes",
            s.cache.lock().expect("cache mutex").usage().unwrap_or(0)
        ))
        .build();
    g.add(&usage);
    {
        let s = s.clone();
        let custom = custom.clone();
        size.connect_selected_notify(move |r| {
            if let Some(value) = sizes.get(r.selected() as usize) {
                custom.set_value(*value as f64);
                update_cache(&s, Some(*value), None);
            }
        });
    }
    {
        let s = s.clone();
        custom.connect_value_notify(move |r| update_cache(&s, Some(r.value() as u64), None));
    }
    {
        let s = s.clone();
        ttl.connect_value_notify(move |r| update_cache(&s, None, Some(r.value() as u64)));
    }
    let onexit = adw::SwitchRow::builder()
        .title("Clear cache on full exit")
        .active(s.settings.borrow().clear_cache_on_exit)
        .build();
    g.add(&onexit);
    {
        let s = s.clone();
        onexit.connect_active_notify(move |e| {
            s.settings.borrow_mut().clear_cache_on_exit = e.is_active();
            s.save();
        });
    }
    let clear = adw::ActionRow::builder()
        .title("Clear all cache now")
        .activatable(true)
        .build();
    g.add(&clear);
    {
        let s = s.clone();
        clear.connect_activated(
            move |_| match s.cache.lock().expect("cache mutex").clear() {
                Ok(()) => {
                    usage.set_subtitle("0 bytes");
                    s.info("Disposable cache cleared");
                }
                Err(e) => s.error(&e),
            },
        );
    }
    storage.add(&g);
    dialog.add(&storage);
    let general = adw::PreferencesPage::builder()
        .title("Notifications & Privacy")
        .icon_name("preferences-system-notifications-symbolic")
        .build();
    let g=adw::PreferencesGroup::builder().title("While Luma is running").description("Bounded Gmail history polling continues in the tray. Tray → Quit exits completely; no daemon or autostart is installed.").build();
    let poll = adw::SpinRow::with_range(30., 3600., 30.);
    poll.set_title("Polling interval (seconds)");
    poll.set_value(s.settings.borrow().poll_seconds as f64);
    g.add(&poll);
    {
        let s = s.clone();
        poll.connect_value_notify(move |r| {
            s.settings.borrow_mut().poll_seconds = r.value() as u64;
            s.save();
        });
    }
    let privacy = adw::SwitchRow::builder()
        .title("Hide sender and subject in notifications")
        .active(s.settings.borrow().notification_privacy)
        .build();
    g.add(&privacy);
    {
        let s = s.clone();
        privacy.connect_active_notify(move |r| {
            s.settings.borrow_mut().notification_privacy = r.is_active();
            s.save();
        });
    }
    g.add(
        &adw::ActionRow::builder()
            .title("Remote images and scripts are blocked")
            .subtitle(
                "Mail is rendered as safe text; no browser engine or remote content requests.",
            )
            .build(),
    );
    general.add(&g);
    let tray_group=adw::PreferencesGroup::builder().title("System tray").description("Closing hides Luma when a StatusNotifier tray host is available. Email polling and system notifications continue. Tray right-click → Quit exits completely.").build();
    let tray = adw::SwitchRow::builder()
        .title("Close to system tray")
        .active(s.settings.borrow().close_to_tray)
        .build();
    tray_group.add(&tray);
    {
        let s = s.clone();
        tray.connect_active_notify(move |r| {
            s.settings.borrow_mut().close_to_tray = r.is_active();
            s.save();
        });
    }
    tray_group.add(
        &adw::ActionRow::builder()
            .title("Tray connection")
            .subtitle(if s.tray_available.get() {
                "Connected to the desktop StatusNotifier service"
            } else {
                "Unavailable; closing will exit instead of hiding"
            })
            .build(),
    );
    let test = adw::ActionRow::builder()
        .title("Send a system test notification")
        .subtitle("Uses the desktop notification daemon; no email is simulated")
        .activatable(true)
        .build();
    tray_group.add(&test);
    {
        let s = s.clone();
        test.connect_activated(move |_|{let n=gtk::gio::Notification::new("Luma system notification test");n.set_body(Some("This notification was requested through the desktop notification service."));s.app.send_notification(Some("luma-system-test"),&n);s.info("System notification requested. Its display is controlled by your desktop daemon and Do Not Disturb settings.");});
    }
    general.add(&tray_group);
    dialog.add(&general);
    let about = adw::PreferencesPage::builder()
        .title("About & Diagnostics")
        .icon_name("help-about-symbolic")
        .build();
    let g = adw::PreferencesGroup::builder()
        .title(format!(
            "Luma Mail {} · GPL-3.0-or-later",
            env!("CARGO_PKG_VERSION")
        ))
        .description("Development build. See STATUS.md for verified and outstanding features.")
        .build();
    for (title, subtitle) in [
        ("Configuration", s.paths.config.display().to_string()),
        ("Cache", s.paths.cache.display().to_string()),
        ("Google sign-in", if s.client_config().is_ok() { "Available" } else { "Unavailable in this build; app registration pending" }.into()),
        ("GTK cursor", crate::desktop::cursor_diagnostic()),
        ("Connected providers", format!("{} enabled Gmail accounts", s.clients.borrow().len())),
        ("Recent error", s.last_error.borrow().clone().unwrap_or_else(|| "None recorded".into())),
        ("Keyboard shortcuts", "Ctrl+N compose · Ctrl+F search · Ctrl+R refresh · Ctrl+E archive · Delete trash · Ctrl+Shift+U unread · Ctrl+1…9 account".into()),
    ] { g.add(&adw::ActionRow::builder().title(title).subtitle(subtitle).build()); }
    about.add(&g);
    dialog.add(&about);
    dialog.present(Some(&s.window));
}
fn update_cache(s: &Rc<State>, size: Option<u64>, ttl: Option<u64>) {
    let mut p = s.settings.borrow_mut();
    if let Some(v) = size {
        p.cache_mib = v;
    }
    if let Some(v) = ttl {
        p.cache_ttl_hours = v;
    }
    let result = s.cache.lock().expect("cache mutex").configure(
        p.cache_mib.saturating_mul(1024 * 1024),
        Duration::from_secs(p.cache_ttl_hours.saturating_mul(3600)),
    );
    drop(p);
    match result {
        Ok(()) => s.save(),
        Err(e) => s.error(&e),
    }
}
fn remove_account(s: &Rc<State>, id: &str, email: &str) {
    let d=adw::AlertDialog::builder().heading(format!("Disconnect {email}?" )).body("Revokes Luma's Google authorization and removes local credentials and cached contents. Gmail messages are kept.").build();
    d.add_responses(&[("cancel", "Cancel"), ("remove", "Disconnect")]);
    d.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    let s = s.clone();
    let id = id.to_owned();
    glib::MainContext::default().spawn_local(async move {
        if d.choose_future(Some(&s.window)).await != "remove" {
            return;
        }
        let config = match s.client_config() {
            Ok(c) => c,
            Err(e) => {
                s.error(&e);
                return;
            }
        };
        let manager = TokenManager::new(id.clone(), config, s.http.clone(), Arc::new(SystemVault));
        let cache = s.cache.clone();
        ui::async_work(
            &s,
            async move {
                manager.revoke().await?;
                cache.lock().expect("cache mutex").clear_account(&id)?;
                Ok(id)
            },
            |s, id| {
                let mut p = s.settings.borrow_mut();
                p.accounts.retain(|a| a.id != id);
                if p.default_account.as_ref() == Some(&id) {
                    p.default_account = p.accounts.iter().find(|a| a.enabled).map(|a| a.id.clone());
                }
                drop(p);
                s.save();
                if let Err(e) = s.rebuild_clients() {
                    s.error(&e);
                }
                ui::sidebar(&s);
                ui::reload(&s);
            },
        );
    });
}

pub fn label_dialog(s: &Rc<State>) {
    let Some(key) = s.selected.borrow().clone() else {
        return;
    };
    let Some(c) = s.clients.borrow().get(&key.account).cloned() else {
        return;
    };
    ui::async_work(
        s,
        async move {
            use luma_core::provider::MailProvider;
            c.labels().await
        },
        |s, labels| {
            let w = adw::Window::builder()
                .transient_for(&s.window)
                .modal(true)
                .title("Gmail labels")
                .default_width(420)
                .default_height(500)
                .build();
            let root = vbox(10);
            root.append(&adw::HeaderBar::new());
            let list = vbox(6);
            margins(&list, 16);
            for l in labels.into_iter().filter(|l| l.kind == "user") {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let name = gtk::Entry::builder().text(&l.name).hexpand(true).build();
                row.append(&name);
                for (title, mode) in [("+", 0), ("−", 1), ("Rename", 2), ("Delete", 3)] {
                    let b = gtk::Button::with_label(title);
                    b.set_tooltip_text(Some(match mode {
                        0 => "Add label to selected conversations",
                        1 => "Remove label from selected conversations",
                        2 => "Rename Gmail label",
                        _ => "Delete Gmail label, keeping messages",
                    }));
                    row.append(&b);
                    let s = s.clone();
                    let id = l.id.clone();
                    let name = name.clone();
                    b.connect_clicked(move |_| match mode {
                        0 => ui::act_selected(&s, MailAction::AddLabel(id.clone())),
                        1 => ui::act_selected(&s, MailAction::RemoveLabel(id.clone())),
                        _ => mutate_label(&s, id.clone(), name.text().into(), mode == 3),
                    });
                }
                list.append(&row);
            }
            let name = gtk::Entry::builder()
                .placeholder_text("New label name")
                .build();
            list.append(&name);
            let create = gtk::Button::with_label("Create label on Gmail");
            list.append(&create);
            {
                let s = s.clone();
                create.connect_clicked(move |_| {
                    let Some(key) = s.selected.borrow().clone() else {
                        return;
                    };
                    let Some(c) = s.clients.borrow().get(&key.account).cloned() else {
                        return;
                    };
                    let name = name.text().to_string();
                    let account = c.account.clone();
                    ui::async_work(
                        &s,
                        async move { c.create_label(&name).await },
                        move |s, _| {
                            ui::load_labels(&s, &account);
                            s.info("Label created on Gmail. Reopen Labels to refresh.")
                        },
                    );
                });
            }
            root.append(
                &gtk::ScrolledWindow::builder()
                    .vexpand(true)
                    .child(&list)
                    .build(),
            );
            w.set_content(Some(&root));
            w.present();
        },
    );
}
fn mutate_label(s: &Rc<State>, id: String, name: String, delete: bool) {
    let Some(key) = s.selected.borrow().clone() else {
        return;
    };
    let Some(c) = s.clients.borrow().get(&key.account).cloned() else {
        return;
    };
    let s = s.clone();
    glib::MainContext::default().spawn_local(async move {
        if delete {
            let d = adw::AlertDialog::builder()
                .heading("Delete this Gmail label?")
                .body("The label is removed from all mail. Messages are kept.")
                .build();
            d.add_responses(&[("cancel", "Cancel"), ("delete", "Delete label")]);
            d.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            if d.choose_future(Some(&s.window)).await != "delete" {
                return;
            }
        }
        ui::async_work(
            &s,
            async move {
                if delete {
                    c.delete_label(&id).await?;
                } else {
                    c.rename_label(&id, &name).await?;
                }
                Ok(c.account.clone())
            },
            |s, account| {
                ui::load_labels(&s, &account);
                s.info("Gmail label updated. Reopen Labels to refresh.");
            },
        );
    });
}

pub fn drive_dialog(s: &Rc<State>, text: &gtk::TextView, id: &str) {
    let account = s
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|a| a.id == id)
        .cloned();
    let Some(a) = account else { return };
    if !a.drive_enabled {
        s.info("Enable Drive permission for this account in Settings, then reopen Drive link.");
        return;
    }
    let Some(c) = s.clients.borrow().get(&a.id).cloned() else {
        return;
    };
    let w = adw::Window::builder()
        .application(&s.app)
        .transient_for(&s.window)
        .title("Drive attachment")
        .default_width(540)
        .default_height(380)
        .build();
    let root = vbox(12);
    root.append(&adw::HeaderBar::new());
    let form = vbox(12);
    margins(&form, 20);
    let explanation = label(
        "Uploads stay private until you choose recipients below. Each recipient gets read-only access. No public link permission is created.",
        "",
    );
    explanation.set_wrap(true);
    form.append(&explanation);
    let recipients = gtk::Entry::builder()
        .placeholder_text("Share with email addresses (comma separated)")
        .build();
    form.append(&recipients);
    let existing = gtk::Entry::builder()
        .placeholder_text("Existing app-authorized Drive file ID (optional)")
        .build();
    form.append(&existing);
    let use_existing = gtk::Button::with_label("Share existing file and insert link");
    form.append(&use_existing);
    let upload = gtk::Button::with_label("Choose file, upload and share…");
    form.append(&upload);
    let progress = gtk::ProgressBar::new();
    progress.set_show_text(true);
    form.append(&progress);
    let status = label("Choose sharing recipients before continuing.", "dim-label");
    status.set_wrap(true);
    form.append(&status);
    let cancel = gtk::Button::with_label("Cancel upload");
    cancel.set_sensitive(false);
    form.append(&cancel);
    root.append(&form);
    w.set_content(Some(&root));
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let flag = cancelled.clone();
        cancel.connect_clicked(move |_| flag.store(true, Ordering::Relaxed));
    }
    for (b, is_upload) in [(upload.clone(), true), (use_existing.clone(), false)] {
        let (
            s,
            c,
            text,
            w,
            recipients,
            existing,
            status,
            progress,
            cancel,
            cancelled,
            upload,
            use_existing,
        ) = (
            s.clone(),
            c.clone(),
            text.clone(),
            w.clone(),
            recipients.clone(),
            existing.clone(),
            status.clone(),
            progress.clone(),
            cancel.clone(),
            cancelled.clone(),
            upload.clone(),
            use_existing.clone(),
        );
        b.connect_clicked(move |_|{
            let emails=recipients.text().to_string();if luma_core::composer::recipients(&emails).is_err()||emails.trim().is_empty(){status.set_text("Enter valid recipients to receive read-only access.");return;}
            let(s,c,text,w,status,progress,cancel,cancelled,upload,use_existing)=(s.clone(),c.clone(),text.clone(),w.clone(),status.clone(),progress.clone(),cancel.clone(),cancelled.clone(),upload.clone(),use_existing.clone());let id=existing.text().to_string();
            glib::MainContext::default().spawn_local(async move{
                let path=if is_upload{let picker=gtk::FileDialog::builder().title("Upload to Google Drive").build();let Ok(f)=picker.open_future(Some(&w)).await else{return};let Some(p)=f.path()else{return};Some(p)}else{None};
                let d=adw::AlertDialog::builder().heading("Upload and grant read access?").body(format!("This will change Drive sharing for: {emails}. A link is inserted only after every permission request succeeds.")).build();d.add_responses(&[("cancel","Cancel"),("share","Upload / share")]);if d.choose_future(Some(&w)).await!="share"{return;}
                cancelled.store(false,Ordering::Relaxed);upload.set_sensitive(false);use_existing.set_sensitive(false);cancel.set_sensitive(true);status.set_text("Preparing Drive attachment…");let(tx,mut rx)=tokio::sync::watch::channel((0,1));
                glib::MainContext::default().spawn_local(async move{while rx.changed().await.is_ok(){let(done,total)=*rx.borrow();progress.set_fraction(done as f64/total.max(1)as f64);progress.set_text(Some(&format!("{done} / {total} bytes")));}});
                let flag=cancelled.clone();ui::async_work(&s,async move{let result=async{let file=if let Some(path)=path{luma_core::drive::upload(&c,&path,tx,flag.clone()).await?}else{luma_core::drive::existing(&c,&id).await?};if flag.load(Ordering::Relaxed){return Err(Error::Invalid("Upload cancelled. No link inserted.".into()));}let link=luma_core::drive::share(&c,&file,&emails).await?;Ok::<_,Error>((file.name,link))}.await;Ok(result)},move|_,result|{upload.set_sensitive(true);use_existing.set_sensitive(true);cancel.set_sensitive(false);match result{Ok((name,link))=>{text.buffer().insert_at_cursor(&format!("\n{name}: {link}\n"));w.close();},Err(e)=>status.set_text(&format!("{e}. No link inserted. An uploaded file may remain private in Drive."))}});
            });
        });
    }
    w.present();
}
