//! Windows development/acceptance builds use the OS credential vault. Tokens
//! never fall back to settings, environment variables or plaintext files.
use super::{Credentials, SystemVault, Vault};
use crate::{Error, Result};
use async_trait::async_trait;
use std::ptr;
use windows_sys::Win32::{
    Foundation::{ERROR_NOT_FOUND, GetLastError},
    Security::Credentials::{
        CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW,
        CredDeleteW, CredFree, CredReadW, CredWriteW,
    },
};
use zeroize::Zeroizing;

fn target(account: &str) -> Result<Vec<u16>> {
    if account.is_empty() || account.len() > 256 || account.contains('\0') {
        return Err(Error::Keyring("Invalid account identifier".into()));
    }
    Ok(format!("io.github.lumamail.Luma/{account}\0")
        .encode_utf16()
        .collect())
}

fn failure(operation: &str, code: u32) -> Error {
    Error::Keyring(format!(
        "Windows Credential Manager could not {operation} this account (OS error {code})"
    ))
}

struct Stored(*mut CREDENTIALW);
impl Drop for Stored {
    fn drop(&mut self) {
        // SAFETY: CredReadW returned this allocation. It remains owned here until
        // CredFree; scrub the bounded secret payload before freeing it once.
        unsafe {
            let record = &mut *self.0;
            if !record.CredentialBlob.is_null()
                && record.CredentialBlobSize <= CRED_MAX_CREDENTIAL_BLOB_SIZE
            {
                ptr::write_bytes(record.CredentialBlob, 0, record.CredentialBlobSize as usize);
            }
            CredFree(self.0.cast());
        }
    }
}

#[async_trait]
impl Vault for SystemVault {
    async fn load(&self, account: &str) -> Result<Credentials> {
        let target = target(account)?;
        let mut value = ptr::null_mut();
        // SAFETY: target is NUL terminated and value is a valid out pointer.
        if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut value) } == 0 {
            let code = unsafe { GetLastError() };
            return Err(if code == ERROR_NOT_FOUND {
                Error::Reauthenticate
            } else {
                failure("read", code)
            });
        }
        if value.is_null() {
            return Err(Error::Keyring(
                "Windows returned an empty credential".into(),
            ));
        }
        let stored = Stored(value);
        // SAFETY: allocation is owned by stored until after deserialization.
        let record = unsafe { &*stored.0 };
        if record.CredentialBlob.is_null()
            || record.CredentialBlobSize == 0
            || record.CredentialBlobSize > CRED_MAX_CREDENTIAL_BLOB_SIZE
        {
            return Err(Error::Reauthenticate);
        }
        let data = unsafe {
            std::slice::from_raw_parts(record.CredentialBlob, record.CredentialBlobSize as usize)
        };
        serde_json::from_slice(data).map_err(|_| Error::Reauthenticate)
    }

    async fn store(&self, account: &str, value: &Credentials) -> Result<()> {
        let mut target = target(account)?;
        let mut user: Vec<u16> = "Google OAuth\0".encode_utf16().collect();
        let mut data = Zeroizing::new(serde_json::to_vec(value)?);
        if data.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
            return Err(Error::Keyring(
                "Google credential exceeds the Windows vault limit".into(),
            ));
        }
        let record = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            UserName: user.as_mut_ptr(),
            CredentialBlob: data.as_mut_ptr(),
            CredentialBlobSize: data.len() as u32,
            // Persist only for this Windows user on this machine, not roaming.
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..unsafe { std::mem::zeroed() }
        };
        // SAFETY: pointers refer to live buffers until this synchronous call
        // finishes. The OS copies the credential; data is zeroized on return.
        if unsafe { CredWriteW(&record, 0) } == 0 {
            return Err(failure("save", unsafe { GetLastError() }));
        }
        Ok(())
    }

    async fn delete(&self, account: &str) -> Result<()> {
        let target = target(account)?;
        // SAFETY: target is NUL terminated. Delete only our exact target/type.
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
            let code = unsafe { GetLastError() };
            if code != ERROR_NOT_FOUND {
                return Err(failure("remove", code));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "writes a synthetic credential to the current Windows user's OS vault"]
    async fn windows_vault_round_trip_isolated() {
        let account = format!("acceptance-{}", uuid::Uuid::new_v4());
        let vault = SystemVault;
        assert!(matches!(
            vault.load(&account).await,
            Err(Error::Reauthenticate)
        ));
        let first = Credentials {
            refresh_token: "synthetic-one".into(),
            client_id: "test-client".into(),
        };
        vault.store(&account, &first).await.unwrap();
        let stored = vault.load(&account).await.unwrap();
        assert_eq!(stored.refresh_token, first.refresh_token);
        assert_eq!(stored.client_id, first.client_id);
        let updated = Credentials {
            refresh_token: "synthetic-two".into(),
            client_id: "test-client-two".into(),
        };
        vault.store(&account, &updated).await.unwrap();
        assert_eq!(
            vault.load(&account).await.unwrap().refresh_token,
            updated.refresh_token
        );
        vault.delete(&account).await.unwrap();
        assert!(matches!(
            vault.load(&account).await,
            Err(Error::Reauthenticate)
        ));
        vault.delete(&account).await.unwrap();
    }
}
