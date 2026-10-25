//! Online-first mail services. No mailbox database or background history crawler.
pub mod auth;
pub mod cache;
pub mod composer;
pub mod config;
pub mod drive;
pub mod gmail;
pub mod html_mail;
pub mod http;
pub mod mail_images;
pub mod model;
pub mod notifications;
pub mod pagination;
pub mod preview;
pub mod provider;
mod rate;
pub mod search;
pub mod theme;

pub use error::{Error, Result};
#[cfg(test)]
mod tests;

mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("Network request failed; check your connection")]
        Network(#[from] reqwest::Error),
        #[error("Storage error: {0}")]
        Io(#[from] std::io::Error),
        #[error("Invalid server response")]
        Json(#[from] serde_json::Error),
        #[error("Google returned HTTP {status} ({reason})")]
        Api { status: u16, reason: String },
        #[error("This account needs to be reconnected in Settings")]
        Reauthenticate,
        #[error("Keyring unavailable: {0}")]
        Keyring(String),
        #[error("{0}")]
        Invalid(String),
        #[error("The operation timed out")]
        Timeout,
        #[error("Server response exceeds the safety limit")]
        ResponseLimit,
        #[error("Draft changed in Gmail. Save a separate draft to preserve both versions.")]
        DraftConflict,
    }
    impl Error {
        pub fn write_may_have_succeeded(&self) -> bool {
            matches!(
                self,
                Self::Network(_)
                    | Self::Json(_)
                    | Self::ResponseLimit
                    | Self::Api {
                        status: 500..=599,
                        ..
                    }
            )
        }
    }
    pub type Result<T> = std::result::Result<T, Error>;
}
