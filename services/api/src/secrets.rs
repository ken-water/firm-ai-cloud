use std::env;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};

use crate::error::{AppError, AppResult};

pub const MONITORING_INLINE_SECRET_POLICY_ALLOW: &str = "allow";
pub const MONITORING_INLINE_SECRET_POLICY_FORBID: &str = "forbid";

const SECRET_ENV_PREFIX: &str = "env:";
const SECRET_PLAIN_PREFIX: &str = "plain:";
const SECRET_ENVELOPE_PREFIX: &str = "enc:v1:";
const SECRET_REF_ENCRYPTED_SENTINEL: &str = "enc:v1";
const MAX_SECRET_REF_LEN: usize = 255;
const MAX_INLINE_SECRET_LEN: usize = 4096;

#[derive(Debug, Clone)]
pub struct StoredMonitoringSecret {
    pub secret_ref: String,
    pub secret_ciphertext: Option<String>,
}

pub fn prepare_monitoring_secret_for_storage(
    input: &str,
    inline_policy: &str,
    encryption_key_b64: Option<&str>,
) -> AppResult<StoredMonitoringSecret> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AppError::Validation("secret_ref is required".to_string()));
    }

    if let Some(env_key) = input.strip_prefix(SECRET_ENV_PREFIX) {
        let env_key = env_key.trim();
        if env_key.is_empty() {
            return Err(AppError::Validation(
                "secret_ref env key is empty".to_string(),
            ));
        }
        let normalized_ref = format!("{SECRET_ENV_PREFIX}{env_key}");
        if normalized_ref.len() > MAX_SECRET_REF_LEN {
            return Err(AppError::Validation(format!(
                "secret_ref env key length must be <= {}",
                MAX_SECRET_REF_LEN - SECRET_ENV_PREFIX.len()
            )));
        }
        return Ok(StoredMonitoringSecret {
            secret_ref: normalized_ref,
            secret_ciphertext: None,
        });
    }

    if input == SECRET_REF_ENCRYPTED_SENTINEL || input.starts_with(SECRET_ENVELOPE_PREFIX) {
        return Err(AppError::Validation(
            "secret_ref must be provided as env:KEY or plain/raw secret; encrypted envelope is managed by server"
                .to_string(),
        ));
    }

    let policy = inline_policy.trim().to_ascii_lowercase();
    if policy == MONITORING_INLINE_SECRET_POLICY_FORBID {
        return Err(AppError::Validation(
            "inline secret_ref is disabled by MONITORING_SECRET_INLINE_POLICY=forbid; use env:YOUR_SECRET_ENV instead"
                .to_string(),
        ));
    }
    if policy != MONITORING_INLINE_SECRET_POLICY_ALLOW {
        return Err(AppError::Validation(
            "MONITORING_SECRET_INLINE_POLICY must be one of: allow, forbid".to_string(),
        ));
    }

    let inline_secret = input
        .strip_prefix(SECRET_PLAIN_PREFIX)
        .map(str::trim)
        .unwrap_or(input);
    if inline_secret.is_empty() {
        return Err(AppError::Validation(
            "secret_ref plain value is empty".to_string(),
        ));
    }
    if inline_secret.len() > MAX_INLINE_SECRET_LEN {
        return Err(AppError::Validation(format!(
            "inline secret length must be <= {MAX_INLINE_SECRET_LEN}"
        )));
    }

    let ciphertext = encrypt_secret(inline_secret, encryption_key_b64)?;
    Ok(StoredMonitoringSecret {
        secret_ref: SECRET_REF_ENCRYPTED_SENTINEL.to_string(),
        secret_ciphertext: Some(ciphertext),
    })
}

