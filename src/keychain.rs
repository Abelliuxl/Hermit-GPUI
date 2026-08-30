use anyhow::Result;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

/// Keychain-backed credential storage (generic passwords), mirroring the
/// SwiftUI CredentialStore.
pub struct CredentialStore {
    service: String,
}

impl CredentialStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    pub fn save(&self, account: &str, secret: &str) -> Result<()> {
        match get_generic_password(&self.service, account) {
            Ok(_) => {
                set_generic_password(&self.service, account, secret.as_bytes())?;
            }
            Err(_) => {
                set_generic_password(&self.service, account, secret.as_bytes())?;
            }
        }
        Ok(())
    }

    pub fn read(&self, account: &str) -> Result<Option<String>> {
        match get_generic_password(&self.service, account) {
            Ok(bytes) => Ok(Some(String::from_utf8(bytes)?)),
            Err(_) => Ok(None),
        }
    }

    pub fn delete(&self, account: &str) -> Result<()> {
        match delete_generic_password(&self.service, account) {
            Ok(()) => Ok(()),
            Err(err) => {
                // ItemNotFound should not be an error for our purposes.
                let code = err.code();
                if code == -25300 {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("keychain delete failed: {code}"))
                }
            }
        }
    }
}

pub fn main_credential_store() -> CredentialStore {
    CredentialStore::new("HermitGPUI.HermesBackend")
}

pub fn openclaw_device_store() -> CredentialStore {
    CredentialStore::new("HermitGPUI.OpenClawDevice")
}
