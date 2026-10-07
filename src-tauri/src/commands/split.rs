use crate::error::PdfError;
use crate::pdfutil::{materialize_inherited, media_box};
use crate::types::OpResult;
use lopdf::{Dictionary, Document, Object};
use serde::Deserialize;
use std::path::Path;

/// Inclusive 1-based page range, e.g. `{ start: 1, end: 5 }`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PageRange {
    pub start: u32,
    pub end: u32,
}

/// Splits a PDF into multiple files, one per page range.
///
/// - Ranges are 1-based and inclusive.
/// - An empty `ranges` list means "extract every page into its own file".
/// - Output file names: `<stem>_p<start>-<end>.pdf` inside `output_dir`.
///
/// Every surviving page gets inheritable attributes materialized, is reparented
/// to a fresh flat Pages node, and unreachable objects are pruned.
#[tauri::command]
pub async fn split_pdf(
    input: String,
    ranges: Vec<PageRange>,
    output_dir: String,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        split_document(
            Path::new(&input),
            &ranges,
            Path::new(&output_dir),
        )
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

fn split_document(
    input: &Path,
    ranges: &[PageRange],
    output_dir: &Path,
) -> Result<OpResult, PdfError> {
    let doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let page_count = doc.get_pages().len() as u32;
    if page_count == 0 {
        return Err(PdfError::EmptyOutput);
    }

    // Default mode: every page becomes its own file.
    let effective: Vec<PageRange> = if ranges.is_empty() {
        (1..=page_count)
            .map(|p| PageRange { start: p, end: p })
            .collect()
    } else {
        ranges.to_vec()
    };

    for r in &effective {
        if r.start == 0 || r.end < r.start || r.end > page_count {
            return Err(PdfError::InvalidRange {
                start: r.start,
                end: r.end,
                page_count,
            });
        }
    }

    std::fs::create_dir_all(output_dir)?;
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_string());

    let mut outputs: Vec<String> = Vec::with_capacity(effective.len());
    let mut total_size_bytes: u64 = 0;

    for range in &effective {
        let mut part = doc.clone();
        let page_map = part.get_pages();
        let new_pages_id = part.new_object_id();
        let mut kids: Vec<Object> = Vec::with_capacity((range.end - range.start + 1) as usize);

        for p in range.start..=range.end {
            if let Some(&page_id) = page_map.get(&p) {
                materialize_inherited(&mut part, page_id)?;
                let box_values = media_box(&part, page_id);
                let dict = part.get_dictionary_mut(page_id)?;
                dict.set(
                    "MediaBox",
                    Object::Array(box_values.iter().map(|v| Object::Real(*v as f32)).collect()),
                );
                dict.set("Parent", Object::Reference(new_pages_id));
                kids.push(Object::Reference(page_id));
            }
        }

        let count = kids.len() as i64;
        let new_pages = Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Pages".to_vec())),
            ("Kids", Object::Array(kids)),
            ("Count", Object::Integer(count)),
        ]);
        part.objects.insert(new_pages_id, Object::Dictionary(new_pages));

        part.catalog_mut()?
            .set("Pages", Object::Reference(new_pages_id));

        part.prune_objects();
        part.compress();

        let out_path = output_dir.join(format!("{stem}_p{}-{}.pdf", range.start, range.end));
        part.save(&out_path)?;
        drop(part);

        total_size_bytes += std::fs::metadata(&out_path)?.len();
        outputs.push(out_path.to_string_lossy().into_owned());
    }

    drop(doc);
    Ok(OpResult {
        total_pages: page_count,
        total_size_bytes,
        outputs,
    })
}
