use crate::error::PdfError;
use crate::types::OpResult;
use lopdf::{Document, Object};
use std::path::Path;

/// Rotates pages of a PDF by a relative angle.
///
/// - `angle` must be a multiple of 90 (positive = clockwise).
/// - An empty `pages` list rotates every page.
/// - Rotation is cumulative: the delta is added to each page's existing
///   `/Rotate` value and normalized to `0..360`.
#[tauri::command]
pub async fn rotate_pages(
    input: String,
    pages: Vec<u32>,
    angle: i64,
    output: String,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        rotate_document(Path::new(&input), &pages, angle, Path::new(&output))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

fn rotate_document(
    input: &Path,
    pages: &[u32],
    angle: i64,
    output: &Path,
) -> Result<OpResult, PdfError> {
    if angle % 90 != 0 {
        return Err(PdfError::InvalidAngle(angle));
    }

    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let page_map = doc.get_pages();
    let page_count = page_map.len() as u32;
    if page_count == 0 {
        return Err(PdfError::EmptyOutput);
    }

    let targets: Vec<u32> = if pages.is_empty() {
        (1..=page_count).collect()
    } else {
        pages.to_vec()
    };

    for page_num in targets {
        let page_id = *page_map
            .get(&page_num)
            .ok_or(PdfError::PageOutOfBounds {
                page: page_num,
                page_count,
            })?;

        let dict = doc.get_dictionary_mut(page_id)?;
        let current = match dict.get(b"Rotate") {
            Ok(obj) => obj.as_i64()?,
            Err(_) => 0,
        };
        let next = ((current + angle) % 360 + 360) % 360;
        dict.set("Rotate", Object::Integer(next));
    }

    doc.save(output)?;

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: page_count,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}
