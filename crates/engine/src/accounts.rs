use crate::{app::App, protocol::Command};
use luma_core::{
    Error, Result,
    auth::{Authorization, Credentials, SystemVault, Vault},
    config::{Account, Settings},
    gmail::Gmail,
};
use serde_json::json;
use std::sync::{Arc, atomic::Ordering};

pub async fn publish(app: &App, id: u64) {
    let value = serde_json::to_value(&*app.settings.lock().unwrap()).unwrap_or_default();
    app.events.emit(id, "settings", value).await;
}
pub async fn cache_usage(app: &App, id: u64) -> Result<()> {
    let (bytes, accounts) = app.cache.lock().unwrap().usage_report()?;
    app.events
        .emit(id, "cacheUsage", json!({"bytes":bytes,"accounts":accounts}))
        .await;
    Ok(())
}
pub async fn update(app: &App, c: &Command) -> Result<()> {
    let _preferences = app.preferences.lock().await;
    const ALLOWED: &[&str] = &[
        "default_account",
        "cache_mib",
        "cache_ttl_hours",
        "clear_cache_on_exit",
        "theme",
        "compact",
        "ui_scale_percent",
        "reduced_motion",
        "window_chrome",
        "poll_seconds",
        "notification_privacy",
        "close_to_tray",
        "draft_mode",
        "recent_recipients",
        "window_width",
        "window_height",
        "list_width",
        "sidebar_width",
    ];
    let mut value = serde_json::to_value(&*app.settings.lock().unwrap())?;
    let changes = c
        .data
        .as_object()
        .ok_or_else(|| Error::Invalid("Invalid settings update".into()))?;
    for (key, v) in changes {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(Error::Invalid(
                "Use account controls to update account details".into(),
            ));
        }
        value[key] = v.clone();
    }
    let mut settings: Settings = serde_json::from_value(value)?;
    settings.ui_scale_percent = settings.ui_scale_percent.clamp(75, 200);
    settings.poll_seconds = settings.poll_seconds.clamp(30, 3600);
    settings.sidebar_width = settings.sidebar_width.clamp(160, 480);
    settings.list_width = settings.list_width.clamp(180, 1200);
    settings.window_width = settings.window_width.clamp(760, 8192);
    settings.window_height = settings.window_height.clamp(560, 8192);
    if settings.cache_mib > 1_048_576 || settings.cache_ttl_hours > 87_600 {
        return Err(Error::Invalid(
            "Cache value exceeds the supported limit".into(),
        ));
    }
    {
        app.cache.lock().unwrap().configure(
            settings.cache_mib.saturating_mul(1024 * 1024),
            std::time::Duration::from_secs(settings.cache_ttl_hours.saturating_mul(3600)),
        )?;
    }
    if !app.preview {
        app.paths.save(&settings)?;
    }
    app.recent.configure(false, settings.recent_recipients);
    *app.settings.lock().unwrap() = settings;
    if changes.contains_key("poll_seconds") {
        app.sync.settings_changed();
    }
    publish(app, c.id).await;
    if changes.contains_key("theme") {
        theme(app, c.id).await?;
    }
    Ok(())
}
pub async fn account_update(app: &App, c: &Command) -> Result<()> {
    let _start = if c.data["enabled"] == false {
        Some(app.compose_start.lock().await)
    } else {
        None
    };
    if c.data["enabled"] == false {
        ensure_composer_closed(app, c.text("account"))?;
    }
    let _preferences = app.preferences.lock().await;
    let mut settings = app.settings.lock().unwrap().clone();
    let account = settings
        .accounts
        .iter_mut()
        .find(|a| a.id == c.text("account"))
        .ok_or_else(|| Error::Invalid("Account no longer exists".into()))?;
    if let Some(value) = c.data["name"].as_str() {
        if value.len() > 256 {
            return Err(Error::ResponseLimit);
        }
        account.name = value.trim().into();
    }
    if let Some(value) = c.data["enabled"].as_bool() {
        account.enabled = value;
    }
    if let Some(value) = c.data["notifications"].as_bool() {
        account.notifications = value;
    }
    for (key, target) in [
        ("drive_enabled", &mut account.drive_enabled),
        ("sender_controls", &mut account.sender_controls),
        ("contact_photos", &mut account.contact_photos),
    ] {
        if let Some(value) = c.data[key].as_bool() {
            if value && !*target {
                return Err(Error::Invalid(
                    "Connect this feature with Google first".into(),
                ));
            }
            *target = value;
        }
    }
    if !app.preview {
        app.paths.save(&settings)?;
    }
    *app.settings.lock().unwrap() = settings;
    if !app.preview && c.data.get("enabled").is_some() {
        app.rebuild_clients()?;
    }
    if ["enabled", "drive_enabled", "contact_photos"]
        .iter()
        .any(|key| c.data.get(key).is_some())
    {
        app.photos.reset();
        app.events.emit(c.id, "photosReset", json!({})).await;
    }
    publish(app, c.id).await;
    Ok(())
}
pub async fn reorder(app: &App, c: &Command) -> Result<()> {
    let _preferences = app.preferences.lock().await;
    let ids = c.data["accounts"]
        .as_array()
        .ok_or_else(|| Error::Invalid("Missing account order".into()))?;
    let mut settings = app.settings.lock().unwrap().clone();
    if ids.len() != settings.accounts.len() {
        return Err(Error::Invalid("Include every account exactly once".into()));
    }
    let mut ordered = Vec::new();
    for id in ids {
        let Some(i) = settings
            .accounts
            .iter()
            .position(|a| Some(a.id.as_str()) == id.as_str())
        else {
            return Err(Error::Invalid("Account order is stale".into()));
        };
        ordered.push(settings.accounts.remove(i));
    }
    settings.accounts = ordered;
    if !app.preview {
        app.paths.save(&settings)?;
    }
    *app.settings.lock().unwrap() = settings;
    publish(app, c.id).await;
    Ok(())
}
pub async fn remove(app: &App, c: &Command) -> Result<()> {
    let _start = app.compose_start.lock().await;
    let _lock = app.mutation.lock().await;
    let id = c.text("account");
    ensure_composer_closed(app, id)?;
    if !app
        .settings
        .lock()
        .unwrap()
        .accounts
        .iter()
        .any(|a| a.id == id)
    {
        return Err(Error::Invalid("Account no longer exists".into()));
    }
    // Only a deliberate confirmed removal reaches this command. Never delete mail.
    if !c.flag("confirmed") {
        return Err(Error::Invalid("Confirm account removal first".into()));
    }
    if !app.preview {
        if let Ok(client) = app.client(id) {
            let _ = client.tokens.revoke().await;
        }
        let _ = SystemVault.delete(id).await;
        app.cache.lock().unwrap().clear_account(id)?;
        app.recent.clear_account(id)?;
    }
    let _preferences = app.preferences.lock().await;
    let mut settings = app.settings.lock().unwrap().clone();
    settings.accounts.retain(|a| a.id != id);
    if settings.default_account.as_deref() == Some(id) {
        settings.default_account = None;
    }
    if !app.preview {
        app.paths.save(&settings)?;
    }
    *app.settings.lock().unwrap() = settings;
    if !app.preview {
        app.rebuild_clients()?;
    }
    app.photos.reset();
    app.events.emit(c.id, "photosReset", json!({})).await;
    publish(app, c.id).await;
    Ok(())
}
fn ensure_composer_closed(app: &App, account: &str) -> Result<()> {
    if app.signatures.account_active(account) {
        return Err(Error::Invalid(
            "Close this account’s signature editor before disconnecting it".into(),
        ));
    }
    if let Some(s) = app.composer.lock().unwrap().as_ref() {
        let d = s.data.lock().unwrap();
        if d.form.account.as_deref() == Some(account)
            && !matches!(
                d.phase,
                crate::compose::Phase::Sent | crate::compose::Phase::Closed
            )
        {
            return Err(Error::Invalid(
                "Save or close the open composer before disabling or removing this account".into(),
            ));
        }
    }
    Ok(())
}
pub async fn auth(app: &Arc<App>, c: &Command) -> Result<()> {
    if app.preview {
        return Err(Error::Invalid(
            "Sign-in is disabled in the isolated design preview".into(),
        ));
    }
    if app
        .auth_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(Error::Invalid(
            "Finish the current Google sign-in first".into(),
        ));
    }
    struct Busy(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Busy {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst)
        }
    }
    let _busy = Busy(app.auth_busy.clone());
    let config = crate::registration::client_config()?;
    let existing = app.settings.lock().unwrap().accounts.clone();
    let expected = existing.iter().find(|a| a.id == c.text("account"));
    let auth = Authorization::begin_with_photos(
        &config,
        c.flag("drive") || expected.is_some_and(|a| a.drive_enabled),
        c.flag("settings") || expected.is_some_and(|a| a.sender_controls),
        c.flag("photos") || expected.is_some_and(|a| a.contact_photos),
    )
    .await?;
    app.events
        .emit(
            c.id,
            "authUrl",
            json!({"url":auth.url.as_str(),"expiresSeconds":300}),
        )
        .await;
    let token = auth.complete(&app.http, &config).await?;
    let permissions = token.permissions()?;
    let profile = Gmail::profile_with_token(&app.http, &token.access_token).await?;
    if expected.is_some_and(|a| !a.email.eq_ignore_ascii_case(&profile.email_address)) {
        return Err(Error::Invalid(
            "Reconnect using the same Google account".into(),
        ));
    }
    let _preferences = app.preferences.lock().await;
    let current = app.settings.lock().unwrap().accounts.clone();
    if expected.is_some_and(|a| !current.iter().any(|v| v.id == a.id)) {
        return Err(Error::Invalid("This account was removed while Google sign-in was open. Start sign-in again if you want to reconnect it.".into()));
    }
    let id = current
        .iter()
        .find(|a| a.email.eq_ignore_ascii_case(&profile.email_address))
        .map(|a| a.id.clone())
        .unwrap_or_else(luma_core::composer::new_id);
    if let Some(refresh) = &token.refresh_token {
        SystemVault
            .store(
                &id,
                &Credentials {
                    refresh_token: refresh.clone(),
                    client_id: config.client_id.clone(),
                },
            )
            .await?;
    } else {
        let credential = SystemVault.load(&id).await?;
        if credential.client_id != config.client_id {
            return Err(Error::Reauthenticate);
        }
    }
    let mut settings = app.settings.lock().unwrap().clone();
    if let Some(a) = settings.accounts.iter_mut().find(|a| a.id == id) {
        a.enabled = true;
        a.drive_enabled = permissions.drive;
        a.sender_controls = permissions.settings;
        a.contact_photos = permissions.photos;
    } else {
        settings.accounts.push(Account {
            id: id.clone(),
            email: profile.email_address.clone(),
            name: profile.email_address.split('@').next().unwrap_or("").into(),
            enabled: true,
            notifications: true,
            signature: String::new(),
            drive_enabled: permissions.drive,
            sender_controls: permissions.settings,
            contact_photos: permissions.photos,
            blocked_senders: vec![],
        });
    }
    app.paths.save(&settings)?;
    *app.settings.lock().unwrap() = settings;
    app.rebuild_clients()?;
    app.events.emit(c.id, "photosReset", json!({})).await;
    publish(app, c.id).await;
    app.events
        .emit(c.id, "authComplete", json!({"account":id}))
        .await;
    Ok(())
}
pub async fn theme(app: &App, id: u64) -> Result<()> {
    let theme = app.settings.lock().unwrap().theme.clone();
    if theme != "noctalia" {
        app.events
            .emit(id, "theme", json!({"system":theme=="system","mode":theme}))
            .await;
        return Ok(());
    }
    let bytes = tokio::fs::read(app.paths.config.join("noctalia.toml")).await?;
    if bytes.len() > 8192 {
        return Err(Error::ResponseLimit);
    }
    let p = luma_core::theme::Palette::parse(
        std::str::from_utf8(&bytes)
            .map_err(|_| Error::Invalid("Noctalia palette must be UTF-8".into()))?,
    )?;
    app.events.emit(id,"theme",json!({"mode":p.mode,"background":p.background,"surface":p.surface,"text":p.text,"muted":p.muted,"accent":p.accent,"accentText":p.on_accent,"outline":p.outline,"hover":p.hover})).await;
    Ok(())
}
