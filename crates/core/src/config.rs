use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftMode {
    #[default]
    Ask,
    OnClose,
    Manual,
}
impl DraftMode {
    pub fn saves_on_close(self, recipient: bool, body: bool) -> bool {
        self == Self::OnClose && recipient && body
    }
}

#[cfg(test)]
mod draft_mode_tests {
    use super::*;
    #[test]
    fn old_settings_ask_and_only_on_close_with_both_fields_can_save_automatically() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.draft_mode, DraftMode::Ask);
        for mode in [DraftMode::Ask, DraftMode::OnClose, DraftMode::Manual] {
            for recipient in [false, true] {
                for body in [false, true] {
                    assert_eq!(
                        mode.saves_on_close(recipient, body),
                        mode == DraftMode::OnClose && recipient && body
                    );
                }
            }
            let mut settings = settings.clone();
            settings.draft_mode = mode;
            let reopened: Settings =
                serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(reopened.draft_mode, mode);
            assert!(!reopened.local_drafts);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub name: String,
    pub enabled: bool,
    pub notifications: bool,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub drive_enabled: bool,
    #[serde(default)]
    pub sender_controls: bool,
    #[serde(default)]
    pub contact_photos: bool,
    #[serde(default)]
    pub blocked_senders: Vec<BlockedSender>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockedSender {
    pub address: String,
    pub filter_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub accounts: Vec<Account>,
    pub default_account: Option<String>,
    pub cache_mib: u64,
    pub cache_ttl_hours: u64,
    pub clear_cache_on_exit: bool,
    pub theme: String,
    pub compact: bool,
    pub ui_scale_percent: u16,
    pub reduced_motion: bool,
    pub window_chrome: bool,
    pub poll_seconds: u64,
    #[serde(default)]
    pub poll_version: u8,
    pub notification_privacy: bool,
    pub auto_load_images: bool,
    pub auto_preview_attachments: bool,
    pub close_to_tray: bool,
    pub draft_mode: DraftMode,
    // Old development builds wrote recovery copies. Retain their reader, but
    // never resume local autosaving from an old preference file.
    #[serde(skip)]
    pub local_drafts: bool,
    pub recent_recipients: bool,
    pub window_width: i32,
    pub window_height: i32,
    pub list_width: i32,
    pub sidebar_width: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            accounts: vec![],
            default_account: None,
            cache_mib: 100,
            cache_ttl_hours: 24,
            clear_cache_on_exit: false,
            theme: "system".into(),
            compact: false,
            ui_scale_percent: 100,
            reduced_motion: false,
            window_chrome: false,
            poll_seconds: 60,
            poll_version: 1,
            notification_privacy: false,
            auto_load_images: false,
            auto_preview_attachments: true,
            close_to_tray: true,
            draft_mode: DraftMode::Ask,
            local_drafts: false,
            recent_recipients: true,
            window_width: 1320,
            window_height: 840,
            list_width: 390,
            sidebar_width: 230,
        }
    }
}
impl Settings {
    pub fn hides_on_close(&self, tray_available: bool, quitting: bool) -> bool {
        self.close_to_tray && tray_available && !quitting
    }
}

#[derive(Clone, Debug)]
pub struct Paths {
    pub config: PathBuf,
    pub cache: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        // Explicit, isolated launch-test paths; normal desktop runs use XDG.
        if let Some(root) = std::env::var_os("LUMA_TEST_ROOT") {
            let root = PathBuf::from(root);
            return Ok(Self {
                config: root.join("config"),
                cache: root.join("cache"),
            });
        }
        let dirs = directories::ProjectDirs::from("", "", "luma-mail")
            .ok_or_else(|| Error::Invalid("Cannot find the user's XDG directories".into()))?;
        Ok(Self {
            config: dirs.config_dir().into(),
            cache: dirs.cache_dir().into(),
        })
    }
    pub fn load(&self) -> Result<Settings> {
        match fs::read(self.config.join("settings.json")) {
            Ok(data) => {
                let mut settings: Settings = serde_json::from_slice(&data)?;
                if settings.poll_version == 0 && settings.poll_seconds == 120 {
                    settings.poll_seconds = 60;
                }
                settings.poll_version = 1;
                settings.ui_scale_percent = settings.ui_scale_percent.clamp(75, 200);
                Ok(settings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, settings: &Settings) -> Result<()> {
        private_dir(&self.config)?;
        let temp = self.config.join("settings.json.tmp");
        fs::write(&temp, serde_json::to_vec_pretty(settings)?)?;
        private_file(&temp)?;
        // Settings never contain tokens or a mailbox mirror.
        fs::rename(temp, self.config.join("settings.json"))?;
        Ok(())
    }
}

pub fn private_dir(path: &std::path::Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn private_file(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
