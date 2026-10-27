//! Installed desktop applications use a public OAuth client registration.
//! Only the maintainer configures it, before compiling the distributable.
use crate::{Result, auth::ClientConfig};

const CLIENT_ID: &str = match option_env!("LUMA_GOOGLE_CLIENT_ID") {
    Some(value) => value,
    None => "",
};
const CLIENT_SECRET: &str = match option_env!("LUMA_GOOGLE_CLIENT_SECRET") {
    Some(value) => value,
    None => "",
};

#[cfg(feature = "require-google-client")]
const _: () = assert!(
    !CLIENT_ID.is_empty() && !CLIENT_SECRET.is_empty(),
    "Release builds require LUMA_GOOGLE_CLIENT_ID and LUMA_GOOGLE_CLIENT_SECRET from the maintainer's Google Desktop app registration"
);

pub fn client_config() -> Result<ClientConfig> {
    ClientConfig::from_registration(CLIENT_ID, CLIENT_SECRET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_registration_is_valid_or_deliberately_unconfigured() {
        if CLIENT_ID.is_empty() && CLIENT_SECRET.is_empty() {
            assert!(client_config().is_err());
        } else {
            assert!(
                client_config().is_ok(),
                "Invalid or incomplete build-time Google app registration"
            );
        }
    }
}
