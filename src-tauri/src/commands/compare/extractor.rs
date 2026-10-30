use crate::commands::compare::types::{BoundingBox, EnrichedPageData};
use crate::commands::forms::decode_pdf_string;
use crate::error::PdfError;
use crate::pdfutil::{inherited_value, media_box};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ExtractedWord {
    pub text: String,
    pub bbox: BoundingBox,
    pub font_name: String,
    pub font_size: f64,
    pub color_rgb: [u8; 3],
    pub in_header_zone: bool,
    pub in_footer_zone: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractedLine {
    pub words: Vec<ExtractedWord>,
    pub text: String,
    pub bbox: BoundingBox,
    pub font_name: String,
    pub font_size: f64,
    pub color_rgb: [u8; 3],
    pub in_header_zone: bool,
    pub in_footer_zone: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractedImage {
    pub name: String,
    pub bbox: BoundingBox,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub content_hash: u64,
    pub is_background: bool,
    /// Optional downsampled 16x16 RGB thumbnail for structural/visual comparison of embedded images
    pub sample_rgb: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ExtractedVector {
    pub kind: String, // "rect", "line", "curve_path"
    pub bbox: BoundingBox,
    pub stroke_rgb: Option<[u8; 3]>,
    pub fill_rgb: Option<[u8; 3]>,
    pub line_width: f64,
    pub op_count: usize,
    pub is_background: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractedAnnotation {
    pub subtype: String,
    pub bbox: BoundingBox,
    pub contents: String,
    pub author: String,
    pub color_rgb: Option<[u8; 3]>,
}

#[derive(Debug, Clone)]
pub struct ExtractedFormField {
    pub name: String,
    pub field_type: String,
    pub value: String,
    pub bbox: Option<BoundingBox>,
    pub page: u32,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractedPage {
    pub page_number: u32,
    pub media_box: [f64; 4],
    pub crop_box: Option<[f64; 4]>,
    pub rotation: i64,
    pub width_pt: f64,
    pub height_pt: f64,
    pub words: Vec<ExtractedWord>,
    pub lines: Vec<ExtractedLine>,
    pub images: Vec<ExtractedImage>,
    pub vectors: Vec<ExtractedVector>,
    pub annotations: Vec<ExtractedAnnotation>,
    pub form_fields: Vec<ExtractedFormField>,
    pub is_scanned_candidate: bool,
    pub has_invisible_ocr_text: bool,
    pub has_table_grid: bool,
    pub unsupported_features: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExtractedDocument {
    pub path: String,
    pub page_count: u32,
    pub pages: Vec<ExtractedPage>,
    pub form_fields: Vec<ExtractedFormField>,
    pub unsupported_features: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct FontDecodeMap {
    base_font: String,
    to_unicode: HashMap<u16, String>,
    differences: HashMap<u8, char>,
    is_two_byte: bool,
}

#[derive(Debug, Clone, Copy)]
struct GraphicsState {
    ctm: [f64; 6],
    fill_rgb: [u8; 3],
    stroke_rgb: [u8; 3],
    line_width: f64,
    font_key: [u8; 32],
    font_key_len: usize,
    font_size: f64,
    text_render_mode: i64,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            fill_rgb: [0, 0, 0],
            stroke_rgb: [0, 0, 0],
            line_width: 1.0,
            font_key: [0; 32],
            font_key_len: 0,
            font_size: 12.0,
            text_render_mode: 0,
        }
    }
}

impl GraphicsState {
    fn font_key_str(&self) -> String {
        String::from_utf8_lossy(&self.font_key[..self.font_key_len]).into_owned()
    }

    fn set_font_key(&mut self, name: &[u8]) {
        let len = name.len().min(32);
        self.font_key[..len].copy_from_slice(&name[..len]);
        self.font_key_len = len;
    }
}

/// Extracts complete structured information from a PDF file on disk.
pub fn extract_document(
    path: &Path,
    enriched_pages: Option<&[EnrichedPageData]>,
) -> Result<ExtractedDocument, PdfError> {
    if !path.exists() {
        return Err(PdfError::Custom(format!(
            "Dosya bulunamadı: {}",
            path.display()
        )));
    }

    let doc = Document::load(path).map_err(|e| {
        PdfError::Custom(format!(
            "PDF dosyası okunamadı veya bozuk ({}): {}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("bilinmeyen"),
            e
        ))
    })?;

    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let pages_map = doc.get_pages();
    if pages_map.is_empty() {
        return Err(PdfError::EmptyOutput);
    }

    let enriched_lookup: HashMap<u32, &EnrichedPageData> = enriched_pages
        .unwrap_or(&[])
        .iter()
        .map(|p| (p.page, p))
        .collect();

    // Extract document-wide AcroForm fields and map them to pages where possible
    let doc_form_fields = extract_document_form_fields(&doc, &pages_map);

    let mut extracted_pages = Vec::with_capacity(pages_map.len());
    let mut doc_unsupported = Vec::new();

    for (&page_num, &page_id) in &pages_map {
        let page_forms: Vec<ExtractedFormField> = doc_form_fields
            .iter()
            .filter(|f| f.page == page_num || f.page == 0)
            .cloned()
            .collect();

        let page = extract_single_page(
            &doc,
            page_num,
            page_id,
            page_forms,
            enriched_lookup.get(&page_num).copied(),
        )?;

        for feat in &page.unsupported_features {
            if !doc_unsupported.contains(feat) {
                doc_unsupported.push(feat.clone());
            }
        }

        extracted_pages.push(page);
    }

    // Refine header/footer zone flags across multi-page documents
    refine_header_footer_zones(&mut extracted_pages);

    Ok(ExtractedDocument {
        path: path.to_string_lossy().into_owned(),
        page_count: pages_map.len() as u32,
        pages: extracted_pages,
        form_fields: doc_form_fields,
        unsupported_features: doc_unsupported,
    })
}

fn extract_single_page(
    doc: &Document,
    page_number: u32,
    page_id: ObjectId,
    form_fields: Vec<ExtractedFormField>,
    enriched: Option<&EnrichedPageData>,
) -> Result<ExtractedPage, PdfError> {
    let mbox = media_box(doc, page_id);
    let cbox = crop_box(doc, page_id);
    let rotation = page_rotation(doc, page_id);

    let effective_box = cbox.unwrap_or(mbox);
    let raw_w = (effective_box[2] - effective_box[0]).abs().max(1.0);
    let raw_h = (effective_box[3] - effective_box[1]).abs().max(1.0);
    let (width_pt, height_pt) = if rotation == 90 || rotation == 270 {
        (raw_h, raw_w)
    } else {
        (raw_w, raw_h)
    };

    let font_maps = extract_page_fonts(doc, page_id);
    let mut words = Vec::new();
    let mut images = Vec::new();
    let mut vectors = Vec::new();
    let mut unsupported_features = Vec::new();
    let mut has_invisible_ocr_text = false;

    let content_bytes = doc.get_page_content(page_id);
    parse_content_stream(
        doc,
        page_id,
        &content_bytes,
        &font_maps,
        effective_box,
        width_pt,
        height_pt,
        GraphicsState::default(),
        0,
        &mut words,
        &mut images,
        &mut vectors,
        &mut has_invisible_ocr_text,
        &mut unsupported_features,
    );

    // If frontend provided pdf.js enriched spans and our stream had custom unmapped glyphs (or fewer words),
    // merge/enrich words while preserving style/color metadata from stream where overlapping!
    if let Some(enr) = enriched {
        if !enr.spans.is_empty() {
            let mut enriched_words = Vec::new();
            for span in &enr.spans {
                let trimmed = span.text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Match against nearest stream word to inherit RGB color and real PDF /BaseFont if available
                let span_bbox = BoundingBox::new(span.x, span.y, span.width.max(2.0), span.height.max(4.0));
                let nearest_word = words
                    .iter()
                    .find(|w| w.bbox.intersects_or_near(&span_bbox, 6.0));
                let nearest_color = nearest_word
                    .map(|w| w.color_rgb)
                    .unwrap_or([0, 0, 0]);
                let nearest_font = nearest_word
                    .map(|w| clean_font_name(&w.font_name))
                    .filter(|f| !f.is_empty() && f != "Default")
                    .unwrap_or_else(|| {
                        if !span.font_name.is_empty() {
                            clean_font_name(&span.font_name)
                        } else {
                            "Default".to_string()
                        }
                    });
                let font_size = if span.font_size > 0.0 {
                    span.font_size
                } else {
                    span.height.max(10.0)
                };

                split_span_into_words(
                    trimmed,
                    span_bbox,
                    &nearest_font,
                    font_size,
                    nearest_color,
                    height_pt,
                    &mut enriched_words,
                );
            }
            if !enriched_words.is_empty() {
                words = enriched_words;
            }
        }
    }

    let lines = group_words_into_visual_lines(&words, width_pt, height_pt);
    let annotations = extract_page_annotations(doc, page_id, effective_box, height_pt);

    // Detect table grid: >= 4 horizontal/vertical lines or >= 4 aligned rectangular cells
    let rect_or_line_count = vectors
        .iter()
        .filter(|v| !v.is_background && (v.kind == "rect" || v.kind == "line"))
        .count();
    let has_table_grid = rect_or_line_count >= 4;

    // Detect scanned candidate: no visible words (or only invisible OCR text) + large image XObject
    let has_large_image = images
        .iter()
        .any(|img| img.bbox.area() >= (width_pt * height_pt * 0.30) || (img.pixel_width >= 300 && img.pixel_height >= 300));
    let is_scanned_candidate = (words.is_empty() || has_invisible_ocr_text) && has_large_image;

    Ok(ExtractedPage {
        page_number,
        media_box: mbox,
        crop_box: cbox,
        rotation,
        width_pt,
        height_pt,
        words,
        lines,
        images,
        vectors,
        annotations,
        form_fields,
        is_scanned_candidate,
        has_invisible_ocr_text,
        has_table_grid,
        unsupported_features,
    })
}

fn crop_box(doc: &Document, page_id: ObjectId) -> Option<[f64; 4]> {
    let Some(Object::Array(items)) = inherited_value(doc, page_id, b"CropBox") else {
        return None;
    };
    if items.len() < 4 {
        return None;
    }
    let mut out = [0.0; 4];
    for (i, item) in items.iter().take(4).enumerate() {
        out[i] = obj_to_f64(item)?;
    }
    Some(out)
}

fn page_rotation(doc: &Document, page_id: ObjectId) -> i64 {
    let raw = match inherited_value(doc, page_id, b"Rotate") {
        Some(Object::Integer(n)) => n,
        Some(Object::Real(n)) => n as i64,
        _ => 0,
    };
    raw.rem_euclid(360)
}

fn obj_to_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(n) => Some(*n as f64),
        Object::Real(n) => Some(*n as f64),
        _ => None,
    }
}

pub fn clean_font_name(raw: &str) -> String {
    let mut trimmed = raw.trim().trim_start_matches('/');
    // Strip pdf.js internal document instance prefix like "g_d16_" or "g_d0_"
    if let Some(rest) = trimmed.strip_prefix("g_d") {
        if let Some(underscore_idx) = rest.find('_') {
            if underscore_idx > 0 && rest[..underscore_idx].chars().all(|c| c.is_ascii_digit()) {
                trimmed = &rest[underscore_idx + 1..];
            }
        }
    }
    // Strip pdf.js synthetic font counter prefix like "f1_sans-serif" -> "sans-serif" or standalone "f1" / "F1" -> "Default"
    if let Some(rest) = trimmed.strip_prefix('f').or_else(|| trimmed.strip_prefix('F')) {
        if let Some(underscore_idx) = rest.find('_') {
            if underscore_idx > 0 && rest[..underscore_idx].chars().all(|c| c.is_ascii_digit()) {
                trimmed = &rest[underscore_idx + 1..];
            }
        } else if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            return "Default".to_string();
        }
    }
    // Remove 6-letter uppercase subset tag like "ABCDEF+Helvetica-Bold"
    if let Some(plus_idx) = trimmed.find('+') {
        if plus_idx == 6 && trimmed[..6].chars().all(|c| c.is_ascii_uppercase()) {
            trimmed = &trimmed[7..];
        }
    }
    if trimmed.is_empty() {
        "Default".to_string()
    } else {
        trimmed.to_string()
    }
}

fn extract_page_fonts(doc: &Document, page_id: ObjectId) -> HashMap<String, FontDecodeMap> {
    let mut out = HashMap::new();
    let Some(res_obj) = inherited_value(doc, page_id, b"Resources") else {
        return out;
    };
    let Ok((_, res_deref)) = doc.dereference(&res_obj) else {
        return out;
    };
    let Ok(res_dict) = res_deref.as_dict() else {
        return out;
    };
    let Ok(font_obj) = res_dict.get(b"Font") else {
        return out;
    };
    let Ok((_, font_deref)) = doc.dereference(font_obj) else {
        return out;
    };
    let Ok(font_dict) = font_deref.as_dict() else {
        return out;
    };

    for (k, v) in font_dict.iter() {
        let key_str = String::from_utf8_lossy(k).into_owned();
        if let Ok((_, f_obj)) = doc.dereference(v) {
            if let Ok(f_dict) = f_obj.as_dict() {
                let mut map = FontDecodeMap::default();
                if let Ok(base) = f_dict.get(b"BaseFont").and_then(|o| o.as_name()) {
                    map.base_font = clean_font_name(&String::from_utf8_lossy(base));
                } else {
                    map.base_font = key_str.clone();
                }

                if let Ok(subtype) = f_dict.get(b"Subtype").and_then(|o| o.as_name()) {
                    if subtype == b"Type0" {
                        map.is_two_byte = true;
                    }
                }

                // Parse /ToUnicode stream if present
                if let Ok(tu_ref) = f_dict.get(b"ToUnicode") {
                    if let Ok((_, Object::Stream(stream))) = doc.dereference(tu_ref) {
                        let bytes = stream
                            .decompressed_content()
                            .unwrap_or_else(|_| stream.content.clone());
                        let cmap_text = String::from_utf8_lossy(&bytes);
                        parse_tounicode_cmap(&cmap_text, &mut map.to_unicode);
                    }
                }

                // Parse /Encoding /Differences if present
                if let Ok(enc_ref) = f_dict.get(b"Encoding") {
                    if let Ok((_, Object::Dictionary(enc_dict))) = doc.dereference(enc_ref) {
                        if let Ok(Object::Array(diffs)) = enc_dict.get(b"Differences") {
                            let mut code: u16 = 0;
                            for item in diffs {
                                match item {
                                    Object::Integer(n) => {
                                        code = (*n).clamp(0, 255) as u16;
                                    }
                                    Object::Name(glyph_name) => {
                                        if let Some(ch) = glyph_name_to_char(glyph_name) {
                                            map.differences.insert(code as u8, ch);
                                        }
                                        code = code.saturating_add(1);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                out.insert(key_str, map);
            }
        }
    }
    out
}

fn parse_tounicode_cmap(cmap: &str, out: &mut HashMap<u16, String>) {
    // Parse `<XXXX> <YYYY>` in beginbfchar sections and `<XXXX> <YYYY> <ZZZZ>` in beginbfrange sections
    let tokens: Vec<&str> = cmap.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].ends_with("beginbfchar") {
            i += 1;
            while i < tokens.len() && !tokens[i].starts_with("endbfchar") {
                if i + 1 < tokens.len() {
                    if let (Some(src), Some(dst)) =
                        (parse_hex_u16(tokens[i]), parse_hex_unicode_str(tokens[i + 1]))
                    {
                        out.insert(src, dst);
                        i += 2;
                        continue;
                    }
                }
                i += 1;
            }
        } else if tokens[i].ends_with("beginbfrange") {
            i += 1;
            while i < tokens.len() && !tokens[i].starts_with("endbfrange") {
                if i + 2 < tokens.len() {
                    if let (Some(start), Some(end), Some(base_str)) = (
                        parse_hex_u16(tokens[i]),
                        parse_hex_u16(tokens[i + 1]),
                        parse_hex_unicode_str(tokens[i + 2]),
                    ) {
                        if let Some(first_ch) = base_str.chars().next() {
                            let base_u32 = first_ch as u32;
                            for offset in 0..=(end.saturating_sub(start)).min(256) {
                                if let Some(ch) = char::from_u32(base_u32 + offset as u32) {
                                    out.insert(start + offset, ch.to_string());
                                }
                            }
                        }
                        i += 3;
                        continue;
                    }
                }
                i += 1;
            }
        }
        i += 1;
    }
}

fn parse_hex_u16(token: &str) -> Option<u16> {
    let clean = token.trim_start_matches('<').trim_end_matches('>');
    if clean.is_empty() || clean.len() > 4 {
        return None;
    }
    u16::from_str_radix(clean, 16).ok()
}

fn parse_hex_unicode_str(token: &str) -> Option<String> {
    let clean = token.trim_start_matches('<').trim_end_matches('>');
    if clean.is_empty() {
        return None;
    }
    if clean.len() <= 4 {
        let code = u32::from_str_radix(clean, 16).ok()?;
        return char::from_u32(code).map(|c| c.to_string());
    }
    let mut u16s = Vec::new();
    let mut idx = 0;
    while idx + 4 <= clean.len() {
        if let Ok(val) = u16::from_str_radix(&clean[idx..idx + 4], 16) {
            u16s.push(val);
        }
        idx += 4;
    }
    if u16s.is_empty() {
        None
    } else {
        Some(String::from_utf16_lossy(&u16s))
    }
}

fn glyph_name_to_char(name: &[u8]) -> Option<char> {
    match name {
        b"space" => Some(' '),
        b"Idotaccent" => Some('İ'),
        b"dotlessi" => Some('ı'),
        b"Gbreve" => Some('Ğ'),
        b"gbreve" => Some('ğ'),
        b"Scedilla" | b"Scommaaccent" => Some('Ş'),
        b"scedilla" | b"scommaaccent" => Some('ş'),
        b"Ccedilla" => Some('Ç'),
        b"ccedilla" => Some('ç'),
        b"Odieresis" => Some('Ö'),
        b"odieresis" => Some('ö'),
        b"Udieresis" => Some('Ü'),
        b"udieresis" => Some('ü'),
        b"fi" => Some('ﬁ'),
        b"fl" => Some('ﬂ'),
        b"endash" | b"emdash" | b"hyphen" | b"minus" => Some('-'),
        _ if name.len() == 1 && name[0].is_ascii_graphic() => Some(name[0] as char),
        _ => None,
    }
}

fn decode_text_operand(bytes: &[u8], font_map: Option<&FontDecodeMap>) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    // 1. UTF-16BE with BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_pdf_string(bytes);
    }

    // 2. Font ToUnicode CMap
    if let Some(fm) = font_map {
        if !fm.to_unicode.is_empty() {
            if fm.is_two_byte && bytes.len() % 2 == 0 {
                let mut s = String::new();
                for chunk in bytes.chunks_exact(2) {
                    let code = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
                    if let Some(mapped) = fm.to_unicode.get(&code) {
                        s.push_str(mapped);
                    } else if let Some(ch) = char::from_u32(code as u32) {
                        if !ch.is_control() {
                            s.push(ch);
                        }
                    }
                }
                if !s.is_empty() {
                    return s;
                }
            } else {
                let mut s = String::new();
                for &b in bytes {
                    if let Some(mapped) = fm.to_unicode.get(&(b as u16)) {
                        s.push_str(mapped);
                    } else if let Some(&ch) = fm.differences.get(&b) {
                        s.push(ch);
                    } else {
                        s.push(winansi_or_cp1254_byte(b));
                    }
                }
                if !s.is_empty() {
                    return s;
                }
            }
        }

        if !fm.differences.is_empty() {
            let mut s = String::with_capacity(bytes.len());
            for &b in bytes {
                if let Some(&ch) = fm.differences.get(&b) {
                    s.push(ch);
                } else {
                    s.push(winansi_or_cp1254_byte(b));
                }
            }
            return s;
        }
    }

    // 3. Valid UTF-8 (including multi-byte Turkish UTF-8 strings in content streams)
    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        return utf8_str.to_string();
    }

    // 4. CP1254 / WinAnsi fallback
    bytes.iter().map(|&b| winansi_or_cp1254_byte(b)).collect()
}

fn winansi_or_cp1254_byte(b: u8) -> char {
    match b {
        0xD0 => 'Ğ',
        0xDD => 'İ',
        0xDE => 'Ş',
        0xF0 => 'ğ',
        0xFD => 'ı',
        0xFE => 'ş',
        0xC7 => 'Ç',
        0xE7 => 'ç',
        0xD6 => 'Ö',
        0xF6 => 'ö',
        0xDC => 'Ü',
        0xFC => 'ü',
        _ => b as char,
    }
}

fn multiply_matrix(m1: [f64; 6], m2: [f64; 6]) -> [f64; 6] {
    [
        m1[0] * m2[0] + m1[1] * m2[2],
        m1[0] * m2[1] + m1[1] * m2[3],
        m1[2] * m2[0] + m1[3] * m2[2],
        m1[2] * m2[1] + m1[3] * m2[3],
        m1[4] * m2[0] + m1[5] * m2[2] + m2[4],
        m1[4] * m2[1] + m1[5] * m2[3] + m2[5],
    ]
}

fn transform_point(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

#[allow(clippy::too_many_arguments)]
fn parse_content_stream(
    doc: &Document,
    page_id: ObjectId,
    content_bytes: &[u8],
    font_maps: &HashMap<String, FontDecodeMap>,
    effective_box: [f64; 4],
    width_pt: f64,
    height_pt: f64,
    initial_gs: GraphicsState,
    depth: usize,
    words: &mut Vec<ExtractedWord>,
    images: &mut Vec<ExtractedImage>,
    vectors: &mut Vec<ExtractedVector>,
    has_invisible_ocr_text: &mut bool,
    unsupported_features: &mut Vec<String>,
) {
    if depth > 5 || content_bytes.is_empty() {
        return;
    }

    let Ok(content) = Content::decode(content_bytes) else {
        return;
    };

    let mut gs_stack: Vec<GraphicsState> = Vec::new();
    let mut gs = initial_gs;

    // Text state within BT..ET
    let mut text_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut text_line_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut text_leading = 14.0;

    // Current vector path accumulator
    struct PathBounds {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        op_count: usize,
        kind: &'static str,
    }
    impl PathBounds {
        fn new() -> Self {
            Self {
                min_x: f64::INFINITY,
                min_y: f64::INFINITY,
                max_x: f64::NEG_INFINITY,
                max_y: f64::NEG_INFINITY,
                op_count: 0,
                kind: "path",
            }
        }
        fn record(&mut self, px: f64, py: f64, ctm: [f64; 6], origin_x: f64, origin_y: f64, height_pt: f64) {
            let (tx, ty) = transform_point(ctm, px, py);
            let vx = tx - origin_x;
            let vy = height_pt - (ty - origin_y);
            self.min_x = self.min_x.min(vx);
            self.min_y = self.min_y.min(vy);
            self.max_x = self.max_x.max(vx);
            self.max_y = self.max_y.max(vy);
        }
        fn reset(&mut self) {
            *self = Self::new();
        }
    }

    let mut pb = PathBounds::new();
    let origin_x = effective_box[0];
    let origin_y = effective_box[1];

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => {
                gs_stack.push(gs);
            }
            "Q" => {
                if let Some(prev) = gs_stack.pop() {
                    gs = prev;
                }
            }
            "cm" if op.operands.len() >= 6 => {
                let mut m = [0.0; 6];
                for (i, item) in op.operands.iter().take(6).enumerate() {
                    m[i] = obj_to_f64(item).unwrap_or(0.0);
                }
                gs.ctm = multiply_matrix(m, gs.ctm);
            }
            "w" if !op.operands.is_empty() => {
                if let Some(w) = obj_to_f64(&op.operands[0]) {
                    gs.line_width = w;
                }
            }
            "rg" | "sc" | "scn" if op.operands.len() >= 3 => {
                let r = (obj_to_f64(&op.operands[0]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                let g = (obj_to_f64(&op.operands[1]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                let b = (obj_to_f64(&op.operands[2]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                gs.fill_rgb = [r, g, b];
            }
            "RG" | "SC" | "SCN" if op.operands.len() >= 3 => {
                let r = (obj_to_f64(&op.operands[0]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                let g = (obj_to_f64(&op.operands[1]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                let b = (obj_to_f64(&op.operands[2]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                gs.stroke_rgb = [r, g, b];
            }
            "g" if !op.operands.is_empty() => {
                let v = (obj_to_f64(&op.operands[0]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                gs.fill_rgb = [v, v, v];
            }
            "G" if !op.operands.is_empty() => {
                let v = (obj_to_f64(&op.operands[0]).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                gs.stroke_rgb = [v, v, v];
            }
            "k" if op.operands.len() >= 4 => {
                let c = obj_to_f64(&op.operands[0]).unwrap_or(0.0).clamp(0.0, 1.0);
                let m = obj_to_f64(&op.operands[1]).unwrap_or(0.0).clamp(0.0, 1.0);
                let y = obj_to_f64(&op.operands[2]).unwrap_or(0.0).clamp(0.0, 1.0);
                let k = obj_to_f64(&op.operands[3]).unwrap_or(0.0).clamp(0.0, 1.0);
                let r = ((1.0 - c) * (1.0 - k) * 255.0).round() as u8;
                let g = ((1.0 - m) * (1.0 - k) * 255.0).round() as u8;
                let b = ((1.0 - y) * (1.0 - k) * 255.0).round() as u8;
                gs.fill_rgb = [r, g, b];
            }
            "K" if op.operands.len() >= 4 => {
                let c = obj_to_f64(&op.operands[0]).unwrap_or(0.0).clamp(0.0, 1.0);
                let m = obj_to_f64(&op.operands[1]).unwrap_or(0.0).clamp(0.0, 1.0);
                let y = obj_to_f64(&op.operands[2]).unwrap_or(0.0).clamp(0.0, 1.0);
                let k = obj_to_f64(&op.operands[3]).unwrap_or(0.0).clamp(0.0, 1.0);
                let r = ((1.0 - c) * (1.0 - k) * 255.0).round() as u8;
                let g = ((1.0 - m) * (1.0 - k) * 255.0).round() as u8;
                let b = ((1.0 - y) * (1.0 - k) * 255.0).round() as u8;
                gs.stroke_rgb = [r, g, b];
            }
            "BT" => {
                text_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                text_line_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
            }
            "Tf" if op.operands.len() >= 2 => {
                if let Ok(name) = op.operands[0].as_name() {
                    gs.set_font_key(name);
                }
                if let Some(sz) = obj_to_f64(&op.operands[1]) {
                    gs.font_size = sz.abs();
                }
            }
            "Tr" if !op.operands.is_empty() => {
                if let Ok(mode) = op.operands[0].as_i64() {
                    gs.text_render_mode = mode;
                }
            }
            "TL" if !op.operands.is_empty() => {
                if let Some(lead) = obj_to_f64(&op.operands[0]) {
                    text_leading = lead;
                }
            }
            "Tm" if op.operands.len() >= 6 => {
                let mut m = [0.0; 6];
                for (i, item) in op.operands.iter().take(6).enumerate() {
                    m[i] = obj_to_f64(item).unwrap_or(0.0);
                }
                text_matrix = m;
                text_line_matrix = m;
            }
            "Td" | "TD" if op.operands.len() >= 2 => {
                let tx = obj_to_f64(&op.operands[0]).unwrap_or(0.0);
                let ty = obj_to_f64(&op.operands[1]).unwrap_or(0.0);
                if op.operator == "TD" {
                    text_leading = -ty;
                }
                let trans = [1.0, 0.0, 0.0, 1.0, tx, ty];
                text_line_matrix = multiply_matrix(trans, text_line_matrix);
                text_matrix = text_line_matrix;
            }
            "T*" => {
                let trans = [1.0, 0.0, 0.0, 1.0, 0.0, -text_leading];
                text_line_matrix = multiply_matrix(trans, text_line_matrix);
                text_matrix = text_line_matrix;
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if op.operator == "'" || op.operator == "\"" {
                    let trans = [1.0, 0.0, 0.0, 1.0, 0.0, -text_leading];
                    text_line_matrix = multiply_matrix(trans, text_line_matrix);
                    text_matrix = text_line_matrix;
                }

                let fkey = gs.font_key_str();
                let fmap = font_maps.get(&fkey);
                let base_font = fmap
                    .map(|f| f.base_font.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| if fkey.is_empty() { "Helvetica".to_string() } else { fkey });

                let decoded = extract_text_from_operands(&op.operands, fmap);
                if gs.text_render_mode == 3 {
                    if !decoded.trim().is_empty() {
                        *has_invisible_ocr_text = true;
                    }
                    continue;
                }

                let combined = multiply_matrix(text_matrix, gs.ctm);
                let scale_factor = (combined[0].hypot(combined[1]))
                    .max(combined[2].hypot(combined[3]))
                    .max(0.1);
                let effective_font_size = (gs.font_size * scale_factor).max(4.0);
                let tx_pt = combined[4] - origin_x;
                let ty_pt = combined[5] - origin_y;
                // Convert PDF bottom-left baseline (tx_pt, ty_pt) to top-left bounding box
                let y_top = (height_pt - ty_pt - effective_font_size * 0.85).clamp(0.0, (height_pt - 2.0).max(0.0));
                let x_left = tx_pt.clamp(0.0, (width_pt - 2.0).max(0.0));
                let est_width = (decoded.chars().count() as f64 * effective_font_size * 0.52).max(4.0);

                let span_bbox = BoundingBox::new(x_left, y_top, est_width, effective_font_size * 1.15);
                split_span_into_words(
                    &decoded,
                    span_bbox,
                    &base_font,
                    (effective_font_size * 10.0).round() / 10.0,
                    gs.fill_rgb,
                    height_pt,
                    words,
                );

                // Advance text_matrix X position for consecutive Tj calls on the same line
                text_matrix[4] += decoded.chars().count() as f64 * gs.font_size * 0.52;
            }
            // Vector Path Construction
            "m" | "l" if op.operands.len() >= 2 => {
                if let (Some(x), Some(y)) = (obj_to_f64(&op.operands[0]), obj_to_f64(&op.operands[1])) {
                    pb.record(x, y, gs.ctm, origin_x, origin_y, height_pt);
                    pb.op_count += 1;
                    if op.operator == "l" && pb.kind == "path" {
                        pb.kind = "line";
                    }
                }
            }
            "c" if op.operands.len() >= 6 => {
                if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x3), Some(y3)) = (
                    obj_to_f64(&op.operands[0]),
                    obj_to_f64(&op.operands[1]),
                    obj_to_f64(&op.operands[2]),
                    obj_to_f64(&op.operands[3]),
                    obj_to_f64(&op.operands[4]),
                    obj_to_f64(&op.operands[5]),
                ) {
                    pb.record(x1, y1, gs.ctm, origin_x, origin_y, height_pt);
                    pb.record(x2, y2, gs.ctm, origin_x, origin_y, height_pt);
                    pb.record(x3, y3, gs.ctm, origin_x, origin_y, height_pt);
                    pb.op_count += 1;
                    pb.kind = "curve_path";
                }
            }
            "re" if op.operands.len() >= 4 => {
                if let (Some(rx), Some(ry), Some(rw), Some(rh)) = (
                    obj_to_f64(&op.operands[0]),
                    obj_to_f64(&op.operands[1]),
                    obj_to_f64(&op.operands[2]),
                    obj_to_f64(&op.operands[3]),
                ) {
                    pb.record(rx, ry, gs.ctm, origin_x, origin_y, height_pt);
                    pb.record(rx + rw, ry + rh, gs.ctm, origin_x, origin_y, height_pt);
                    pb.op_count += 1;
                    pb.kind = "rect";
                }
            }
            // Path Painting
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                if pb.op_count > 0 && pb.min_x.is_finite() && pb.min_y.is_finite() {
                    let w = (pb.max_x - pb.min_x).abs().max(1.0);
                    let h = (pb.max_y - pb.min_y).abs().max(1.0);
                    let bbox = BoundingBox::new(pb.min_x.max(0.0), pb.min_y.max(0.0), w, h);
                    let is_stroke = matches!(op.operator.as_str(), "S" | "s" | "B" | "B*" | "b" | "b*");
                    let is_fill = matches!(op.operator.as_str(), "f" | "F" | "f*" | "B" | "B*" | "b" | "b*");
                    let is_bg = is_fill && (bbox.area() >= width_pt * height_pt * 0.72);

                    vectors.push(ExtractedVector {
                        kind: pb.kind.to_string(),
                        bbox,
                        stroke_rgb: if is_stroke { Some(gs.stroke_rgb) } else { None },
                        fill_rgb: if is_fill { Some(gs.fill_rgb) } else { None },
                        line_width: gs.line_width,
                        op_count: pb.op_count,
                        is_background: is_bg,
                    });
                }
                pb.reset();
            }
            "n" => {
                pb.reset();
            }
            "sh" => {
                unsupported_features.push("Shading (degrade dolgu) operatörü algılandı; karmaşık vektör degradeleri görsel katmanda karşılaştırılır.".to_string());
            }
            "Do" if !op.operands.is_empty() => {
                if let Ok(xobj_name) = op.operands[0].as_name() {
                    let name_str = String::from_utf8_lossy(xobj_name).into_owned();
                    inspect_xobject_invocation(
                        doc,
                        page_id,
                        xobj_name,
                        &name_str,
                        gs,
                        font_maps,
                        effective_box,
                        width_pt,
                        height_pt,
                        depth,
                        words,
                        images,
                        vectors,
                        has_invisible_ocr_text,
                        unsupported_features,
                    );
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn inspect_xobject_invocation(
    doc: &Document,
    page_id: ObjectId,
    xobj_name: &[u8],
    name_str: &str,
    gs: GraphicsState,
    font_maps: &HashMap<String, FontDecodeMap>,
    effective_box: [f64; 4],
    width_pt: f64,
    height_pt: f64,
    depth: usize,
    words: &mut Vec<ExtractedWord>,
    images: &mut Vec<ExtractedImage>,
    vectors: &mut Vec<ExtractedVector>,
    has_invisible_ocr_text: &mut bool,
    unsupported_features: &mut Vec<String>,
) {
    let Some(res_obj) = inherited_value(doc, page_id, b"Resources") else {
        return;
    };
    let Ok((_, res_deref)) = doc.dereference(&res_obj) else {
        return;
    };
    let Ok(res_dict) = res_deref.as_dict() else {
        return;
    };
    let Ok(xobj_container) = res_dict.get(b"XObject") else {
        return;
    };
    let Ok((_, xobj_dict_obj)) = doc.dereference(xobj_container) else {
        return;
    };
    let Ok(xobj_dict) = xobj_dict_obj.as_dict() else {
        return;
    };
    let Ok(target_ref) = xobj_dict.get(xobj_name) else {
        return;
    };
    let Ok((_, Object::Stream(stream))) = doc.dereference(target_ref) else {
        return;
    };

    let subtype = stream
        .dict
        .get(b"Subtype")
        .and_then(|o| o.as_name())
        .unwrap_or(b"");

    if subtype == b"Image" {
        let px_w = stream
            .dict
            .get(b"Width")
            .and_then(|o| o.as_i64())
            .unwrap_or(0)
            .max(0) as u32;
        let px_h = stream
            .dict
            .get(b"Height")
            .and_then(|o| o.as_i64())
            .unwrap_or(0)
            .max(0) as u32;

        // Compute image bounding box from current CTM applied to unit square [0,0]..[1,1]
        let origin_x = effective_box[0];
        let origin_y = effective_box[1];
        let corners = [
            transform_point(gs.ctm, 0.0, 0.0),
            transform_point(gs.ctm, 1.0, 0.0),
            transform_point(gs.ctm, 0.0, 1.0),
            transform_point(gs.ctm, 1.0, 1.0),
        ];
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (cx, cy) in corners {
            let vx = cx - origin_x;
            let vy = height_pt - (cy - origin_y);
            min_x = min_x.min(vx);
            min_y = min_y.min(vy);
            max_x = max_x.max(vx);
            max_y = max_y.max(vy);
        }
        let bbox = BoundingBox::new(
            min_x.clamp(0.0, width_pt),
            min_y.clamp(0.0, height_pt),
            (max_x - min_x).abs().max(2.0),
            (max_y - min_y).abs().max(2.0),
        );

        let raw_bytes = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        let content_hash = fnv1a_hash(&raw_bytes);
        let sample_rgb = build_stream_sample(&raw_bytes, px_w, px_h);
        let is_bg = bbox.area() >= width_pt * height_pt * 0.75;

        images.push(ExtractedImage {
            name: name_str.to_string(),
            bbox,
            pixel_width: px_w,
            pixel_height: px_h,
            content_hash,
            is_background: is_bg,
            sample_rgb,
        });
    } else if subtype == b"Form" {
        let form_bytes = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        let mut next_gs = gs;
        if let Ok(Object::Array(mat)) = stream.dict.get(b"Matrix") {
            if mat.len() >= 6 {
                let mut m = [0.0; 6];
                for (i, item) in mat.iter().take(6).enumerate() {
                    m[i] = obj_to_f64(item).unwrap_or(0.0);
                }
                next_gs.ctm = multiply_matrix(m, next_gs.ctm);
            }
        }
        parse_content_stream(
            doc,
            page_id,
            &form_bytes,
            font_maps,
            effective_box,
            width_pt,
            height_pt,
            next_gs,
            depth + 1,
            words,
            images,
            vectors,
            has_invisible_ocr_text,
            unsupported_features,
        );
    }
}

fn fnv1a_hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn build_stream_sample(raw_bytes: &[u8], _w: u32, _h: u32) -> Vec<u8> {
    if raw_bytes.is_empty() {
        return Vec::new();
    }
    let step = (raw_bytes.len() / 64).max(1);
    raw_bytes.iter().step_by(step).take(64).copied().collect()
}

fn extract_text_from_operands(operands: &[Object], font_map: Option<&FontDecodeMap>) -> String {
    let mut out = String::new();
    for operand in operands {
        match operand {
            Object::String(bytes, _) => {
                out.push_str(&decode_text_operand(bytes, font_map));
            }
            Object::Array(items) => {
                for item in items {
                    match item {
                        Object::String(bytes, _) => {
                            out.push_str(&decode_text_operand(bytes, font_map));
                        }
                        Object::Integer(kern) => {
                            if *kern <= -180 && !out.ends_with(' ') {
                                out.push(' ');
                            }
                        }
                        Object::Real(kern) => {
                            if *kern <= -180.0 && !out.ends_with(' ') {
                                out.push(' ');
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn split_span_into_words(
    span_text: &str,
    span_bbox: BoundingBox,
    font_name: &str,
    font_size: f64,
    color_rgb: [u8; 3],
    page_height: f64,
    out_words: &mut Vec<ExtractedWord>,
) {
    let total_chars = span_text.chars().count().max(1) as f64;
    let in_header = span_bbox.y <= page_height * 0.11;
    let in_footer = span_bbox.bottom() >= page_height * 0.89;

    let mut char_idx = 0usize;
    let chars: Vec<char> = span_text.chars().collect();
    while char_idx < chars.len() {
        while char_idx < chars.len() && chars[char_idx].is_whitespace() {
            char_idx += 1;
        }
        if char_idx >= chars.len() {
            break;
        }
        let start = char_idx;
        while char_idx < chars.len() && !chars[char_idx].is_whitespace() {
            char_idx += 1;
        }
        let word_str: String = chars[start..char_idx].iter().collect();
        let rel_start = start as f64 / total_chars;
        let rel_len = ((char_idx - start) as f64 / total_chars).max(0.05);
        let wx = span_bbox.x + span_bbox.width * rel_start;
        let ww = (span_bbox.width * rel_len).max(2.0);

        out_words.push(ExtractedWord {
            text: word_str,
            bbox: BoundingBox::new(wx, span_bbox.y, ww, span_bbox.height),
            font_name: font_name.to_string(),
            font_size,
            color_rgb,
            in_header_zone: in_header,
            in_footer_zone: in_footer,
        });
    }
}

/// Groups extracted words into visual reading order lines, accounting for multi-column layouts.
fn group_words_into_visual_lines(
    words: &[ExtractedWord],
    page_width: f64,
    page_height: f64,
) -> Vec<ExtractedLine> {
    if words.is_empty() {
        return Vec::new();
    }

    // Separate body words from top/bottom margin words to check if body is 2-column
    let mid_x = page_width * 0.5;
    let gutter_min = page_width * 0.44;
    let gutter_max = page_width * 0.56;

    let body_words: Vec<&ExtractedWord> = words
        .iter()
        .filter(|w| w.bbox.y > page_height * 0.15 && w.bbox.bottom() < page_height * 0.85)
        .collect();

    let mut left_col_count = 0;
    let mut right_col_count = 0;
    let mut crossing_gutter_count = 0;

    for w in &body_words {
        if w.bbox.right() <= gutter_max {
            left_col_count += 1;
        } else if w.bbox.x >= gutter_min {
            right_col_count += 1;
        } else {
            crossing_gutter_count += 1;
        }
    }

    let is_two_column = body_words.len() >= 10
        && left_col_count >= 4
        && right_col_count >= 4
        && crossing_gutter_count * 5 < body_words.len();

    let mut sorted_words = words.to_vec();
    sorted_words.sort_by(|a, b| {
        let col_a = if is_two_column
            && a.bbox.y > page_height * 0.15
            && a.bbox.bottom() < page_height * 0.85
        {
            if a.bbox.x >= mid_x { 1 } else { 0 }
        } else if a.bbox.bottom() >= page_height * 0.85 {
            2
        } else {
            0
        };
        let col_b = if is_two_column
            && b.bbox.y > page_height * 0.15
            && b.bbox.bottom() < page_height * 0.85
        {
            if b.bbox.x >= mid_x { 1 } else { 0 }
        } else if b.bbox.bottom() >= page_height * 0.85 {
            2
        } else {
            0
        };

        col_a
            .cmp(&col_b)
            .then_with(|| {
                let y_tol = (a.bbox.height.min(b.bbox.height) * 0.45).max(3.5);
                if (a.bbox.y - b.bbox.y).abs() <= y_tol {
                    a.bbox.x.partial_cmp(&b.bbox.x).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.bbox.y.partial_cmp(&b.bbox.y).unwrap_or(std::cmp::Ordering::Equal)
                }
            })
    });

    // Cluster into lines
    let mut lines: Vec<ExtractedLine> = Vec::new();
    for w in sorted_words {
        if let Some(last) = lines.last_mut() {
            let y_tol = (last.bbox.height.min(w.bbox.height) * 0.5).max(4.0);
            let same_col = !is_two_column
                || ((last.bbox.x < mid_x) == (w.bbox.x < mid_x))
                || w.in_header_zone
                || w.in_footer_zone;
            if (last.bbox.y - w.bbox.y).abs() <= y_tol && same_col {
                last.bbox = last.bbox.union(&w.bbox);
                last.text.push(' ');
                last.text.push_str(&w.text);
                last.words.push(w);
                continue;
            }
        }
        lines.push(ExtractedLine {
            text: w.text.clone(),
            bbox: w.bbox,
            font_name: w.font_name.clone(),
            font_size: w.font_size,
            color_rgb: w.color_rgb,
            in_header_zone: w.in_header_zone,
            in_footer_zone: w.in_footer_zone,
            words: vec![w],
        });
    }

    // Sort words left-to-right inside each line and rebuild line text
    for line in &mut lines {
        line.words.sort_by(|a, b| {
            a.bbox.x.partial_cmp(&b.bbox.x).unwrap_or(std::cmp::Ordering::Equal)
        });
        line.text = line
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
    }

    lines
}

fn extract_page_annotations(
    doc: &Document,
    page_id: ObjectId,
    effective_box: [f64; 4],
    height_pt: f64,
) -> Vec<ExtractedAnnotation> {
    let mut out = Vec::new();
    let Ok(page_dict) = doc.get_dictionary(page_id) else {
        return out;
    };
    let Ok(annots_obj) = page_dict.get(b"Annots") else {
        return out;
    };
    let Ok((_, annots_deref)) = doc.dereference(annots_obj) else {
        return out;
    };
    let Ok(arr) = annots_deref.as_array() else {
        return out;
    };

    let origin_x = effective_box[0];
    let origin_y = effective_box[1];

    for item in arr {
        let Ok((_, Object::Dictionary(adict))) = doc.dereference(item) else {
            continue;
        };
        let subtype = adict
            .get(b"Subtype")
            .and_then(|o| o.as_name())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_else(|_| "Annot".to_string());

        // Skip Widget annotations here because they are handled in AcroForm fields
        if subtype == "Widget" || subtype == "Popup" {
            continue;
        }

        let bbox = extract_rect_top_left(adict, origin_x, origin_y, height_pt)
            .unwrap_or_else(|| BoundingBox::new(0.0, 0.0, 20.0, 20.0));

        let contents = adict
            .get(b"Contents")
            .ok()
            .and_then(|o| match o {
                Object::String(b, _) => Some(decode_pdf_string(b)),
                _ => None,
            })
            .unwrap_or_default();

        let author = adict
            .get(b"T")
            .ok()
            .and_then(|o| match o {
                Object::String(b, _) => Some(decode_pdf_string(b)),
                _ => None,
            })
            .unwrap_or_default();

        let color_rgb = adict.get(b"C").ok().and_then(|o| {
            let arr = o.as_array().ok()?;
            if arr.len() >= 3 {
                let r = (obj_to_f64(&arr[0])?.clamp(0.0, 1.0) * 255.0).round() as u8;
                let g = (obj_to_f64(&arr[1])?.clamp(0.0, 1.0) * 255.0).round() as u8;
                let b = (obj_to_f64(&arr[2])?.clamp(0.0, 1.0) * 255.0).round() as u8;
                Some([r, g, b])
            } else {
                None
            }
        });

        out.push(ExtractedAnnotation {
            subtype,
            bbox,
            contents,
            author,
            color_rgb,
        });
    }

    out
}

fn extract_rect_top_left(
    dict: &Dictionary,
    origin_x: f64,
    origin_y: f64,
    height_pt: f64,
) -> Option<BoundingBox> {
    let Object::Array(r) = dict.get(b"Rect").ok()? else {
        return None;
    };
    if r.len() < 4 {
        return None;
    }
    let x1 = obj_to_f64(&r[0])? - origin_x;
    let y1 = obj_to_f64(&r[1])? - origin_y;
    let x2 = obj_to_f64(&r[2])? - origin_x;
    let y2 = obj_to_f64(&r[3])? - origin_y;
    let min_x = x1.min(x2).max(0.0);
    let max_x = x1.max(x2);
    let min_y_pdf = y1.min(y2);
    let max_y_pdf = y1.max(y2);
    let y_top = (height_pt - max_y_pdf).max(0.0);
    Some(BoundingBox::new(
        min_x,
        y_top,
        (max_x - min_x).max(2.0),
        (max_y_pdf - min_y_pdf).max(2.0),
    ))
}

fn extract_document_form_fields(
    doc: &Document,
    pages_map: &std::collections::BTreeMap<u32, ObjectId>,
) -> Vec<ExtractedFormField> {
    let mut out = Vec::new();
    let page_id_to_num: HashMap<ObjectId, u32> =
        pages_map.iter().map(|(&num, &id)| (id, num)).collect();

    // Map widget object IDs to page numbers by scanning page /Annots
    let mut annot_id_to_page: HashMap<ObjectId, u32> = HashMap::new();
    for (&page_num, &page_id) in pages_map {
        if let Ok(pdict) = doc.get_dictionary(page_id) {
            if let Ok(annots_obj) = pdict.get(b"Annots") {
                if let Ok((_, Object::Array(arr))) = doc.dereference(annots_obj) {
                    for item in arr {
                        if let Ok(id) = item.as_reference() {
                            annot_id_to_page.insert(id, page_num);
                        }
                    }
                }
            }
        }
    }

    let field_ids = crate::commands::forms::field_ids(doc);
    for fid in field_ids {
        let Ok(dict) = doc.get_dictionary(fid) else {
            continue;
        };

        let name = match dict.get(b"T") {
            Ok(Object::String(bytes, _)) => decode_pdf_string(bytes),
            _ => continue,
        };
        if name.is_empty() {
            continue;
        }

        let ft_bytes = dict
            .get(b"FT")
            .ok()
            .and_then(|o| o.as_name().ok())
            .unwrap_or(b"Tx");
        let field_type = match ft_bytes {
            b"Btn" => "checkbox",
            b"Ch" => "choice",
            b"Sig" => "signature",
            _ => "text",
        }
        .to_string();

        let value = match dict.get(b"V") {
            Ok(Object::String(bytes, _)) => decode_pdf_string(bytes),
            Ok(Object::Name(bytes)) => String::from_utf8_lossy(bytes).into_owned(),
            _ => String::new(),
        };

        let page = dict
            .get(b"P")
            .ok()
            .and_then(|o| o.as_reference().ok())
            .and_then(|pid| page_id_to_num.get(&pid).copied())
            .or_else(|| annot_id_to_page.get(&fid).copied())
            .unwrap_or(1);

        let page_id = pages_map.get(&page).copied();
        let (origin_x, origin_y, height_pt) = if let Some(pid) = page_id {
            let mb = media_box(doc, pid);
            (mb[0], mb[1], (mb[3] - mb[1]).abs().max(1.0))
        } else {
            (0.0, 0.0, 792.0)
        };

        let bbox = extract_rect_top_left(dict, origin_x, origin_y, height_pt);
        let flags = dict.get(b"Ff").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
        let read_only = (flags & 1) != 0;

        out.push(ExtractedFormField {
            name,
            field_type,
            value,
            bbox,
            page,
            read_only,
        });
    }

    out
}

/// Refines header and footer zone detection so we don't blindly label every top/bottom line
/// as header/footer unless it either repeats across pages or sits inside the strict margin band.
fn refine_header_footer_zones(pages: &mut [ExtractedPage]) {
    if pages.len() <= 1 {
        // On a 1-page document, only keep strict margin band if the page has multiple lines (header/footer + body)
        if let Some(p) = pages.first_mut() {
            let h = p.height_pt;
            let has_multiple_lines = p.lines.len() >= 2;
            for line in &mut p.lines {
                line.in_header_zone = has_multiple_lines && line.bbox.y <= h * 0.08;
                line.in_footer_zone = has_multiple_lines && line.bbox.bottom() >= h * 0.92;
            }
        }
        return;
    }

    let mut top_candidates: HashMap<String, usize> = HashMap::new();
    let mut bottom_candidates: HashMap<String, usize> = HashMap::new();

    for p in pages.iter() {
        for line in &p.lines {
            let norm = normalize_header_footer_pattern(&line.text);
            if norm.is_empty() {
                continue;
            }
            if line.in_header_zone {
                *top_candidates.entry(norm.clone()).or_insert(0) += 1;
            }
            if line.in_footer_zone {
                *bottom_candidates.entry(norm).or_insert(0) += 1;
            }
        }
    }

    for p in pages.iter_mut() {
        let h = p.height_pt;
        for line in &mut p.lines {
            let norm = normalize_header_footer_pattern(&line.text);
            let repeats_top = top_candidates.get(&norm).copied().unwrap_or(0) >= 2;
            let repeats_bottom = bottom_candidates.get(&norm).copied().unwrap_or(0) >= 2;
            line.in_header_zone = line.in_header_zone && (repeats_top || line.bbox.y <= h * 0.07);
            line.in_footer_zone =
                line.in_footer_zone && (repeats_bottom || line.bbox.bottom() >= h * 0.93);
        }
    }
}

fn normalize_header_footer_pattern(text: &str) -> String {
    // Replace digits with '#' so "Sayfa 1 / 5" and "Sayfa 2 / 5" match as a repeating footer pattern
    text.chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c.to_lowercase().next().unwrap_or(c) })
        .collect::<String>()
        .trim()
        .to_string()
}
