//! Shared low-level PDF helpers used by several command modules.

use crate::error::PdfError;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Page-tree attributes a leaf page may inherit from its ancestors.
pub const INHERITABLE_KEYS: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// Walks up the page tree from `start` looking for `key`, returning the first
/// value found (dereferenced enough to be directly usable).
pub fn inherited_value(doc: &Document, start: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = Some(start);
    while let Some(id) = current {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(value) = dict.get(key) {
            return Some(value.clone());
        }
        current = dict
            .get(b"Parent")
            .ok()
            .and_then(|p| p.as_reference().ok());
    }
    None
}

/// Copies every missing inheritable attribute from the page-tree ancestors
/// down into the leaf page dictionary. Required before re-parenting a page
/// to a different Pages node.
pub fn materialize_inherited(doc: &mut Document, page_id: ObjectId) -> Result<(), PdfError> {
    for key in INHERITABLE_KEYS {
        let needs_copy = match doc.get_dictionary(page_id) {
            Ok(dict) => dict.get(key).is_err(),
            Err(_) => false,
        };
        if needs_copy {
            if let Some(value) = inherited_value(doc, page_id, key) {
                doc.get_dictionary_mut(page_id)?.set(key, value);
            }
        }
    }
    Ok(())
}

/// Returns the effective MediaBox of a page as `[x0, y0, x1, y1]`,
/// honoring inheritance. Defaults to US Letter when absent.
pub fn media_box(doc: &Document, page_id: ObjectId) -> [f64; 4] {
    let fallback = [0.0, 0.0, 612.0, 792.0];
    let Some(Object::Array(items)) = inherited_value(doc, page_id, b"MediaBox") else {
        return fallback;
    };
    let mut out = fallback;
    for (i, item) in items.iter().take(4).enumerate() {
        out[i] = match item {
            Object::Integer(n) => *n as f64,
            Object::Real(n) => *n as f64,
            _ => return fallback,
        };
    }
    out
}

pub const PDFKIT_FONT_NAME: &[u8] = b"PK_Font";

/// Ensures a WinAnsi Helvetica base-14 font is registered in the page's
/// resources under `PK_Font`.
///
/// Unlike naive implementations that overwrite `/F1` or wipe out indirect font dictionaries,
/// this implementation:
/// 1. Materializes inherited page resources so inherited fonts and attributes are never lost.
/// 2. If the Resources dictionary is an indirect reference, updates that dictionary in place.
/// 3. If the Font sub-dictionary is an indirect reference, updates that dictionary in place.
/// 4. If the Font sub-dictionary is an inline dictionary, updates it in place.
/// 5. Uses dedicated name `/PK_Font`, completely preserving existing document fonts (/F1, /F2, etc.).
pub fn ensure_helvetica(doc: &mut Document, page_id: ObjectId) -> Result<(), PdfError> {
    materialize_inherited(doc, page_id)?;

    let font = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Font".to_vec())),
        ("Subtype", Object::Name(b"Type1".to_vec())),
        ("BaseFont", Object::Name(b"Helvetica".to_vec())),
        ("Encoding", Object::Name(b"WinAnsiEncoding".to_vec())),
    ]);
    let font_id = doc.add_object(font);

    // Check if page has Resources, create if missing
    let res_ref = {
        let page_dict = doc.get_dictionary_mut(page_id)?;
        if page_dict.get(b"Resources").is_err() {
            page_dict.set("Resources", Dictionary::new());
        }
        match page_dict.get(b"Resources") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };

    if let Some(res_id) = res_ref {
        // Resources is an indirect object
        insert_font_into_resources_obj(doc, res_id, font_id)?;
    } else {
        // Resources is inline in page_dict
        let font_ref = {
            let page_dict = doc.get_dictionary_mut(page_id)?;
            if let Ok(Object::Dictionary(ref mut res_dict)) = page_dict.get_mut(b"Resources") {
                match res_dict.get_mut(b"Font") {
                    Ok(Object::Reference(id)) => Some(*id),
                    Ok(Object::Dictionary(ref mut fonts)) => {
                        fonts.set(PDFKIT_FONT_NAME.to_vec(), Object::Reference(font_id));
                        None
                    }
                    _ => {
                        let mut fonts = Dictionary::new();
                        fonts.set(PDFKIT_FONT_NAME.to_vec(), Object::Reference(font_id));
                        res_dict.set("Font", Object::Dictionary(fonts));
                        None
                    }
                }
            } else {
                None
            }
        };

        if let Some(font_obj_id) = font_ref {
            if let Ok(fonts) = doc.get_object_mut(font_obj_id).and_then(|o| o.as_dict_mut()) {
                fonts.set(PDFKIT_FONT_NAME.to_vec(), Object::Reference(font_id));
            }
        }
    }

    Ok(())
}

fn insert_font_into_resources_obj(
    doc: &mut Document,
    res_id: ObjectId,
    font_id: ObjectId,
) -> Result<(), PdfError> {
    let font_ref = {
        if let Ok(res_dict) = doc.get_object(res_id).and_then(|o| o.as_dict()) {
            match res_dict.get(b"Font") {
                Ok(Object::Reference(f_id)) => Some(*f_id),
                _ => None,
            }
        } else {
            None
        }
    };

    if let Some(font_obj_id) = font_ref {
        if let Ok(fonts) = doc.get_object_mut(font_obj_id).and_then(|o| o.as_dict_mut()) {
            fonts.set(PDFKIT_FONT_NAME.to_vec(), Object::Reference(font_id));
            return Ok(());
        }
    }

    if let Ok(res_dict) = doc.get_object_mut(res_id).and_then(|o| o.as_dict_mut()) {
        match res_dict.get_mut(b"Font") {
            Ok(Object::Dictionary(ref mut fonts)) => {
                fonts.set(PDFKIT_FONT_NAME.to_vec(), Object::Reference(font_id));
            }
            _ => {
                let mut fonts = Dictionary::new();
                fonts.set(PDFKIT_FONT_NAME.to_vec(), Object::Reference(font_id));
                res_dict.set("Font", Object::Dictionary(fonts));
            }
        }
    }
    Ok(())
}

/// Escapes a Rust string for use inside a PDF literal string `( ... )`.
/// Maps common Turkish characters to safe equivalents for standard WinAnsi Helvetica
/// to prevent displaying '?' marks on Turkish text.
pub fn escape_pdf_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            // Turkish characters fallback for Helvetica Base-14
            'ğ' => out.push('g'),
            'Ğ' => out.push('G'),
            'ı' => out.push('i'),
            'İ' => out.push('I'),
            'ş' => out.push('s'),
            'Ş' => out.push('S'),
            'ç' => out.push('\u{00e7}'), // c-cedilla is in WinAnsi (0xE7)
            'Ç' => out.push('\u{00c7}'), // C-cedilla is in WinAnsi (0xC7)
            'ö' => out.push('\u{00f6}'), // o-diaeresis is in WinAnsi (0xF6)
            'Ö' => out.push('\u{00d6}'), // O-diaeresis is in WinAnsi (0xD6)
            'ü' => out.push('\u{00fc}'), // u-diaeresis is in WinAnsi (0xFC)
            'Ü' => out.push('\u{00dc}'), // U-diaeresis is in WinAnsi (0xDC)
            // Fallback for other characters outside WinAnsi
            c if (c as u32) > 0xFF => out.push('?'),
            c => out.push(c),
        }
    }
    out
}
