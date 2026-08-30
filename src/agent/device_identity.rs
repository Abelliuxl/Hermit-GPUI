use crate::keychain::openclaw_device_store;
use anyhow::{anyhow, Result};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;

/// Stable Ed25519 device identity persisted in the Keychain, used to answer
/// the OpenClaw Gateway connect challenge (port of OpenClawDeviceIdentity).
pub struct OpenClawDeviceIdentity {
    pub device_id: String,
    pub public_key: String,
    signing_key: SigningKey,
}

impl OpenClawDeviceIdentity {
    pub fn load_or_create() -> Result<Self> {
        let store = openclaw_device_store();
        let account = "ed25519-private-key";
        let signing_key = match store.read(account)? {
            Some(encoded) => {
                let raw = base64::engine::general_purpose::STANDARD.decode(encoded)?;
                let bytes: [u8; 32] = raw
                    .as_slice()
                    .try_into()
                    .map_err(|_| anyhow!("Invalid stored OpenClaw device key."))?;
                SigningKey::from_bytes(&bytes)
            }
            None => {
                let mut seed = [0u8; 32];
                use rand::RngCore;
                rand::rngs::OsRng.fill_bytes(&mut seed);
                let key = SigningKey::from_bytes(&seed);
                seed.fill(0);
                store.save(
                    account,
                    &base64::engine::general_purpose::STANDARD.encode(key.to_bytes()),
                )?;
                key
            }
        };

        let public = signing_key.verifying_key().to_bytes();
        let fingerprint = {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(public);
            digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };

        Ok(Self {
            device_id: fingerprint,
            public_key: base64_url(&public),
            signing_key,
        })
    }

    pub fn signed_connect_device(
        &self,
        nonce: &str,
        client_id: &str,
        client_mode: &str,
        role: &str,
        scopes: &[&str],
        token: &str,
    ) -> serde_json::Value {
        let signed_at = crate::models::now_millis();
        let payload = [
            "v2",
            &self.device_id,
            client_id,
            client_mode,
            role,
            &scopes.join(","),
            &signed_at.to_string(),
            token,
            nonce,
        ]
        .join("|");
        let signature = self.signing_key.sign(payload.as_bytes());
        json!({
            "id": self.device_id,
            "publicKey": self.public_key,
            "signature": base64_url(&signature.to_bytes()),
            "signedAt": signed_at,
            "nonce": nonce
        })
    }
}

fn base64_url(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD
        .encode(data)
        .replace('+', "-")
        .replace('/', "_")
        .replace('=', "")
}
