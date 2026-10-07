use crate::error::PdfError;
use crate::pdfutil::{materialize_inherited, media_box};
use crate::types::OpResult;
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Deserialize;
use std::path::Path;

/// One entry of the desired output page order.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct OrganizePage {
    /// 1-based page number in the source document.
    pub source_page: u32,
    /// Extra rotation to apply on top of the page's current /Rotate.
    pub rotate_delta: i64,
}

/// Rebuilds a PDF with pages reordered, duplicated, removed and/or rotated
/// in a single pass — the backend for the drag & drop "Organize" screen.
///
/// `pages` is the exact desired output order; omitting a source page deletes
/// it, repeating it duplicates it.
///
/// Implementation: every surviving leaf page gets its inheritable attributes
/// (Resources, MediaBox, CropBox, Rotate) materialized, is re-parented to a
/// fresh flat Pages node, and the old page tree is pruned. This keeps the
/// operation O(surviving pages) and frees deleted pages from memory.
#[tauri::command]
pub async fn organize_pdf(
    input: String,
    pages: Vec<OrganizePage>,
    output: String,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        organize_document(Path::new(&input), &pages, Path::new(&output))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

fn organize_document(
    input: &Path,
    pages: &[OrganizePage],
    output: &Path,
) -> Result<OpResult, PdfError> {
    if pages.is_empty() {
        return Err(PdfError::EmptyOutput);
    }

    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let page_map = doc.get_pages();
    let page_count = page_map.len() as u32;

    // Resolve target page IDs up front (validates bounds before mutating).
    let mut targets: Vec<(ObjectId, i64)> = Vec::with_capacity(pages.len());
    for op in pages {
        if op.rotate_delta % 90 != 0 {
            return Err(PdfError::InvalidAngle(op.rotate_delta));
        }
        let id = *page_map
            .get(&op.source_page)
            .ok_or(PdfError::PageOutOfBounds {
                page: op.source_page,
                page_count,
            })?;
        targets.push((id, op.rotate_delta));
    }

    let new_pages_id = doc.new_object_id();
    let mut kids: Vec<Object> = Vec::with_capacity(targets.len());

    for (page_id, delta) in targets {
        // Pull inherited attributes down before cutting the page off its tree.
        materialize_inherited(&mut doc, page_id)?;

        // Force a MediaBox key so flattened pages never lose their size.
        let box_values = media_box(&doc, page_id);
        let dict = doc.get_dictionary_mut(page_id)?;
        dict.set(
            "MediaBox",
            Object::Array(box_values.iter().map(|v| Object::Real(*v as f32)).collect()),
        );

        let current = match dict.get(b"Rotate") {
            Ok(obj) => obj.as_i64()?,
            Err(_) => 0,
        };
        let next = ((current + delta) % 360 + 360) % 360;
        if next == 0 {
            dict.remove(b"Rotate");
        } else {
            dict.set("Rotate", Object::Integer(next));
        }
        dict.set("Parent", Object::Reference(new_pages_id));
        kids.push(Object::Reference(page_id));
    }

    let count = kids.len() as i64;
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(kids)),
        ("Count", Object::Integer(count)),
    ]);
    doc.objects
        .insert(new_pages_id, Object::Dictionary(pages_dict));
    doc.catalog_mut()?
        .set("Pages", Object::Reference(new_pages_id));

    // Old page tree + deleted pages become unreferenced and are dropped.
    doc.prune_objects();
    doc.compress();
    doc.save(output)?;

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: count as u32,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}
