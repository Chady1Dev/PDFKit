use crate::error::PdfError;
use crate::types::OpResult;
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
use lopdf::encryption::{EncryptionState, EncryptionVersion, Permissions};
use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EncryptionPermissions {
    #[serde(default = "default_true")]
    pub allow_print: bool,
    #[serde(default = "default_true")]
    pub allow_copy: bool,
    #[serde(default = "default_true")]
    pub allow_modify: bool,
    #[serde(default = "default_true")]
    pub allow_annotate: bool,
}

impl Default for EncryptionPermissions {
    fn default() -> Self {
        Self {
            allow_print: true,
            allow_copy: true,
            allow_modify: true,
            allow_annotate: true,
        }
    }
}

/// Encrypts a PDF with AES-256 (default, V5 / PDF 2.0 / ISO 32000-2) or AES-128 (V4).
///
/// - `user_password`: required to open the document (may be empty to only
///   restrict permissions).
/// - `owner_password`: required to change permissions; falls back to the
///   user password when empty.
/// - `permissions`: granular permission flags (printing, copying, modification, annotations).
/// - `algorithm`: "aes256" (default) or "aes128".
#[tauri::command]
pub async fn encrypt_pdf(
    input: String,
    output: String,
    user_password: String,
    owner_password: String,
    permissions: Option<EncryptionPermissions>,
    algorithm: Option<String>,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        encrypt_document(
            Path::new(&input),
            Path::new(&output),
            &user_password,
            &owner_password,
            permissions.unwrap_or_default(),
            algorithm.as_deref().unwrap_or("aes256"),
        )
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Removes password protection. Requires the current user or owner password.
#[tauri::command]
pub async fn decrypt_pdf(
    input: String,
    output: String,
    password: String,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        decrypt_document(Path::new(&input), Path::new(&output), &password)
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// 16 cryptographically secure random bytes for PDF File ID using OS entropy.
fn random_file_id() -> Vec<u8> {
    let mut buf = [0u8; 16];
    if getrandom::getrandom(&mut buf).is_err() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut x = nanos ^ ((std::process::id() as u128) << 64);
        for b in buf.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = (x & 0xFF) as u8;
        }
    }
    buf.to_vec()
}

fn encrypt_document(
    input: &Path,
    output: &Path,
    user_password: &str,
    owner_password: &str,
    permissions: EncryptionPermissions,
    algorithm: &str,
) -> Result<OpResult, PdfError> {
    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    // Encryption requires a file /ID in the trailer; generate one if absent.
    if doc.trailer.get(b"ID").is_err() {
        let id = Object::Array(vec![
            Object::String(random_file_id(), lopdf::StringFormat::Hexadecimal),
            Object::String(random_file_id(), lopdf::StringFormat::Hexadecimal),
        ]);
        doc.trailer.set("ID", id);
    }

    let owner = if owner_password.is_empty() {
        user_password
    } else {
        owner_password
    };

    // Build permissions bitflags
    let mut perms = Permissions::empty();
    perms.insert(Permissions::COPYABLE_FOR_ACCESSIBILITY);
    if permissions.allow_print {
        perms.insert(Permissions::PRINTABLE | Permissions::PRINTABLE_IN_HIGH_QUALITY);
    }
    if permissions.allow_copy {
        perms.insert(Permissions::COPYABLE);
    }
    if permissions.allow_modify {
        perms.insert(Permissions::MODIFIABLE | Permissions::ASSEMBLABLE);
    }
    if permissions.allow_annotate {
        perms.insert(Permissions::ANNOTABLE | Permissions::FILLABLE);
    }

    let state = if algorithm.eq_ignore_ascii_case("aes128") {
        let mut crypt_filters: BTreeMap<Vec<u8>, Arc<dyn CryptFilter>> = BTreeMap::new();
        crypt_filters.insert(b"StdCF".to_vec(), Arc::new(Aes128CryptFilter));
        EncryptionState::try_from(EncryptionVersion::V4 {
            document: &doc,
            encrypt_metadata: true,
            crypt_filters,
            stream_filter: b"StdCF".to_vec(),
            string_filter: b"StdCF".to_vec(),
            owner_password: owner,
            user_password,
            permissions: perms,
        })?
    } else {
        // AES-256 (Revision 5/6 standard handler)
        let mut crypt_filters: BTreeMap<Vec<u8>, Arc<dyn CryptFilter>> = BTreeMap::new();
        crypt_filters.insert(b"StdCF".to_vec(), Arc::new(Aes256CryptFilter));
        let mut key = [0u8; 32];
        if getrandom::getrandom(&mut key).is_err() {
            let rand_bytes = random_file_id();
            key[..16].copy_from_slice(&rand_bytes);
            key[16..].copy_from_slice(&random_file_id());
        }
        EncryptionState::try_from(EncryptionVersion::V5 {
            encrypt_metadata: true,
            crypt_filters,
            file_encryption_key: &key,
            stream_filter: b"StdCF".to_vec(),
            string_filter: b"StdCF".to_vec(),
            owner_password: owner,
            user_password,
            permissions: perms,
        })?
    };


    let page_count = doc.get_pages().len() as u32;
    doc.encrypt(&state)?;
    doc.save(output)?;

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: page_count,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}

fn decrypt_document(input: &Path, output: &Path, password: &str) -> Result<OpResult, PdfError> {
    // `Document::load` refuses to materialize objects of encrypted files;
    // `load_with_password` authenticates and decrypts everything eagerly.
    // A wrong password fails right here.
    let mut doc = Document::load_with_password(input, password)
        .map_err(|_| PdfError::WrongPassword)?;

    // Fully detach encryption: drop the trailer entry AND the in-memory
    // state so the saved file is written plain.
    doc.trailer.remove(b"Encrypt");
    doc.encryption_state = None;

    let page_count = doc.get_pages().len() as u32;
    doc.compress();
    doc.save(output)?;

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: page_count,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}
