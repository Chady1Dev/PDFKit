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

fn deref_obj<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| deref_obj(doc, o)),
        other => Some(other),
    }
}

fn get_appearance_states(doc: &Document, dict: &lopdf::Dictionary) -> Vec<String> {
    let mut states = Vec::new();
    if let Ok(ap_obj) = dict.get(b"AP") {
        if let Some(ap_resolved) = deref_obj(doc, ap_obj) {
            if let Ok(ap_dict) = ap_resolved.as_dict() {
                if let Ok(n_obj) = ap_dict.get(b"N") {
                    if let Some(n_resolved) = deref_obj(doc, n_obj) {
                        if let Ok(n_dict) = n_resolved.as_dict() {
                            for (k, _) in n_dict.iter() {
                                let key_str = String::from_utf8_lossy(k).into_owned();
                                if key_str != "Off" && !states.contains(&key_str) {
                                    states.push(key_str);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    states
}

fn get_opt_strings(doc: &Document, dict: &lopdf::Dictionary) -> Vec<String> {
    let mut options = Vec::new();
    if let Ok(opt_obj) = dict.get(b"Opt") {
        if let Some(opt_resolved) = deref_obj(doc, opt_obj) {
            if let Ok(opt_arr) = opt_resolved.as_array() {
                for item in opt_arr {
                    if let Some(item_res) = deref_obj(doc, item) {
                        match item_res {
                            Object::String(bytes, _) => {
                                options.push(String::from_utf8_lossy(bytes).into_owned());
                            }
                            Object::Array(pair) if !pair.is_empty() => {
                                let label_obj = pair.get(1).unwrap_or(&pair[0]);
                                if let Some(lbl_res) = deref_obj(doc, label_obj) {
                                    if let Object::String(bytes, _) = lbl_res {
                                        options.push(String::from_utf8_lossy(bytes).into_owned());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    options
}

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

    let raw_ft = match dict.get(b"FT").ok() {
        Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => "Tx".to_string(),
    };

    let ff = dict
        .get(b"Ff")
        .ok()
        .and_then(|o| match o {
            Object::Integer(i) => Some(*i as u32),
            _ => None,
        })
        .unwrap_or(0);

    let read_only = (ff & 1) != 0;
    let is_pushbutton = (ff & (1 << 16)) != 0;
    let is_radio_flag = (ff & (1 << 15)) != 0;
    let is_multiline = (ff & (1 << 12)) != 0;
    let is_password = (ff & (1 << 13)) != 0;
    let is_combo = (ff & (1 << 17)) != 0;

    let mut states = get_appearance_states(doc, dict);
    let has_kids = if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
        for kid in kids {
            if let Some(kid_res) = deref_obj(doc, kid) {
                if let Ok(kid_dict) = kid_res.as_dict() {
                    for s in get_appearance_states(doc, kid_dict) {
                        if !states.contains(&s) {
                            states.push(s);
                        }
                    }
                }
            }
        }
        !kids.is_empty()
    } else {
        false
    };

    let (canonical_type, options, export_value) = match raw_ft.as_str() {
        "Btn" => {
            if is_pushbutton {
                ("pushbutton".to_string(), Vec::new(), None)
            } else if is_radio_flag || (has_kids && states.len() >= 2) {
                let opts = if !states.is_empty() {
                    states
                } else {
                    vec!["Yes".to_string(), "Off".to_string()]
                };
                ("radio".to_string(), opts, None)
            } else {
                let exp = states.first().cloned().unwrap_or_else(|| "Yes".to_string());
                ("checkbox".to_string(), Vec::new(), Some(exp))
            }
        }
        "Tx" => {
            if is_multiline {
                ("multiline".to_string(), Vec::new(), None)
            } else if is_password {
                ("password".to_string(), Vec::new(), None)
            } else {
                ("text".to_string(), Vec::new(), None)
            }
        }
        "Ch" => {
            let opts = get_opt_strings(doc, dict);
            if is_combo {
                ("choice".to_string(), opts, None)
            } else {
                ("listbox".to_string(), opts, None)
            }
        }
        "Sig" => ("signature".to_string(), Vec::new(), None),
        _ => ("text".to_string(), Vec::new(), None),
    };

    let mut value = match dict.get(b"V").ok() {
        Some(Object::String(bytes, _)) => String::from_utf8_lossy(bytes).into_owned(),
        Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    };

    if canonical_type == "checkbox" {
        if value.is_empty() || value == "Off" || value == "/" {
            value = "Off".to_string();
        } else if value == "On" || value == "Yes" || Some(&value) == export_value.as_ref() {
            value = export_value.clone().unwrap_or_else(|| "Yes".to_string());
        }
    }

    Some(FormField {
        name,
        field_type: canonical_type,
        value,
        options,
        read_only,
        export_value,
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

        // Collect child widget ObjectIds if field has /Kids
        let kid_ids: Vec<ObjectId> = if let Ok(Object::Array(kids)) =
            doc.get_dictionary(id).and_then(|d| d.get(b"Kids"))
        {
            kids.iter()
                .filter_map(|k| match k {
                    Object::Reference(kid_id) => Some(*kid_id),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };

        if field.field_type == "checkbox" {
            let is_checked = new_value == "true"
                || new_value == "1"
                || new_value == "Yes"
                || new_value == "On"
                || (!new_value.is_empty()
                    && new_value != "Off"
                    && new_value != "false"
                    && new_value != "0");
            let on_name = field
                .export_value
                .as_deref()
                .unwrap_or("Yes")
                .to_string();
            let active_state = if is_checked {
                on_name.into_bytes()
            } else {
                b"Off".to_vec()
            };

            let dict = doc.get_dictionary_mut(id)?;
            dict.set("V", Object::Name(active_state.clone()));
            dict.set("AS", Object::Name(active_state.clone()));

            for kid_id in kid_ids {
                if let Ok(kid_dict) = doc.get_dictionary_mut(kid_id) {
                    kid_dict.set("AS", Object::Name(active_state.clone()));
                }
            }
        } else if field.field_type == "radio" {
            let selected_val = new_value.trim();
            let dict = doc.get_dictionary_mut(id)?;
            dict.set("V", Object::Name(selected_val.as_bytes().to_vec()));

            for kid_id in kid_ids {
                let kid_states = doc
                    .get_dictionary(kid_id)
                    .map(|d| get_appearance_states(&doc, d))
                    .unwrap_or_default();
                let kid_state = if kid_states.iter().any(|s| s == selected_val) {
                    selected_val.as_bytes().to_vec()
                } else {
                    b"Off".to_vec()
                };
                if let Ok(kid_dict) = doc.get_dictionary_mut(kid_id) {
                    kid_dict.set("AS", Object::Name(kid_state));
                }
            }
        } else if field.field_type == "pushbutton" {
            // Pushbuttons do not hold fillable values
            continue;
        } else {
            // Text, Multiline, Password, Choice, Listbox
            let dict = doc.get_dictionary_mut(id)?;
            dict.set("V", Object::string_literal(new_value.clone()));
            dict.remove(b"AP"); // force appearance regeneration
        }
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
