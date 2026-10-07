use crate::error::PdfError;
use crate::types::CompressResult;
use lopdf::Document;
use std::path::Path;

/// Fast lossless-ish compression:
/// - deflates every uncompressed stream,
/// - removes zero-length streams and unreferenced objects,
/// - strips the Info dictionary and XMP metadata.
///
/// No image re-encoding, so it stays quick and safe for large documents.
#[tauri::command]
pub async fn compress_pdf(input: String, output: String) -> Result<CompressResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        compress_document(Path::new(&input), Path::new(&output))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

fn compress_document(input: &Path, output: &Path) -> Result<CompressResult, PdfError> {
    let before_bytes = std::fs::metadata(input)?.len();

    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    // Strip metadata: document Info dict and XMP Metadata stream reference.
    doc.trailer.remove(b"Info");
    if let Ok(catalog) = doc.catalog_mut() {
        catalog.remove(b"Metadata");
    }

    doc.delete_zero_length_streams();
    doc.prune_objects();
    doc.compress();

    // Unique temporary path to check resulting file size before committing to destination
    let temp_path = std::env::temp_dir().join(format!(
        "pdfkit_cmp_{}_{}.pdf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    doc.save(&temp_path)?;
    drop(doc);

    let after_bytes = std::fs::metadata(&temp_path)?.len();

    // Safety Guard:
    // If the re-encoded document is not strictly smaller than the original,
    // do not create/overwrite the destination file. Clean up temp and return already_optimized.
    if after_bytes >= before_bytes {
        let _ = std::fs::remove_file(&temp_path);
        return Ok(CompressResult {
            output: String::new(),
            before_bytes,
            after_bytes,
            already_optimized: true,
        });
    }

    // Move or copy temp file to final destination
    if let Err(_) = std::fs::rename(&temp_path, output) {
        std::fs::copy(&temp_path, output)?;
        let _ = std::fs::remove_file(&temp_path);
    }

    let result = CompressResult {
        output: output.to_string_lossy().into_owned(),
        before_bytes,
        after_bytes,
        already_optimized: false,
    };
    Ok(result)
}