pub fn resolve_monitoring_secret(
    secret_ref: &str,
    secret_ciphertext: Option<&str>,
    encryption_key_b64: Option<&str>,
) -> AppResult<String> {
    if let Some(secret_ciphertext) = trim_optional_str(secret_ciphertext) {
        return decrypt_secret(secret_ciphertext, encryption_key_b64);
    }

    let secret_ref = secret_ref.trim();
    if secret_ref.is_empty() {
        return Err(AppError::Validation("secret_ref is empty".to_string()));
    }

    if let Some(key) = secret_ref.strip_prefix(SECRET_ENV_PREFIX) {
        let key = key.trim();
        if key.is_empty() {
            return Err(AppError::Validation(
                "secret_ref env key is empty".to_string(),
            ));
        }
        let value = env::var(key)
            .map_err(|_| AppError::Validation(format!("secret_ref env key '{key}' is not set")))?;
        let value = value.trim();
        if value.is_empty() {
            return Err(AppError::Validation(format!(
                "secret_ref env key '{key}' resolved to empty value"
            )));
        }
        return Ok(value.to_string());
    }

    if secret_ref == SECRET_REF_ENCRYPTED_SENTINEL {
        return Err(AppError::Validation(
            "encrypted secret_ref is missing secret_ciphertext".to_string(),
        ));
    }

    if secret_ref.starts_with(SECRET_ENVELOPE_PREFIX) {
        return decrypt_secret(secret_ref, encryption_key_b64);
    }

    if let Some(value) = secret_ref.strip_prefix(SECRET_PLAIN_PREFIX) {
        let value = value.trim();
        if value.is_empty() {
            return Err(AppError::Validation(
                "secret_ref plain value is empty".to_string(),
            ));
        }
        return Ok(value.to_string());
    }

    Ok(secret_ref.to_string())
}

pub fn classify_monitoring_secret_storage(
    secret_ref: &str,
    secret_ciphertext: Option<&str>,
) -> &'static str {
    if trim_optional_str(secret_ciphertext).is_some() {
        return "encrypted";
    }

    let secret_ref = secret_ref.trim();
    if secret_ref.is_empty() {
        return "unknown";
    }
    if secret_ref.starts_with(SECRET_ENV_PREFIX) {
        return "env";
    }
    if secret_ref == SECRET_REF_ENCRYPTED_SENTINEL || secret_ref.starts_with(SECRET_ENVELOPE_PREFIX)
    {
        return "encrypted";
    }
    "legacy-inline"
}

pub fn mask_monitoring_secret(secret_ref: &str, secret_ciphertext: Option<&str>) -> String {
    match classify_monitoring_secret_storage(secret_ref, secret_ciphertext) {
        "encrypted" => "encrypted".to_string(),
        "env" => {
            let env_key = secret_ref
                .trim()
                .strip_prefix(SECRET_ENV_PREFIX)
                .map(str::trim)
                .unwrap_or_default();
            format!("{SECRET_ENV_PREFIX}{}", mask_identifier(env_key))
        }
        "legacy-inline" => "legacy-inline(hidden)".to_string(),
        _ => "not-configured".to_string(),
    }
}

fn encrypt_secret(plaintext: &str, encryption_key_b64: Option<&str>) -> AppResult<String> {
    let key = decode_key(encryption_key_b64)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| {
        AppError::Validation("invalid monitoring secret encryption key".to_string())
    })?;

    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|_| AppError::Validation("failed to encrypt monitoring secret".to_string()))?;

    let nonce_b64 = URL_SAFE_NO_PAD.encode(nonce);
    let ciphertext_b64 = URL_SAFE_NO_PAD.encode(ciphertext);
    Ok(format!(
        "{SECRET_ENVELOPE_PREFIX}{nonce_b64}:{ciphertext_b64}"
    ))
}

fn decrypt_secret(
    ciphertext_envelope: &str,
    encryption_key_b64: Option<&str>,
) -> AppResult<String> {
    let envelope = ciphertext_envelope.trim();
    let payload = envelope
        .strip_prefix(SECRET_ENVELOPE_PREFIX)
        .ok_or_else(|| {
            AppError::Validation("unsupported encrypted secret envelope format".to_string())
        })?;
    let (nonce_b64, ciphertext_b64) = payload.split_once(':').ok_or_else(|| {
        AppError::Validation("encrypted secret envelope payload is invalid".to_string())
    })?;
    if nonce_b64.trim().is_empty() || ciphertext_b64.trim().is_empty() {
        return Err(AppError::Validation(
            "encrypted secret envelope payload is invalid".to_string(),
        ));
    }

    let nonce = decode_base64(nonce_b64.trim()).map_err(|_| {
        AppError::Validation("encrypted secret envelope nonce is not valid base64".to_string())
    })?;
    if nonce.len() != 12 {
        return Err(AppError::Validation(
            "encrypted secret envelope nonce length is invalid".to_string(),
        ));
    }
    let ciphertext = decode_base64(ciphertext_b64.trim()).map_err(|_| {
        AppError::Validation("encrypted secret envelope ciphertext is not valid base64".to_string())
    })?;

    let key = decode_key(encryption_key_b64)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| {
        AppError::Validation("invalid monitoring secret encryption key".to_string())
    })?;
    let decrypted = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_slice())
        .map_err(|_| {
            AppError::Validation(
                "failed to decrypt monitoring secret; verify MONITORING_SECRET_ENCRYPTION_KEY"
                    .to_string(),
            )
        })?;

    String::from_utf8(decrypted).map_err(|_| {
        AppError::Validation("decrypted monitoring secret is not valid utf-8".to_string())
    })
}

