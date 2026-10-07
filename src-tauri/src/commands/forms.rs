use crate::commands::images::build_image_xobject;
use crate::error::PdfError;
use crate::types::{FormField, OpResult};
use lopdf::{Document, Object, ObjectId};
use std::collections::HashMap;
use std::path::Path;

/// Lists the AcroForm fields of a PDF (name, type, current value).
#[tauri::command]
pub async fn list_form_fields(input: String) -> Result<Vec<FormField>, PdfError> {
    tauri::async_runtime::spawn_blocking(move || collect_fields(Path::new(&input)))
        .await
        .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Fills text/checkbox-ish AcroForm fields by name and saves a copy.
///
/// Sets `/NeedAppearances` so viewers regenerate field appearances.
#[tauri::command]
pub async fn fill_form(
    input: String,
    output: String,
    values: HashMap<String, String>,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        fill_fields(Path::new(&input), Path::new(&output), &values)
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Stamps an image (e.g. a scanned signature) onto a page at a rectangle
/// given in PDF points (origin = bottom-left).
#[tauri::command]
pub async fn stamp_image(
    input: String,
    output: String,
    page: u32,
    image_path: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        stamp(
            Path::new(&input),
            Path::new(&output),
            page,
            Path::new(&image_path),
            x,
            y,
            width,
            height,
        )
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

// ---------------------------------------------------------------------------

fn field_ids(doc: &Document) -> Vec<ObjectId> {
    let mut out = Vec::new();
    let Ok(catalog) = doc.catalog() else { return out };
    let Ok(acro_obj) = catalog.get(b"AcroForm") else { return out };
    let Ok((_, acro)) = doc.dereference(acro_obj) else { return out };
    let Ok(acro_dict) = acro.as_dict() else { return out };
    let Ok(Object::Array(fields)) = acro_dict.get(b"Fields") else {
        return out;
    };
    for f in fields {
        collect_field_ids(doc, f, &mut out);
    }
    out
}

fn collect_field_ids(doc: &Document, obj: &Object, out: &mut Vec<ObjectId>) {
    if let Ok((Some(id), resolved)) = doc.dereference(obj) {
        if let Ok(dict) = resolved.as_dict() {
            if dict.get(b"T").is_ok() {
                out.push(id);
            }
            // Fields can nest via /Kids — walk one structure recursively.
            if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
                for kid in kids {
                    collect_field_ids(doc, kid, out);
                }
            }
        }
    }
}

fn read_field(doc: &Document, id: ObjectId) -> Option<FormField> {
    let dict = doc.get_dictionary(id).ok()?;
    let name = match dict.get(b"T").ok()? {
        Object::String(bytes, _) => String::from_utf8_lossy(bytes).into_owned(),
        _ => return None,
    };
    let field_type = match dict.get(b"FT").ok() {
        Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => "Tx".to_string(),
    };
    let value = match dict.get(b"V").ok() {
        Some(Object::String(bytes, _)) => String::from_utf8_lossy(bytes).into_owned(),
        Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    };
    Some(FormField {
        name,
        field_type,
        value,
    })
}

fn collect_fields(input: &Path) -> Result<Vec<FormField>, PdfError> {
    let doc = Document::load(input)?;
    let fields = field_ids(&doc)
        .iter()
        .filter_map(|id| read_field(&doc, *id))
        .collect();
    drop(doc);
    Ok(fields)
}

fn fill_fields(
    input: &Path,
    output: &Path,
    values: &HashMap<String, String>,
) -> Result<OpResult, PdfError> {
    let mut doc = Document::load(input)?;
    let ids = field_ids(&doc);

    for id in ids {
        let Some(field) = read_field(&doc, id) else {
            continue;
        };
        let Some(new_value) = values.get(&field.name) else {
            continue;
        };
        let dict = doc.get_dictionary_mut(id)?;
        if field.field_type == "Btn" {
            // Checkbox / radio: set the export value as a name object.
            dict.set("V", Object::Name(new_value.clone().into_bytes()));
            dict.set("AS", Object::Name(new_value.clone().into_bytes()));
        } else {
            dict.set("V", Object::string_literal(new_value.clone()));
        }
        dict.remove(b"AP"); // force appearance regeneration
    }

    if let Ok(acro) = doc.catalog_mut()?.get_mut(b"AcroForm") {
        if let Ok(dict) = acro.as_dict_mut() {
            dict.set("NeedAppearances", Object::Boolean(true));
        }
    }

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

#[allow(clippy::too_many_arguments)]
fn stamp(
    input: &Path,
    output: &Path,
    page: u32,
    image_path: &Path,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<OpResult, PdfError> {
    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let page_map = doc.get_pages();
    let page_count = page_map.len() as u32;
    let page_id = *page_map.get(&page).ok_or(PdfError::PageOutOfBounds {
        page,
        page_count,
    })?;

    let bytes = std::fs::read(image_path)?;
    let img = build_image_xobject(&bytes)?;
    drop(bytes);
    let img_id = doc.add_object(img.stream);

    doc.get_or_create_resources(page_id)?;
    doc.add_xobject(page_id, "ImSig", img_id)?;

    let content = format!(
        "q\n{w:.1} 0 0 {h:.1} {x:.1} {y:.1} cm\n/ImSig Do\nQ\n",
        w = width,
        h = height,
        x = x,
        y = y
    );
    doc.add_page_contents(page_id, content.into_bytes())?;
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
