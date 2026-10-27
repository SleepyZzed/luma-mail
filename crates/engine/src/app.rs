use crate::protocol::{Command, Events};
use luma_core::{
    Error, Result,
    auth::{SystemVault, TokenManager},
    cache::Cache,
    config::{Paths, Settings},
    gmail::Gmail,
    http::Http,
    model::*,
    pagination::UnifiedPager,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

pub struct View {
    pub folder: Folder,
    pub search: String,
    pub accounts: Option<HashSet<String>>,
    pub rows: Vec<Conversation>,
    pub selected: Option<ThreadKey>,
    pub thread: Option<Thread>,
    pub bodies: crate::resident::Bodies,
}
pub struct App {
    pub events: Events,
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    pub http: Http,
    pub cache: Arc<Mutex<Cache>>,
    pub clients: Mutex<HashMap<String, Arc<Gmail>>>,
    pub view: Mutex<View>,
    pub pager: tokio::sync::Mutex<Option<UnifiedPager>>,
    pub generation: AtomicU64,
    pub reading: AtomicU64,
    pub mutation: tokio::sync::Mutex<()>,
    pub mutation_serial: AtomicU64,
    pub undo: Mutex<Vec<crate::mail::Undo>>,
    pub preview: bool,
    pub auth_busy: Arc<AtomicBool>,
    pub recent: luma_core::local_compose::Store,
    pub composer: Mutex<Option<Arc<crate::compose::Session>>>,
    pub compose_start: tokio::sync::Mutex<()>,
    pub photos: Arc<crate::photos::Service>,
    pub reader_assets: Mutex<crate::reader::Assets>,
    pub render_lock: tokio::sync::Mutex<()>,
    pub quit_pending: AtomicBool,
    jobs: Mutex<HashMap<u64, (String, tokio::task::AbortHandle)>>,
    slots: Arc<tokio::sync::Semaphore>,
}
impl App {
    pub async fn open(events: Events) -> Result<Arc<Self>> {
        let paths = Paths::discover()?;
        let preview = std::env::args().any(|a| a == "--design-preview");
        if preview && std::env::var_os("LUMA_TEST_ROOT").is_none() {
            return Err(Error::Invalid(
                "Preview requires isolated LUMA_TEST_ROOT".into(),
            ));
        }
        let mut settings = if preview {
            Settings::default()
        } else {
            paths.load()?
        };
        if preview {
            settings.cache_mib = 0;
            if let Ok(theme) = std::env::var("LUMA_PREVIEW_THEME")
                && ["system", "dark", "light", "noctalia"].contains(&theme.as_str())
            {
                settings.theme = theme;
            }
            settings.accounts.push(luma_core::config::Account {
                id: "preview".into(),
                email: "alex@example.com".into(),
                name: "Alex Morgan".into(),
                enabled: true,
                notifications: false,
                signature: String::new(),
                drive_enabled: false,
                sender_controls: false,
                contact_photos: false,
                blocked_senders: vec![],
            });
            if std::env::var("LUMA_PREVIEW_TWO_ACCOUNTS").is_ok_and(|v| v == "1") {
                settings.accounts.push(luma_core::config::Account {
                    id: "preview-b".into(),
                    email: "jamie@example.com".into(),
                    name: "Jamie Taylor".into(),
                    enabled: true,
                    notifications: false,
                    signature: String::new(),
                    drive_enabled: false,
                    sender_controls: false,
                    contact_photos: false,
                    blocked_senders: vec![],
                });
            }
        }
        let cache = Cache::new(
            paths.cache.join("bodies"),
            settings.cache_mib.saturating_mul(1024 * 1024),
            Duration::from_secs(settings.cache_ttl_hours.saturating_mul(3600)),
        )?;
        let recent = luma_core::local_compose::Store::new(&paths.config);
        recent.configure(false, settings.recent_recipients);
        recent.set_accounts(settings.accounts.iter().map(|a| a.id.clone()));
        let app = Arc::new(Self {
            events,
            paths,
            settings: Mutex::new(settings),
            http: Http::new()?,
            cache: Arc::new(Mutex::new(cache)),
            clients: Mutex::new(HashMap::new()),
            view: Mutex::new(View {
                folder: Folder::Inbox,
                search: String::new(),
                accounts: None,
                rows: vec![],
                selected: None,
                thread: None,
                bodies: crate::resident::Bodies::default(),
            }),
            pager: tokio::sync::Mutex::new(None),
            generation: AtomicU64::new(0),
            reading: AtomicU64::new(0),
            mutation: tokio::sync::Mutex::new(()),
            mutation_serial: AtomicU64::new(0),
            undo: Mutex::new(vec![]),
            preview,
            auth_busy: Arc::new(AtomicBool::new(false)),
            recent,
            composer: Mutex::new(None),
            compose_start: tokio::sync::Mutex::new(()),
            photos: Arc::new(crate::photos::Service::default()),
            reader_assets: Mutex::new(crate::reader::Assets::default()),
            render_lock: tokio::sync::Mutex::new(()),
            quit_pending: AtomicBool::new(false),
            jobs: Mutex::new(HashMap::new()),
            slots: Arc::new(tokio::sync::Semaphore::new(32)),
        });
        app.rebuild_clients()?;
        crate::photos::Service::start(&app);
        Ok(app)
    }
    pub fn rebuild_clients(&self) -> Result<()> {
        self.photos.reset();
        let settings = self.settings.lock().unwrap();
        let mut clients = self.clients.lock().unwrap();
        clients.clear();
        if settings.accounts.is_empty() || self.preview {
            return Ok(());
        }
        let config = crate::registration::client_config()?;
        for a in settings.accounts.iter().filter(|a| a.enabled) {
            let tokens = Arc::new(TokenManager::new(
                a.id.clone(),
                config.clone(),
                self.http.clone(),
                Arc::new(SystemVault),
            ));
            clients.insert(
                a.id.clone(),
                Arc::new(Gmail::new(a.id.clone(), self.http.clone(), tokens)),
            );
        }
        Ok(())
    }
    pub fn client(&self, account: &str) -> Result<Arc<Gmail>> {
        self.clients
            .lock()
            .unwrap()
            .get(account)
            .cloned()
            .ok_or_else(|| Error::Invalid("Choose a connected account".into()))
    }
    pub async fn boot(&self) {
        let data = {
            let s = self.settings.lock().unwrap();
            json!({"settings":*s,"configured":crate::registration::client_config().is_ok(),"preview":self.preview,"version":env!("CARGO_PKG_VERSION"),"configPath":self.paths.config.to_string_lossy()})
        };
        self.events.emit(0, "ready", data).await;
        if self.preview {
            crate::mail::fixture(self).await;
        }
    }
    pub fn dispatch(self: Arc<Self>, command: Command) {
        if command.action == "cancel" {
            let request = command.data["request"].as_u64().unwrap_or(0);
            let mut jobs = self.jobs.lock().unwrap();
            // Never abort a provider write after it may have reached Gmail.
            let cancellable = jobs.get(&request).is_some_and(|(kind, _)| {
                matches!(
                    kind.as_str(),
                    "view" | "page" | "select" | "message" | "authStart"
                )
            });
            if cancellable && let Some((_, job)) = jobs.remove(&request) {
                job.abort();
            }
            return;
        }
        if matches!(command.action.as_str(), "mutate" | "undo") {
            self.mutation_serial.fetch_add(1, Ordering::SeqCst);
        }
        let permit = match self.slots.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                let e = self.events.clone();
                tokio::spawn(async move {
                    e.error(command.id, "Finishing pending requests; try again shortly")
                        .await;
                });
                return;
            }
        };
        let id = command.id;
        // Form revisions and write intents are accepted in pipe order, before
        // spawning asynchronous work. A delayed update cannot overtake Send.
        let compose_prepared = command
            .action
            .starts_with("compose")
            .then(|| crate::compose_commands::prepare(&self, &command));
        let owner = self.clone();
        let action = command.action.clone();
        let mut jobs = self.jobs.lock().unwrap();
        if matches!(action.as_str(), "view" | "select" | "releaseReader") {
            let cancel: Vec<_> = jobs
                .iter()
                .filter(|(_, (kind, _))| {
                    if action == "view" {
                        matches!(
                            kind.as_str(),
                            "view" | "page" | "select" | "message" | "renderMessage"
                        )
                    } else {
                        matches!(kind.as_str(), "select" | "message" | "renderMessage")
                    }
                })
                .map(|(id, _)| *id)
                .collect();
            for old in cancel {
                if let Some((_, handle)) = jobs.remove(&old) {
                    handle.abort()
                }
            }
            if action == "view" {
                self.generation.store(id, Ordering::SeqCst);
            }
            self.reading.store(id, Ordering::SeqCst);
            self.reader_assets.lock().unwrap().clear();
        }
        let task = tokio::spawn(async move {
            let _permit = permit;
            let result = async {
                if let Some(prepared) = compose_prepared {
                    return crate::compose_commands::run(&owner, &command, prepared?).await;
                }
                match command.action.as_str() {
                    "boot" => {
                        owner.boot().await;
                        Ok(())
                    }
                    "view" => crate::mail::change_view(&owner, &command).await,
                    "page" => crate::mail::page(&owner, command.id, false).await,
                    "select" => crate::mail::open(&owner, &command).await,
                    "message" => crate::mail::body(&owner, &command).await,
                    "renderMessage" => crate::reader::render(&owner, &command).await,
                    "mutate" => crate::mail::mutate(&owner, &command).await,
                    "undo" => crate::mail::undo(&owner, command.id).await,
                    "authStart" => crate::accounts::auth(&owner, &command).await,
                    "settingsUpdate" => crate::accounts::update(&owner, &command).await,
                    "accountUpdate" => crate::accounts::account_update(&owner, &command).await,
                    "accountReorder" => crate::accounts::reorder(&owner, &command).await,
                    "accountRemove" => crate::accounts::remove(&owner, &command).await,
                    "labels" => crate::accounts::labels(&owner, &command).await,
                    "themeReload" => crate::accounts::theme(&owner, command.id).await,
                    "photoRequest" => owner.photos.request(&owner, &command).await,
                    "photoCancel" => {
                        owner.photos.cancel(command.text("token"));
                        Ok(())
                    }
                    "photosReset" => {
                        owner.photos.reset();
                        owner.events.emit(id, "photosReset", json!({})).await;
                        Ok(())
                    }
                    "recentClear" => {
                        let store = owner.recent.clone();
                        tokio::task::spawn_blocking(move || store.clear_recent())
                            .await
                            .map_err(|_| {
                                Error::Invalid("Could not clear recent recipients".into())
                            })??;
                        owner.events.emit(id, "recentCleared", json!({})).await;
                        Ok(())
                    }
                    "cacheClear" => {
                        if let Some(account) = command.data["account"].as_str() {
                            owner.cache.lock().unwrap().clear_account(account)?;
                        } else {
                            owner.cache.lock().unwrap().clear()?;
                        }
                        let bytes = owner.cache.lock().unwrap().usage()?;
                        owner
                            .events
                            .emit(id, "cacheUsage", json!({"bytes":bytes}))
                            .await;
                        Ok(())
                    }
                    "releaseReader" => {
                        if owner.reading.load(Ordering::SeqCst) != id {
                            return Ok(());
                        }
                        let mut v = owner.view.lock().unwrap();
                        v.thread = None;
                        v.bodies.clear();
                        v.selected = None;
                        Ok(())
                    }
                    "cacheUsage" => {
                        let usage = owner.cache.lock().unwrap().usage()?;
                        owner
                            .events
                            .emit(id, "cacheUsage", json!({"bytes":usage}))
                            .await;
                        Ok(())
                    }
                    _ => Err(Error::Invalid(format!(
                        "Unknown application command: {}",
                        command.action
                    ))),
                }
            }
            .await;
            if let Err(e) = result {
                owner.events.error(id, &e.to_string()).await;
            }
            owner.jobs.lock().unwrap().remove(&id);
            Ok::<(), Error>(())
        });
        jobs.insert(id, (action, task.abort_handle()));
    }
    pub fn protect_reader_write(&self, id: u64, reading: u64, serial: u64) -> bool {
        let mut jobs = self.jobs.lock().unwrap();
        if self.reading.load(Ordering::SeqCst) != reading
            || self.mutation_serial.load(Ordering::SeqCst) != serial
        {
            return false;
        }
        if let Some((kind, _)) = jobs.get_mut(&id) {
            *kind = "markRead".into();
            true
        } else {
            false
        }
    }
    pub async fn shutdown(&self) {
        self.photos.stop();
        for (_, (_, handle)) in self.jobs.lock().unwrap().drain() {
            handle.abort()
        }
        if self.settings.lock().unwrap().clear_cache_on_exit {
            let _ = self.cache.lock().unwrap().clear();
        }
    }
    pub fn current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }
    pub async fn status(&self, id: u64, kind: &str, text: &str) {
        self.events
            .emit(id, "activity", json!({"kind":kind,"text":text}))
            .await;
    }
}

pub fn folder(value: &str) -> Result<Folder> {
    Ok(match value {
        "inbox" => Folder::Inbox,
        "all_mail" => Folder::AllMail,
        "unread" => Folder::Unread,
        "starred" => Folder::Starred,
        "sent" => Folder::Sent,
        "drafts" => Folder::Drafts,
        "archive" => Folder::Archive,
        "trash" => Folder::Trash,
        "spam" => Folder::Spam,
        s if s.starts_with("label:") => Folder::Label(s[6..].into()),
        _ => return Err(Error::Invalid("Unknown folder".into())),
    })
}
pub fn key(value: &Value) -> Result<ThreadKey> {
    let account = value["account"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::Invalid("Missing account identity".into()))?;
    let id = value["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::Invalid("Missing message identity".into()))?;
    Ok(ThreadKey {
        account: account.into(),
        id: id.into(),
    })
}
