//! Encryption for secrets kept in the database, such as the AI provider's API key.
//!
//! AES-256-GCM with a key derived (HKDF-SHA256) from the server's token-signing secret,
//! which lives in the data directory (or `LARDER_JWT_SECRET`), not in the database. A copy
//! of the database alone, such as a PostgreSQL dump, therefore does not reveal the secrets.
//! If the signing secret changes, stored secrets can no longer be read and must be entered again.

use anyhow::anyhow;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::hkdf;
use ring::rand::{SecureRandom, SystemRandom};

const PREFIX: &str = "v1:";
const TAG_LEN: usize = 16;

pub struct SecretBox {
    key: LessSafeKey,
    rng: SystemRandom,
}

struct KeyLen(usize);

impl hkdf::KeyType for KeyLen {
    fn len(&self) -> usize {
        self.0
    }
}

impl SecretBox {
    pub fn new(master: &[u8]) -> Self {
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, b"larder-database-secrets-v1").extract(master);
        let mut bytes = [0u8; 32];
        prk.expand(&[b"aes-256-gcm"], KeyLen(bytes.len()))
            .and_then(|okm| okm.fill(&mut bytes))
            .expect("HKDF output of 32 bytes is always valid");
        let key = UnboundKey::new(&AES_256_GCM, &bytes).expect("a 32-byte AES-256 key is always valid");
        Self { key: LessSafeKey::new(key), rng: SystemRandom::new() }
    }

    /// Encrypts `plain`, bound to `context` (for example the setting's name) so a value
    /// cannot be moved to another setting.
    pub fn seal(&self, plain: &str, context: &str) -> anyhow::Result<String> {
        let mut nonce = [0u8; NONCE_LEN];
        self.rng.fill(&mut nonce).map_err(|_| anyhow!("no random numbers available"))?;
        let mut buf = plain.as_bytes().to_vec();
        self.key
            .seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(context.as_bytes()), &mut buf)
            .map_err(|_| anyhow!("encryption failed"))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&buf);
        Ok(format!("{PREFIX}{}", hex::encode(out)))
    }

    /// Decrypts a value made by [`seal`](Self::seal) with the same context; `None` if it was
    /// sealed with another key or context, or has been altered.
    pub fn open(&self, sealed: &str, context: &str) -> Option<String> {
        let raw = hex::decode(sealed.strip_prefix(PREFIX)?).ok()?;
        if raw.len() < NONCE_LEN + TAG_LEN {
            return None;
        }
        let (nonce, ciphertext) = raw.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).ok()?;
        let mut buf = ciphertext.to_vec();
        let plain = self.key.open_in_place(nonce, Aad::from(context.as_bytes()), &mut buf).ok()?;
        String::from_utf8(plain.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper_detection() {
        let b = SecretBox::new(b"signing-secret-one");
        let sealed = b.seal("sk-ant-example-key", "ai.api_key").unwrap();
        assert!(sealed.starts_with("v1:"));
        assert!(!sealed.contains("sk-ant"));
        assert_eq!(b.open(&sealed, "ai.api_key").as_deref(), Some("sk-ant-example-key"));
        // fresh nonce every time
        assert_ne!(sealed, b.seal("sk-ant-example-key", "ai.api_key").unwrap());
        // wrong context, wrong key, altered or malformed input
        assert!(b.open(&sealed, "other").is_none());
        assert!(SecretBox::new(b"signing-secret-two").open(&sealed, "ai.api_key").is_none());
        let mut altered = sealed.clone().into_bytes();
        let last = altered.len() - 1;
        altered[last] = if altered[last] == b'0' { b'1' } else { b'0' };
        assert!(b.open(std::str::from_utf8(&altered).unwrap(), "ai.api_key").is_none());
        assert!(b.open("v1:zz", "ai.api_key").is_none());
        assert!(b.open("plain", "ai.api_key").is_none());
    }
}
