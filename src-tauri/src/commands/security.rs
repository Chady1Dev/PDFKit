use crate::error::PdfError;
use crate::types::OpResult;
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, CryptFilter};
use lopdf::encryption::{EncryptionState, EncryptionVersion, Permissions};
use lopdf::{Document, Object};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

/// Encrypts a PDF with AES-128 (PDF 1.5 / V4 standard security handler).
///
/// - `user_password`: required to open the document (may be empty to only
///   restrict permissions).
/// - `owner_password`: required to change permissions; falls back to the
///   user password when empty.
/// - All standard permissions are granted; restriction flags can be added
///   later without touching the crypto path.
#[tauri::command]
pub async fn encrypt_pdf(
    input: String,
    output: String,
    user_password: String,
    owner_password: String,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        encrypt_document(
            Path::new(&input),
            Path::new(&output),
            &user_password,
            &owner_password,
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

    let mut crypt_filters: BTreeMap<Vec<u8>, Arc<dyn CryptFilter>> = BTreeMap::new();
    crypt_filters.insert(b"StdCF".to_vec(), Arc::new(Aes128CryptFilter));

    let state = EncryptionState::try_from(EncryptionVersion::V4 {
        document: &doc,
        encrypt_metadata: true,
        crypt_filters,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: owner,
        user_password,
        permissions: Permissions::all(),
    })?;

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
