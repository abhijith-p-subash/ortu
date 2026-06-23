// Field-level encryption for sensitive (masked) clipboard items.
//
// Only items the user marks sensitive (or auto-masked secrets) are encrypted —
// not the whole database. The key lives in a 0600 file in the app data dir.
// Encrypted values are tagged with an `enc:v1:` prefix so they're detectable
// without a separate flag. If the key is ever lost, only sensitive items become
// unreadable; everything else is untouched.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::Engine as _;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const PREFIX: &str = "enc:v1:";
const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn key_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(".sensitive_key"))
}

/// Loads the 256-bit sensitive-item key, creating it on first use.
pub fn get_or_create_key(app: &AppHandle) -> Result<[u8; 32], String> {
    let path = key_path(app).ok_or_else(|| "no app data dir".to_string())?;

    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }

    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, key).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

/// True if `value` is an Ortu-encrypted payload.
pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(PREFIX)
}

/// Encrypts plaintext into a tagged, base64 (nonce || ciphertext) string.
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(format!("{}{}", PREFIX, B64.encode(combined)))
}

/// Decrypts a value produced by `encrypt`. Errors if the key is wrong/missing.
pub fn decrypt(key: &[u8; 32], stored: &str) -> Result<String, String> {
    let b64 = stored
        .strip_prefix(PREFIX)
        .ok_or_else(|| "value is not encrypted".to_string())?;
    let data = B64.decode(b64).map_err(|e| e.to_string())?;
    if data.len() < 12 {
        return Err("ciphertext too short".to_string());
    }
    let (nonce, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "decryption failed (key unavailable or data corrupt)".to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}
