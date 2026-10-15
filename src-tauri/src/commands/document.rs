use crate::error::PdfError;
use crate::types::PdfInfo;
use lopdf::Document;
use std::path::PathBuf;

/// Reads basic metadata of a PDF: page count, version, encryption flag, size.
///
/// The document is dropped before returning, so its memory is freed
/// immediately after the info is extracted.
#[tauri::command]
pub async fn get_pdf_info(path: String) -> Result<PdfInfo, PdfError> {
    tauri::async_runtime::spawn_blocking(move || read_info(PathBuf::from(path)))
        .await
        .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

fn read_info(path: PathBuf) -> Result<PdfInfo, PdfError> {
    let file_size_bytes = std::fs::metadata(&path)?.len();
    let doc = Document::load(&path)?;
    let info = PdfInfo {
        path: path.to_string_lossy().into_owned(),
        page_count: doc.get_pages().len() as u32,
        version: doc.version.clone(),
        encrypted: doc.is_encrypted(),
        file_size_bytes,
    };
    drop(doc);
    Ok(info)
}

/// Opens Windows Explorer with the specified file selected (or folder opened).
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::path::Path;
        use std::process::Command;

        // Sanitize to prevent command/argument injection
        let clean_path = path
            .replace('/', "\\")
            .replace('"', "")
            .replace(['\r', '\n', '\0'], "");
        let p = Path::new(&clean_path);

        if p.is_file() || (p.extension().is_some() && p.parent().map_or(false, |par| par.exists())) {
            Command::new("explorer")
                .raw_arg(format!("/select,\"{}\"", clean_path))
                .spawn()
                .map_err(|e| e.to_string())?;
        } else if p.is_dir() {
            Command::new("explorer")
                .raw_arg(format!("\"{}\"", clean_path))
                .spawn()
                .map_err(|e| e.to_string())?;
        } else if let Some(parent) = p.parent().filter(|par| par.exists()) {
            Command::new("explorer")
                .raw_arg(format!("\"{}\"", parent.display()))
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            Command::new("explorer")
                .raw_arg(format!("\"{}\"", clean_path))
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Ok(())
    }
}

/// Reads the entire contents of a file as raw bytes from disk.
/// Bypasses webview sandbox / scope limitations so that files opened via
/// command line arguments, Windows "Open With", file associations, or drag-and-drop
/// can be loaded into pdf.js without permission errors.
#[tauri::command]
pub async fn read_binary_file(path: String) -> Result<Vec<u8>, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let clean_path = path
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        let p = std::path::Path::new(clean_path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        const ALLOWED_READ_EXTS: &[&str] = &[
            "pdf", "png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tif",
        ];

        if !ALLOWED_READ_EXTS.contains(&ext.as_str()) {
            return Err(PdfError::Custom(
                "Güvenlik kısıtlaması: Yalnızca PDF ve desteklenen görsel dosyaları okunabilir.".into(),
            ));
        }

        let bytes = std::fs::read(clean_path)?;
        Ok(bytes)
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}