fn decode_key(encryption_key_b64: Option<&str>) -> AppResult<[u8; 32]> {
    let raw = trim_optional_str(encryption_key_b64).ok_or_else(|| {
        AppError::Validation(
            "MONITORING_SECRET_ENCRYPTION_KEY is required for inline secret encryption/decryption"
                .to_string(),
        )
    })?;

    let bytes = decode_base64(raw).map_err(|_| {
        AppError::Validation(
            "MONITORING_SECRET_ENCRYPTION_KEY must be a valid base64-encoded 32-byte key"
                .to_string(),
        )
    })?;
    if bytes.len() != 32 {
        return Err(AppError::Validation(
            "MONITORING_SECRET_ENCRYPTION_KEY must decode to exactly 32 bytes".to_string(),
        ));
    }

    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn decode_base64(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
    URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| STANDARD.decode(input))
}

fn mask_identifier(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "***".to_string();
    }

    let chars = value.chars().collect::<Vec<_>>();
    let len = chars.len();
    if len <= 2 {
        return "*".repeat(len);
    }
    if len <= 6 {
        return format!("{}***", chars[0]);
    }
    format!("{}***{}", chars[0], chars[len - 1])
}

fn trim_optional_str(value: Option<&str>) -> Option<&str> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key_b64() -> String {
        URL_SAFE_NO_PAD.encode([7_u8; 32])
    }

    #[test]
    fn prepare_env_secret_ref_keeps_reference() {
        let prepared = prepare_monitoring_secret_for_storage(
            "env:ZABBIX_TOKEN",
            MONITORING_INLINE_SECRET_POLICY_ALLOW,
            None,
        )
        .expect("env ref should pass");
        assert_eq!(prepared.secret_ref, "env:ZABBIX_TOKEN");
        assert!(prepared.secret_ciphertext.is_none());
    }

    #[test]
    fn inline_secret_gets_encrypted_and_resolved() {
        let key = test_key_b64();
        let prepared = prepare_monitoring_secret_for_storage(
            "plain:secret-123",
            MONITORING_INLINE_SECRET_POLICY_ALLOW,
            Some(&key),
        )
        .expect("inline ref should encrypt");

        assert_eq!(prepared.secret_ref, "enc:v1");
        assert!(prepared.secret_ciphertext.is_some());

        let decrypted = resolve_monitoring_secret(
            prepared.secret_ref.as_str(),
            prepared.secret_ciphertext.as_deref(),
            Some(&key),
        )
        .expect("should decrypt");
        assert_eq!(decrypted, "secret-123");
    }

    #[test]
    fn forbid_policy_rejects_inline_secret() {
        let key = test_key_b64();
        let err = prepare_monitoring_secret_for_storage(
            "plain:secret-123",
            MONITORING_INLINE_SECRET_POLICY_FORBID,
            Some(&key),
        )
        .expect_err("inline ref should be blocked");
        assert!(
            err.to_string().contains("inline secret_ref is disabled"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_legacy_plain_secret_without_key() {
        let value = resolve_monitoring_secret("plain:legacy-secret", None, None)
            .expect("legacy plain format should stay compatible");
        assert_eq!(value, "legacy-secret");
    }

    #[test]
    fn mask_never_exposes_plain_secret() {
        assert_eq!(
            mask_monitoring_secret("plain:super-secret", None),
            "legacy-inline(hidden)"
        );
        assert_eq!(
            mask_monitoring_secret("env:ZABBIX_LOCAL_TOKEN", None),
            "env:Z***N"
        );
        assert_eq!(
            mask_monitoring_secret("enc:v1", Some("enc:v1:nonce:cipher")),
            "encrypted"
        );
    }
}
