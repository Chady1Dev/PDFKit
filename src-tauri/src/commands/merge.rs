use crate::error::PdfError;
use crate::types::OpResult;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};

/// Merges multiple PDFs into one, preserving each source's internal page
/// tree (so inherited resources such as MediaBox survive the merge).
///
/// Strategy:
/// 1. Create an empty target document with a fresh root `Pages` node.
/// 2. For every source: renumber its objects above `target.max_id` to avoid
///    ID collisions, re-parent its page-tree root to the new `Pages` node,
///    then move its objects into the target.
/// 3. Write a new catalog, prune unreferenced objects (old source catalogs),
///    compress streams, save.
///
/// Each source `Document` is dropped at the end of its loop iteration, so
/// peak memory stays close to "largest input + accumulated output".
#[tauri::command]
pub async fn merge_pdfs(inputs: Vec<String>, output: String) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let inputs: Vec<PathBuf> = inputs.into_iter().map(PathBuf::from).collect();
        merge_documents(&inputs, Path::new(&output))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

fn merge_documents(inputs: &[PathBuf], output: &Path) -> Result<OpResult, PdfError> {
    if inputs.len() < 2 {
        return Err(PdfError::NotEnoughInputs(2));
    }

    let mut target = Document::with_version("1.7");
    let root_pages_id = target.new_object_id();
    let mut kids: Vec<Object> = Vec::new();
    let mut total_pages: u32 = 0;

    for input in inputs {
        let mut src = Document::load(input)?;
        if src.is_encrypted() {
            return Err(PdfError::Encrypted);
        }

        let page_count = src.get_pages().len() as u32;
        if page_count == 0 {
            continue; // skip empty documents instead of failing the whole merge
        }

        // Shift all object IDs of the source above the current target maximum
        // so the two object maps never collide.
        src.renumber_objects_with(target.max_id + 1);

        // Find the root of the source page tree and re-parent it to the
        // target's root Pages node.
        let src_pages_id: ObjectId = src.catalog()?.get(b"Pages")?.as_reference()?;
        if let Object::Dictionary(pages_dict) = src.get_object_mut(src_pages_id)? {
            pages_dict.set("Parent", Object::Reference(root_pages_id));
        }

        kids.push(Object::Reference(src_pages_id));
        total_pages += page_count;
        target.max_id = target.max_id.max(src.max_id);
        target.objects.extend(src.objects);
        // Remaining fields of `src` drop here at scope end automatically.
    }

    if total_pages == 0 {
        return Err(PdfError::EmptyOutput);
    }

    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(kids)),
        ("Count", Object::Integer(i64::from(total_pages))),
    ]);
    target.objects.insert(root_pages_id, Object::Dictionary(pages_dict));

    let catalog_id = target.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(root_pages_id)),
    ]));
    target.trailer.set("Root", Object::Reference(catalog_id));

    // Remove now-unreferenced objects (old source catalogs) and deflate streams.
    target.prune_objects();
    target.compress();
    target.save(output)?;

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(target);
    Ok(result)
}
