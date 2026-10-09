use crate::error::PdfError;
use crate::pdfutil::{ensure_helvetica, escape_pdf_text, media_box};
use crate::types::OpResult;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::Deserialize;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct PageOverlay {
    pub page: u32,
    pub png_data: Vec<u8>,
    #[serde(default)]
    pub replace_page_content: bool,
    pub bbox: Option<[f64; 4]>,
}


#[derive(Debug, serde::Serialize)]
pub struct EditorLoadResult {
    pub annotations_json: Option<String>,
    pub clean_pdf_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub struct WatermarkZones {
    #[serde(default = "default_zone_val")]
    pub top: bool,
    #[serde(default = "default_zone_val")]
    pub center: bool,
    #[serde(default = "default_zone_val")]
    pub bottom: bool,
}

fn default_zone_val() -> bool {
    true
}

impl Default for WatermarkZones {
    fn default() -> Self {
        Self {
            top: true,
            center: true,
            bottom: true,
        }
    }
}

impl WatermarkZones {
    pub fn all(&self) -> bool {
        self.top && self.center && self.bottom
    }

    pub fn none(&self) -> bool {
        !self.top && !self.center && !self.bottom
    }
}

/// Applies transparent PNG overlays (drawings, text, annotations, signatures)
/// to the respective pages in the PDF document, and embeds editable editor metadata.
#[tauri::command]
pub async fn stamp_page_overlays(
    input: String,
    output: String,
    overlays: Vec<PageOverlay>,
    editor_data: Option<String>,
    input_bytes: Option<Vec<u8>>,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_overlays(
            Path::new(&input),
            input_bytes.as_deref(),
            Path::new(&output),
            overlays,
            editor_data,
        )
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Reads embedded editable annotation metadata and returns clean background PDF bytes.
#[tauri::command]
pub async fn load_editor_document(path: String) -> Result<EditorLoadResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        read_editor_document(Path::new(&path))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Adds a diagonal text watermark to every page.
///
/// Rendered with the base-14 Helvetica font (no font embedding needed) and
/// a graphics-state opacity, centered on each page's MediaBox at 45°.
#[tauri::command]
pub async fn add_watermark(
    input: String,
    output: String,
    text: String,
    font_size: f64,
    opacity: f64,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        annotate(Path::new(&input), Path::new(&output), |doc, page_id, _index, _total| {
            watermark_stream(doc, page_id, &text, font_size, opacity)
        })
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Adds "1 / N" style page numbers at the bottom center of every page.
#[tauri::command]
pub async fn add_page_numbers(
    input: String,
    output: String,
    font_size: f64,
    margin: f64,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_page_numbers(Path::new(&input), Path::new(&output), font_size, margin)
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Removes watermarks and stamps from every page of the document.
/// Detects dedicated watermark content streams, annotations, marked artifacts,
/// or specific target text across regional zones.
#[tauri::command]
pub async fn remove_watermark(
    input: String,
    output: String,
    target_text: Option<String>,
    zones: Option<WatermarkZones>,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        remove_watermark_doc(Path::new(&input), Path::new(&output), target_text.as_deref(), zones.as_ref())
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Removes watermarks and stamps directly in-memory, returning the cleaned PDF bytes.
#[tauri::command]
pub async fn remove_watermark_bytes(
    input: Option<String>,
    pdf_bytes: Option<Vec<u8>>,
    target_text: Option<String>,
    zones: Option<WatermarkZones>,
) -> Result<Vec<u8>, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut doc = if let Some(bytes) = pdf_bytes {
            Document::load_mem(&bytes)?
        } else if let Some(path_str) = input {
            Document::load(Path::new(&path_str))?
        } else {
            return Err(PdfError::Custom("Girdi PDF dosyası veya verisi belirtilmedi".into()));
        };

        if doc.is_encrypted() {
            return Err(PdfError::Encrypted);
        }

        clean_doc_watermarks(&mut doc, target_text.as_deref(), zones.as_ref())?;

        let mut out = Vec::new();
        doc.save_to(&mut out)?;
        Ok(out)
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Directly writes PDF bytes to disk safely.
#[tauri::command]
pub async fn write_pdf_bytes(path: String, data: Vec<u8>) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        if ext != "pdf" {
            return Err(PdfError::Custom(
                "Güvenlik kısıtlaması: Yalnızca .pdf uzantılı dosyalara yazma izni vardır.".to_string(),
            ));
        }

        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(p, &data)?;
        let total_size_bytes = data.len() as u64;
        let total_pages = match Document::load_mem(&data) {
            Ok(d) => d.get_pages().len() as u32,
            Err(_) => 0,
        };
        Ok(OpResult {
            outputs: vec![path],
            total_pages,
            total_size_bytes,
        })
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Cleans watermark content, Form XObjects, and metadata from an open Document.
pub fn clean_doc_watermarks(
    doc: &mut Document,
    target_text: Option<&str>,
    zones: Option<&WatermarkZones>,
) -> Result<u32, PdfError> {
    let default_zones = WatermarkZones::default();
    let active_zones = zones.unwrap_or(&default_zones);

    let page_ids: Vec<lopdf::ObjectId> = doc.page_iter().collect();
    if page_ids.is_empty() {
        return Err(PdfError::EmptyOutput);
    }
    let page_count = page_ids.len() as u32;

    let clean_target = target_text
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    // Clean Document Catalog /PieceInfo if present (only when center/global watermarks enabled)
    if active_zones.center {
        if let Ok(catalog_id) = doc.trailer.get(b"Root").and_then(|r| r.as_reference()) {
            if let Ok(catalog_dict) = doc.get_object_mut(catalog_id).and_then(|o| o.as_dict_mut()) {
                if catalog_dict.get(b"PieceInfo").is_ok() {
                    catalog_dict.remove(b"PieceInfo");
                }
            }
        }
    }

    for page_id in &page_ids {
        clean_page_watermarks(doc, *page_id, clean_target, active_zones)?;
    }

    doc.prune_objects();
    doc.compress();
    Ok(page_count)
}

pub fn remove_watermark_doc(
    input: &Path,
    output: &Path,
    target_text: Option<&str>,
    zones: Option<&WatermarkZones>,
) -> Result<OpResult, PdfError> {
    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let page_count = clean_doc_watermarks(&mut doc, target_text, zones)?;

    // Check if input and output point to the same file (safe atomic write)
    let is_same = input == output
        || std::fs::canonicalize(input).ok() == std::fs::canonicalize(output).ok();

    if is_same {
        let temp_output = output.with_file_name(format!(
            ".tmp_wm_{}.pdf",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
        ));
        doc.save(&temp_output)?;
        drop(doc);
        if std::fs::rename(&temp_output, output).is_err() {
            std::fs::copy(&temp_output, output)?;
            let _ = std::fs::remove_file(&temp_output);
        }
    } else {
        doc.save(output)?;
        drop(doc);
    }

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: page_count,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkZone {
    Top,
    Center,
    Bottom,
}

fn get_num(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(n) => Some(*n as f64),
        Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

fn is_diagonal_matrix(operands: &[Object]) -> bool {
    if operands.len() < 4 {
        return false;
    }
    let a = match get_num(&operands[0]) { Some(v) => v, None => return false };
    let b = match get_num(&operands[1]) { Some(v) => v, None => return false };
    let c = match get_num(&operands[2]) { Some(v) => v, None => return false };
    let d = match get_num(&operands[3]) { Some(v) => v, None => return false };

    let abs_a = a.abs();
    let abs_b = b.abs();
    let abs_c = c.abs();
    let abs_d = d.abs();

    // Diagonal matrix: b and c non-zero (> 0.05) and either a or d non-zero (> 0.05)
    abs_b > 0.05 && abs_c > 0.05 && (abs_a > 0.05 || abs_d > 0.05)
}

fn is_top_watermark_text(s: &str) -> bool {
    let lower = s.to_lowercase();
    const TOP_KEYWORDS: &[&str] = &[
        "aosdestek",
        "anadolu.edu.tr",
        "sisteminde",
        "bilgilendirme",
        "duyuru",
        "öğrenciye özel",
        "ogrenciye ozel",
        "http://",
        "https://",
        "www.",
        ".edu.tr",
        "kullanım kılavuzu",
        "kullanim kilavuzu",
    ];
    for kw in TOP_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }
    false
}

fn is_bottom_watermark_text(s: &str) -> bool {
    let lower = s.to_lowercase();
    const BOTTOM_KEYWORDS: &[&str] = &[
        "her hakkı saklıdır",
        "her hakki saklidir",
        "yazılı izn",
        "yazili izn",
        "kopya edil",
        "çoğaltıl",
        "cogaltil",
        "cezai sorumluluk",
        "cezai sorumluluğu",
        "cezai sorumlulugu",
        "hukuki ve cezai",
        "telif hakk",
        "izinsiz",
        "yayımlanamaz",
        "yayimlanamaz",
        "kopyalanamaz",
        "başka bir amaçla kullanılamaz",
        "baska bir amacla kullanilamaz",
    ];
    for kw in BOTTOM_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }
    false
}

fn contains_general_watermark_keyword(s: &str) -> bool {
    let lower = s.to_lowercase();
    const STRONG_KEYWORDS: &[&str] = &[
        "watermark",
        "confidential",
        "kopyalanamaz",
        "filigran",
        "do not copy",
        "evaluation",
    ];
    for kw in STRONG_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }
    let trimmed = lower.trim();
    trimmed == "gizli"
        || trimmed == "taslak"
        || trimmed == "örnek"
        || trimmed == "ornek"
        || trimmed == "deneme"
        || trimmed == "draft"
        || trimmed == "sample"
        || trimmed == "specimen"
        || trimmed == "preview"
}

fn contains_target_text(s: &str, target: &str) -> bool {
    let target_clean = target.trim();
    if target_clean.is_empty() {
        return false;
    }
    let s_lower = s.to_lowercase();
    let target_lower = target_clean.to_lowercase();
    if s_lower.contains(&target_lower) {
        return true;
    }
    let s_norm = s_lower.replace('ı', "i");
    let t_norm = target_lower.replace('ı', "i");
    s_norm.contains(&t_norm)
}

fn extract_text_from_op(op: &Operation) -> String {
    let mut text = String::new();
    match op.operator.as_str() {
        "Tj" | "'" => {
            if let Some(Object::String(bytes, _)) = op.operands.first() {
                text.push_str(&String::from_utf8_lossy(bytes));
            }
        }
        "\"" => {
            if let Some(Object::String(bytes, _)) = op.operands.get(2) {
                text.push_str(&String::from_utf8_lossy(bytes));
            }
        }
        "TJ" => {
            if let Some(Object::Array(items)) = op.operands.first() {
                for item in items {
                    if let Object::String(bytes, _) = item {
                        text.push_str(&String::from_utf8_lossy(bytes));
                    }
                }
            }
        }
        _ => {}
    }
    text
}

fn extract_block_y_coord(ops: &[Operation]) -> Option<f64> {
    let mut cm_y: Option<f64> = None;
    let mut tm_y: Option<f64> = None;
    let mut td_y: Option<f64> = None;

    for op in ops {
        match op.operator.as_str() {
            "cm" => {
                if op.operands.len() >= 6 {
                    if let Some(y) = get_num(&op.operands[5]) {
                        cm_y = Some(y);
                    }
                }
            }
            "Tm" => {
                if op.operands.len() >= 6 {
                    if let Some(y) = get_num(&op.operands[5]) {
                        tm_y = Some(y);
                    }
                }
            }
            "Td" | "TD" => {
                if op.operands.len() >= 2 {
                    if let Some(y) = get_num(&op.operands[1]) {
                        td_y = Some(y);
                    }
                }
            }
            _ => {}
        }
    }

    match (cm_y, tm_y, td_y) {
        (Some(cy), Some(ty), _) => Some(cy + ty),
        (Some(cy), None, Some(dy)) => Some(cy + dy),
        (Some(cy), None, None) => Some(cy),
        (None, Some(ty), _) => Some(ty),
        (None, None, Some(dy)) => Some(dy),
        (None, None, None) => None,
    }
}

fn determine_block_zone(
    ops: &[Operation],
    y0: f64,
    height: f64,
    extracted_text: &str,
) -> WatermarkZone {
    // 1. Diagonal matrices are always Center
    for op in ops {
        if matches!(op.operator.as_str(), "cm" | "Tm") && is_diagonal_matrix(&op.operands) {
            return WatermarkZone::Center;
        }
    }

    // 2. Check position from coordinates if height > 0
    if height > 0.0 {
        if let Some(y) = extract_block_y_coord(ops) {
            let rel_y = (y - y0) / height;
            if rel_y >= 0.82 {
                return WatermarkZone::Top;
            } else if rel_y <= 0.16 {
                return WatermarkZone::Bottom;
            } else {
                return WatermarkZone::Center;
            }
        }
    }

    // 3. Fallback to keyword heuristics if no coordinate was extractable
    if is_top_watermark_text(extracted_text) {
        WatermarkZone::Top
    } else if is_bottom_watermark_text(extracted_text) {
        WatermarkZone::Bottom
    } else {
        WatermarkZone::Center
    }
}

fn should_remove_watermark_block(
    ops: &[Operation],
    deleted_xobjects: &[Vec<u8>],
    target: Option<&str>,
    y0: f64,
    height: f64,
    zones: &WatermarkZones,
) -> bool {
    let mut has_diagonal = false;
    let mut has_general_wm_kw = false;
    let mut has_top_wm_kw = false;
    let mut has_bottom_wm_kw = false;
    let mut has_target_match = false;
    let mut has_deleted_xobj = false;
    let mut has_artifact_tag = false;
    let mut has_transparency_gs = false;
    let mut text_op_count = 0;
    let mut all_extracted_text = String::new();

    for op in ops {
        match op.operator.as_str() {
            "cm" | "Tm" => {
                if is_diagonal_matrix(&op.operands) {
                    has_diagonal = true;
                }
            }
            "Tj" | "TJ" | "'" | "\"" => {
                text_op_count += 1;
                let txt = extract_text_from_op(op);
                if !txt.is_empty() {
                    if let Some(t) = target {
                        if contains_target_text(&txt, t) {
                            has_target_match = true;
                        }
                    }
                    if contains_general_watermark_keyword(&txt) {
                        has_general_wm_kw = true;
                    }
                    if is_top_watermark_text(&txt) {
                        has_top_wm_kw = true;
                    }
                    if is_bottom_watermark_text(&txt) {
                        has_bottom_wm_kw = true;
                    }
                    all_extracted_text.push_str(&txt);
                    all_extracted_text.push(' ');
                }
            }
            "Do" => {
                if let Some(Object::Name(name)) = op.operands.first() {
                    if deleted_xobjects.contains(name) {
                        has_deleted_xobj = true;
                    }
                    let name_str = String::from_utf8_lossy(name).to_lowercase();
                    if name_str.contains("watermark") || name_str.contains("filigran") || name_str.contains("stamp") {
                        has_deleted_xobj = true;
                    }
                }
            }
            "gs" => {
                has_transparency_gs = true;
            }
            "BDC" | "BMC" => {
                if let Some(Object::Name(tag)) = op.operands.first() {
                    let tag_str = String::from_utf8_lossy(tag).to_lowercase();
                    if tag_str.contains("artifact") || tag_str.contains("watermark") {
                        has_artifact_tag = true;
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(t) = target {
        if contains_target_text(&all_extracted_text, t) {
            has_target_match = true;
        }
    }
    if contains_general_watermark_keyword(&all_extracted_text) {
        has_general_wm_kw = true;
    }
    if is_top_watermark_text(&all_extracted_text) {
        has_top_wm_kw = true;
    }
    if is_bottom_watermark_text(&all_extracted_text) {
        has_bottom_wm_kw = true;
    }

    let is_wm = has_target_match
        || has_deleted_xobj
        || has_artifact_tag
        || (has_diagonal && text_op_count > 0)
        || (has_general_wm_kw && text_op_count > 0)
        || (has_top_wm_kw && text_op_count > 0)
        || (has_bottom_wm_kw && text_op_count > 0)
        || (has_transparency_gs && has_diagonal && text_op_count > 0 && text_op_count <= 4);

    if !is_wm {
        return false;
    }

    let zone = determine_block_zone(ops, y0, height, &all_extracted_text);
    match zone {
        WatermarkZone::Top => zones.top,
        WatermarkZone::Center => zones.center,
        WatermarkZone::Bottom => zones.bottom,
    }
}

#[derive(Debug, Clone)]
struct TextChunk {
    ops: Vec<Operation>,
    is_diagonal: bool,
    y: Option<f64>,
    text: String,
    has_text: bool,
}

fn determine_chunk_zone(
    is_diagonal: bool,
    y: Option<f64>,
    current_cm_y: f64,
    y0: f64,
    height: f64,
    text: &str,
) -> WatermarkZone {
    if is_diagonal {
        return WatermarkZone::Center;
    }
    if is_top_watermark_text(text) {
        return WatermarkZone::Top;
    }
    if is_bottom_watermark_text(text) {
        return WatermarkZone::Bottom;
    }
    if height > 0.0 {
        if let Some(y_val) = y {
            let eff_y = y_val + current_cm_y;
            let rel_y = (eff_y - y0) / height;
            if rel_y >= 0.80 {
                return WatermarkZone::Top;
            } else if rel_y <= 0.18 {
                return WatermarkZone::Bottom;
            } else {
                return WatermarkZone::Center;
            }
        }
    }
    WatermarkZone::Center
}

fn is_chunk_watermark(
    is_wm_context: bool,
    is_diagonal: bool,
    text: &str,
    target: Option<&str>,
) -> bool {
    if is_wm_context {
        return true;
    }
    if is_diagonal {
        return true;
    }
    if let Some(t) = target {
        if contains_target_text(text, t) {
            return true;
        }
    }
    if contains_general_watermark_keyword(text)
        || is_top_watermark_text(text)
        || is_bottom_watermark_text(text)
    {
        return true;
    }
    false
}

fn is_page_number_pattern(s: &str) -> bool {
    let lower = s.to_lowercase();
    let trimmed = lower.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Pure digits: "1", "2", "42"
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    // Enclosed digits: "- 1 -", "— 1 —", "[1]", "(1)", "/ 1 /", "• 1 •"
    let stripped = trimmed
        .trim_matches(|c: char| {
            c == '-' || c == '—' || c == '–' || c == '[' || c == ']' || c == '(' || c == ')' || c == '/' || c == '•' || c == ' '
        })
        .trim();
    if !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    // "1 / 8", "1/8", "1 | 8", "1 of 8"
    if let Some((left, right)) = trimmed.split_once('/') {
        if left.trim().chars().all(|c| c.is_ascii_digit()) && right.trim().chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    if let Some((left, right)) = trimmed.split_once('|') {
        if left.trim().chars().all(|c| c.is_ascii_digit()) && right.trim().chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    if let Some((left, right)) = trimmed.split_once(" of ") {
        if left.trim().chars().all(|c| c.is_ascii_digit()) && right.trim().chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }

    // "Sayfa 1", "Sayfa 1 / 8", "Page 1", "Page 1 of 8"
    if trimmed.starts_with("sayfa") || trimmed.starts_with("page") || trimmed.starts_with("seite") {
        let after_prefix = trimmed
            .trim_start_matches("sayfa")
            .trim_start_matches("page")
            .trim_start_matches("seite")
            .trim_matches(|c: char| c == ':' || c == '.' || c == ' ')
            .trim();
        if !after_prefix.is_empty() {
            if after_prefix.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
            if let Some((left, right)) = after_prefix.split_once('/') {
                if left.trim().chars().all(|c| c.is_ascii_digit()) && right.trim().chars().all(|c| c.is_ascii_digit()) {
                    return true;
                }
            }
        }
    }

    false
}

fn is_page_number_chunk(
    chunk: &TextChunk,
    current_cm_y: f64,
    y0: f64,
    height: f64,
) -> bool {
    // 1. If chunk uses /PK_Font and is in footer area, it is definitely a PDFKit page number
    let uses_pk_font = chunk.ops.iter().any(|op| {
        op.operator == "Tf" && op.operands.iter().any(|arg| match arg {
            Object::Name(n) => n == b"PK_Font",
            _ => false,
        })
    });

    let eff_y = match chunk.y {
        Some(y) => y + current_cm_y,
        None => return uses_pk_font,
    };
    let rel_y = if height > 0.0 { (eff_y - y0) / height } else { 0.5 };

    if uses_pk_font && (rel_y <= 0.15 || (eff_y - y0) <= 90.0) {
        return true;
    }

    // Must be in bottom footer region (bottom 10% or within 65 pt of page bottom)
    if rel_y > 0.10 && (eff_y - y0) > 65.0 {
        return false;
    }

    // 2. Extract total bytes and text
    let mut total_bytes = 0;
    let mut op_text = String::new();
    for op in &chunk.ops {
        match op.operator.as_str() {
            "Tj" | "'" | "\"" => {
                if let Some(Object::String(bytes, _)) = op.operands.first() {
                    total_bytes += bytes.len();
                }
                let t = extract_text_from_op(op);
                op_text.push_str(&t);
            }
            "TJ" => {
                if let Some(Object::Array(arr)) = op.operands.first() {
                    for item in arr {
                        if let Object::String(bytes, _) = item {
                            total_bytes += bytes.len();
                        }
                    }
                }
                let t = extract_text_from_op(op);
                op_text.push_str(&t);
            }
            _ => {}
        }
    }

    let trimmed = op_text.trim();

    // Standard pattern matching for page numbers (e.g. "1", "2", "1 / 8", "- 1 -", "Sayfa 1")
    if !trimmed.is_empty() && is_page_number_pattern(trimmed) {
        return true;
    }

    // Subset font heuristic: in the footer (bottom 8% or <= 55 pt),
    // any isolated text chunk with 1 to 4 glyphs (<= 6 bytes) is a page number.
    // (Legal notices/questions are much longer, >= 30-300 bytes).
    if total_bytes > 0 && total_bytes <= 6 && (rel_y <= 0.08 || (eff_y - y0) <= 55.0) {
        return true;
    }

    false
}

fn clean_bt_block(
    bt_ops: &[Operation],
    is_wm_context: bool,
    target: Option<&str>,
    y0: f64,
    height: f64,
    zones: &WatermarkZones,
    current_cm_y: f64,
    clean_page_numbers: bool,
) -> (Option<Vec<Operation>>, bool) {
    if bt_ops.len() < 2
        || bt_ops.first().map(|o| o.operator.as_str()) != Some("BT")
        || bt_ops.last().map(|o| o.operator.as_str()) != Some("ET")
    {
        return (Some(bt_ops.to_vec()), false);
    }

    let mut initial_setup_ops: Vec<Operation> = Vec::new();
    let mut chunks: Vec<TextChunk> = Vec::new();

    let mut cur_ops: Vec<Operation> = Vec::new();
    let mut cur_is_diag = false;
    let mut cur_y: Option<f64> = None;
    let mut cur_text = String::new();
    let mut cur_has_text = false;

    let inner_ops = &bt_ops[1..bt_ops.len() - 1];

    for op in inner_ops {
        match op.operator.as_str() {
            "Tm" => {
                let is_diag = is_diagonal_matrix(&op.operands);
                let y_val = if op.operands.len() >= 6 {
                    get_num(&op.operands[5])
                } else {
                    None
                };

                if cur_has_text {
                    chunks.push(TextChunk {
                        ops: cur_ops,
                        is_diagonal: cur_is_diag,
                        y: cur_y,
                        text: cur_text,
                        has_text: true,
                    });
                    cur_ops = vec![op.clone()];
                    cur_is_diag = is_diag;
                    cur_y = y_val;
                    cur_text = String::new();
                    cur_has_text = false;
                } else {
                    cur_ops.push(op.clone());
                    cur_is_diag = is_diag;
                    if y_val.is_some() {
                        cur_y = y_val;
                    }
                }
            }
            "Td" | "TD" => {
                let dy = op.operands.get(1).and_then(get_num).unwrap_or(0.0);
                if cur_has_text {
                    chunks.push(TextChunk {
                        ops: cur_ops,
                        is_diagonal: cur_is_diag,
                        y: cur_y,
                        text: cur_text,
                        has_text: true,
                    });
                    cur_ops = vec![op.clone()];
                    cur_is_diag = false;
                    cur_y = Some(cur_y.unwrap_or(0.0) + dy);
                    cur_text = String::new();
                    cur_has_text = false;
                } else {
                    cur_ops.push(op.clone());
                    cur_y = Some(cur_y.unwrap_or(0.0) + dy);
                }
            }
            "T*" => {
                if cur_has_text {
                    chunks.push(TextChunk {
                        ops: cur_ops,
                        is_diagonal: cur_is_diag,
                        y: cur_y,
                        text: cur_text,
                        has_text: true,
                    });
                    cur_ops = vec![op.clone()];
                    cur_is_diag = false;
                    cur_text = String::new();
                    cur_has_text = false;
                } else {
                    cur_ops.push(op.clone());
                }
            }
            "Tj" | "TJ" | "'" | "\"" => {
                let txt = extract_text_from_op(op);
                if !txt.is_empty() {
                    cur_text.push_str(&txt);
                    cur_text.push(' ');
                }
                cur_ops.push(op.clone());
                cur_has_text = true;
            }
            _ => {
                if cur_has_text || !cur_ops.is_empty() {
                    cur_ops.push(op.clone());
                } else {
                    initial_setup_ops.push(op.clone());
                }
            }
        }
    }

    if !cur_ops.is_empty() {
        chunks.push(TextChunk {
            ops: cur_ops,
            is_diagonal: cur_is_diag,
            y: cur_y,
            text: cur_text,
            has_text: cur_has_text,
        });
    }

    let mut kept_chunks: Vec<TextChunk> = Vec::new();
    let mut any_removed = false;

    for chunk in chunks {
        if !chunk.has_text {
            kept_chunks.push(chunk);
            continue;
        }

        // Clean existing page number if requested or if footer zone is targeted
        if (clean_page_numbers || zones.bottom) && is_page_number_chunk(&chunk, current_cm_y, y0, height) {
            any_removed = true;
            continue;
        }

        let is_wm = is_chunk_watermark(is_wm_context, chunk.is_diagonal, &chunk.text, target);
        if !is_wm {
            kept_chunks.push(chunk);
            continue;
        }

        let zone = determine_chunk_zone(chunk.is_diagonal, chunk.y, current_cm_y, y0, height, &chunk.text);
        let should_remove = match zone {
            WatermarkZone::Top => zones.top,
            WatermarkZone::Center => zones.center,
            WatermarkZone::Bottom => zones.bottom,
        };

        if should_remove {
            any_removed = true;
        } else {
            kept_chunks.push(chunk);
        }
    }

    let has_any_remaining_text = kept_chunks.iter().any(|c| c.has_text);
    if !has_any_remaining_text {
        (None, true)
    } else if any_removed {
        let mut new_ops = Vec::new();
        new_ops.push(Operation::new("BT", vec![]));
        new_ops.extend(initial_setup_ops);
        for chunk in kept_chunks {
            new_ops.extend(chunk.ops);
        }
        new_ops.push(Operation::new("ET", vec![]));
        (Some(new_ops), true)
    } else {
        (Some(bt_ops.to_vec()), false)
    }
}

fn is_block_watermark_context(
    ops: &[Operation],
    deleted_xobjects: &[Vec<u8>],
    target: Option<&str>,
) -> bool {
    let mut text_count = 0;
    let mut has_diagonal = false;
    let mut has_transparency = false;
    let mut has_wm_marker = false;
    let mut all_extracted_text = String::new();

    for op in ops {
        match op.operator.as_str() {
            "cm" | "Tm" => {
                if is_diagonal_matrix(&op.operands) {
                    has_diagonal = true;
                }
            }
            "gs" => {
                has_transparency = true;
            }
            "BDC" | "BMC" => {
                for operand in &op.operands {
                    if let Object::Name(tag) = operand {
                        let tag_str = String::from_utf8_lossy(tag).to_lowercase();
                        if tag_str.contains("artifact")
                            || tag_str.contains("watermark")
                            || tag_str.contains("filigran")
                            || tag_str.contains("xi")
                            || tag_str.contains("oc")
                        {
                            has_wm_marker = true;
                        }
                    }
                }
            }
            "Do" => {
                if let Some(Object::Name(name)) = op.operands.first() {
                    if deleted_xobjects.contains(name) {
                        return true;
                    }
                    let name_str = String::from_utf8_lossy(name).to_lowercase();
                    if name_str.contains("watermark")
                        || name_str.contains("filigran")
                        || name_str.contains("stamp")
                    {
                        return true;
                    }
                }
            }
            "Tj" | "TJ" | "'" | "\"" => {
                text_count += 1;
                let txt = extract_text_from_op(op);
                if !txt.is_empty() {
                    all_extracted_text.push_str(&txt);
                    all_extracted_text.push(' ');
                }
            }
            _ => {}
        }
    }

    if let Some(t) = target {
        if contains_target_text(&all_extracted_text, t) {
            return true;
        }
    }
    if contains_general_watermark_keyword(&all_extracted_text)
        || is_top_watermark_text(&all_extracted_text)
        || is_bottom_watermark_text(&all_extracted_text)
    {
        return true;
    }

    has_wm_marker || (has_diagonal && text_count > 0 && text_count <= 50) || (has_transparency && has_diagonal)
}

fn has_printable_content(ops: &[Operation]) -> bool {
    for op in ops {
        match op.operator.as_str() {
            "Tj" | "TJ" | "'" | "\"" => return true,
            "Do" => return true,
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "sh" => return true,
            _ => {}
        }
    }
    false
}

fn clean_operations(
    ops: Vec<Operation>,
    deleted_xobjects: &[Vec<u8>],
    target: Option<&str>,
    y0: f64,
    height: f64,
    zones: &WatermarkZones,
    clean_page_numbers: bool,
) -> (Vec<Operation>, bool) {
    let mut modified = false;
    let mut result = Vec::with_capacity(ops.len());
    let mut i = 0;

    while i < ops.len() {
        // 1. Check for 'q' block
        if ops[i].operator == "q" {
            let mut depth = 1;
            let mut j = i + 1;
            let mut found_q = false;
            while j < ops.len() {
                if ops[j].operator == "q" {
                    depth += 1;
                } else if ops[j].operator == "Q" {
                    depth -= 1;
                    if depth == 0 {
                        found_q = true;
                        break;
                    }
                }
                j += 1;
            }

            if found_q {
                let block_ops = &ops[i..=j];
                let is_wm_context = is_block_watermark_context(block_ops, deleted_xobjects, target);

                if is_wm_context || clean_page_numbers {
                    let text_count = block_ops
                        .iter()
                        .filter(|o| matches!(o.operator.as_str(), "Tj" | "TJ" | "'" | "\""))
                        .count();

                    // If all zones enabled, and text_count <= 50, drop the entire block (only when removing watermarks)
                    if is_wm_context && !clean_page_numbers && zones.all() && text_count <= 50 {
                        modified = true;
                        i = j + 1;
                        continue;
                    }

                    // Perform intra-block line/segment filtering
                    let mut new_block_ops = Vec::new();
                    let mut block_modified = false;
                    let mut current_cm_y = 0.0;
                    let mut k = 0;

                    while k < block_ops.len() {
                        if block_ops[k].operator == "cm" && block_ops[k].operands.len() >= 6 {
                            if let Some(cy) = get_num(&block_ops[k].operands[5]) {
                                current_cm_y += cy;
                            }
                            new_block_ops.push(block_ops[k].clone());
                            k += 1;
                        } else if block_ops[k].operator == "BT" {
                            let mut m = k + 1;
                            let mut found_et = false;
                            while m < block_ops.len() {
                                if block_ops[m].operator == "ET" {
                                    found_et = true;
                                    break;
                                }
                                m += 1;
                            }
                            if found_et {
                                let bt_slice = &block_ops[k..=m];
                                let (cleaned_bt, was_cleaned) = clean_bt_block(
                                    bt_slice,
                                    is_wm_context,
                                    target,
                                    y0,
                                    height,
                                    zones,
                                    current_cm_y,
                                    clean_page_numbers,
                                );
                                if was_cleaned {
                                    block_modified = true;
                                }
                                if let Some(c_bt) = cleaned_bt {
                                    new_block_ops.extend(c_bt);
                                }
                                k = m + 1;
                            } else {
                                new_block_ops.push(block_ops[k].clone());
                                k += 1;
                            }
                        } else if block_ops[k].operator == "Do" {
                            let mut drop_do = false;
                            if let Some(Object::Name(name)) = block_ops[k].operands.first() {
                                if deleted_xobjects.contains(name) && zones.center {
                                    drop_do = true;
                                    block_modified = true;
                                }
                            }
                            if !drop_do {
                                new_block_ops.push(block_ops[k].clone());
                            }
                            k += 1;
                        } else {
                            new_block_ops.push(block_ops[k].clone());
                            k += 1;
                        }
                    }

                    if block_modified {
                        modified = true;
                        // If no printable content remains in the watermark q...Q block, drop it completely
                        if has_printable_content(&new_block_ops) {
                            result.extend(new_block_ops);
                        }
                        i = j + 1;
                        continue;
                    } else {
                        result.extend_from_slice(block_ops);
                        i = j + 1;
                        continue;
                    }
                }
            }
        }

        // 2. Check for 'BT ... ET' block outside q...Q
        if ops[i].operator == "BT" {
            let mut j = i + 1;
            let mut found_et = false;
            while j < ops.len() {
                if ops[j].operator == "ET" {
                    found_et = true;
                    break;
                }
                j += 1;
            }

            if found_et {
                let bt_slice = &ops[i..=j];
                let is_wm_context = is_block_watermark_context(bt_slice, deleted_xobjects, target);
                let text_count = bt_slice
                    .iter()
                    .filter(|o| matches!(o.operator.as_str(), "Tj" | "TJ" | "'" | "\""))
                    .count();

                if is_wm_context && !clean_page_numbers && zones.all() && text_count <= 50 {
                    modified = true;
                    i = j + 1;
                    continue;
                }

                let (cleaned_bt, was_cleaned) = clean_bt_block(
                    bt_slice,
                    is_wm_context,
                    target,
                    y0,
                    height,
                    zones,
                    0.0,
                    clean_page_numbers,
                );
                if was_cleaned {
                    modified = true;
                    if let Some(c_bt) = cleaned_bt {
                        result.extend(c_bt);
                    }
                    i = j + 1;
                    continue;
                } else {
                    result.extend_from_slice(bt_slice);
                    i = j + 1;
                    continue;
                }
            }
        }

        // 3. Check for standalone 'Do' calling deleted XObject
        if ops[i].operator == "Do" {
            if let Some(Object::Name(name)) = ops[i].operands.first() {
                if deleted_xobjects.contains(name) {
                    if zones.center {
                        modified = true;
                        i += 1;
                        continue;
                    }
                }
            }
        }

        result.push(ops[i].clone());
        i += 1;
    }

    (result, modified)
}

fn find_and_remove_watermark_xobjects(
    doc: &mut Document,
    page_id: ObjectId,
    target_text: Option<&str>,
    zones: &WatermarkZones,
) -> Vec<Vec<u8>> {
    let mut deleted_names: Vec<Vec<u8>> = Vec::new();

    let res_obj = doc.get_object(page_id).and_then(|o| o.as_dict())
        .ok()
        .and_then(|d| d.get(b"Resources").ok().cloned())
        .or_else(|| crate::pdfutil::inherited_value(doc, page_id, b"Resources"));

    let res_obj = match res_obj {
        Some(o) => o,
        None => return deleted_names,
    };

    let res_dict_id = match res_obj {
        Object::Reference(id) => Some(id),
        _ => None,
    };

    let mut xobjects_to_check: Vec<(Vec<u8>, ObjectId)> = Vec::new();

    let res_dict = if let Some(id) = res_dict_id {
        doc.get_object(id).ok().and_then(|o| o.as_dict().ok().cloned())
    } else if let Object::Dictionary(ref dict) = res_obj {
        Some(dict.clone())
    } else {
        None
    };

    if let Some(r_dict) = res_dict {
        if let Ok(xobj_entry) = r_dict.get(b"XObject") {
            let xobj_dict = match xobj_entry {
                Object::Reference(xid) => doc.get_object(*xid).ok().and_then(|o| o.as_dict().ok().cloned()),
                Object::Dictionary(ref d) => Some(d.clone()),
                _ => None,
            };

            if let Some(xd) = xobj_dict {
                for (name, val) in xd.iter() {
                    if let Object::Reference(xid) = val {
                        xobjects_to_check.push((name.clone(), *xid));
                    }
                }
            }
        }
    }

    let mut xobjs_to_delete: Vec<(Vec<u8>, ObjectId)> = Vec::new();

    for (name, xid) in xobjects_to_check {
        let is_wm = if let Ok(Object::Stream(ref stream)) = doc.get_object(xid) {
            let mut matches_wm = false;

            if let Ok(subtype) = stream.dict.get(b"Subtype").and_then(|s| s.as_name()) {
                if subtype == b"Watermark" {
                    matches_wm = true;
                }
            }

            if !matches_wm && stream.dict.get(b"PieceInfo").is_ok() {
                if let Ok(piece) = stream.dict.get(b"PieceInfo") {
                    let piece_str = format!("{:?}", piece);
                    if piece_str.contains("Watermark") || piece_str.contains("ADBE") {
                        matches_wm = true;
                    }
                }
            }

            if !matches_wm {
                let name_str = String::from_utf8_lossy(&name).to_lowercase();
                if name_str.contains("watermark") || name_str.contains("filigran") || name_str.contains("stamp") || name_str.contains("wm") {
                    matches_wm = true;
                }
            }

            if !matches_wm {
                let decompressed = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                if let Ok(c) = Content::decode(&decompressed) {
                    if should_remove_watermark_block(&c.operations, &[], target_text, 0.0, 792.0, zones) {
                        matches_wm = true;
                    }
                }
            }

            matches_wm
        } else {
            false
        };

        if is_wm && zones.center {
            deleted_names.push(name.clone());
            xobjs_to_delete.push((name, xid));
        }
    }

    for (name, xid) in xobjs_to_delete {
        if let Some(r_id) = res_dict_id {
            if let Ok(res_mut) = doc.get_object_mut(r_id).and_then(|o| o.as_dict_mut()) {
                if let Ok(xobj_val) = res_mut.get_mut(b"XObject") {
                    match xobj_val {
                        Object::Dictionary(ref mut xd) => {
                            xd.remove(&name);
                        }
                        Object::Reference(xid_ref) => {
                            let xid_ref = *xid_ref;
                            if let Ok(xd_mut) = doc.get_object_mut(xid_ref).and_then(|o| o.as_dict_mut()) {
                                xd_mut.remove(&name);
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else if let Ok(page_dict_mut) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            if let Ok(res_val) = page_dict_mut.get_mut(b"Resources") {
                if let Object::Dictionary(ref mut res_dict) = res_val {
                    if let Ok(xobj_val) = res_dict.get_mut(b"XObject") {
                        match xobj_val {
                            Object::Dictionary(ref mut xd) => {
                                xd.remove(&name);
                            }
                            Object::Reference(xid_ref) => {
                                let xid_ref = *xid_ref;
                                if let Ok(xd_mut) = doc.get_object_mut(xid_ref).and_then(|o| o.as_dict_mut()) {
                                    xd_mut.remove(&name);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        doc.delete_object(xid);
    }

    deleted_names
}

fn clean_page_watermarks(
    doc: &mut Document,
    page_id: ObjectId,
    target_text: Option<&str>,
    zones: &WatermarkZones,
) -> Result<(), PdfError> {
    let mbox = media_box(doc, page_id);
    let y0 = mbox[1].min(mbox[3]);
    let height = (mbox[3] - mbox[1]).abs();
    let height = if height > 0.0 { height } else { 792.0 };

    // 0. Clean Page /PieceInfo & /ADBE_CompoundType (only when center/global watermarks enabled)
    if zones.center {
        if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            if page_dict.get(b"PieceInfo").is_ok() {
                page_dict.remove(b"PieceInfo");
            }
            if page_dict.get(b"ADBE_CompoundType").is_ok() {
                page_dict.remove(b"ADBE_CompoundType");
            }
        }
    }

    // 1. Clean Annotations (/Annots)
    let annot_ids: Vec<ObjectId> = if let Ok(page_dict) = doc.get_object(page_id).and_then(|o| o.as_dict()) {
        if let Ok(annots) = page_dict.get(b"Annots") {
            if let Object::Array(ref arr) = annots {
                arr.iter().filter_map(|item| match item {
                    Object::Reference(id) => Some(*id),
                    _ => None,
                }).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut del_annots: Vec<ObjectId> = Vec::new();
    for annot_id in annot_ids {
        if let Ok(annot_dict) = doc.get_object(annot_id).and_then(|o| o.as_dict()) {
            let mut remove_this = false;
            let mut annot_zone = WatermarkZone::Center;

            if let Ok(rect) = annot_dict.get(b"Rect").and_then(|r| r.as_array()) {
                if rect.len() >= 4 {
                    if let Some(ay) = get_num(&rect[1]) {
                        let rel_y = (ay - y0) / height;
                        if rel_y >= 0.82 {
                            annot_zone = WatermarkZone::Top;
                        } else if rel_y <= 0.16 {
                            annot_zone = WatermarkZone::Bottom;
                        }
                    }
                }
            }

            if let Ok(sub) = annot_dict.get(b"Subtype").and_then(|s| s.as_name()) {
                if sub == b"Watermark" {
                    remove_this = true;
                }
            }
            if !remove_this {
                if let Some(target) = target_text {
                    if let Ok(c) = annot_dict.get(b"Contents").and_then(|c| c.as_str()) {
                        if contains_target_text(&String::from_utf8_lossy(c), target) {
                            remove_this = true;
                        }
                    }
                } else if let Ok(c) = annot_dict.get(b"Contents").and_then(|c| c.as_str()) {
                    let c_str = String::from_utf8_lossy(c);
                    if contains_general_watermark_keyword(&c_str)
                        || is_top_watermark_text(&c_str)
                        || is_bottom_watermark_text(&c_str)
                    {
                        remove_this = true;
                    }
                }
            }

            if remove_this {
                let zone_enabled = match annot_zone {
                    WatermarkZone::Top => zones.top,
                    WatermarkZone::Center => zones.center,
                    WatermarkZone::Bottom => zones.bottom,
                };
                if zone_enabled {
                    del_annots.push(annot_id);
                }
            }
        }
    }

    if !del_annots.is_empty() {
        if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            if let Ok(annots) = page_dict.get_mut(b"Annots") {
                if let Object::Array(ref mut annot_vec) = annots {
                    annot_vec.retain(|item| match item {
                        Object::Reference(id) => !del_annots.contains(id),
                        _ => true,
                    });
                }
            }
        }
        for del_id in del_annots {
            doc.delete_object(del_id);
        }
    }

    // 2. Identify and remove Form XObject watermarks
    let deleted_xobjects = find_and_remove_watermark_xobjects(doc, page_id, target_text, zones);

    // 3. Identify /Contents streams
    let mut stream_ids: Vec<ObjectId> = Vec::new();
    let mut is_array_contents = false;

    if let Ok(page_dict) = doc.get_object(page_id).and_then(|o| o.as_dict()) {
        if let Ok(contents) = page_dict.get(b"Contents") {
            match contents {
                Object::Reference(id) => {
                    stream_ids.push(*id);
                }
                Object::Array(arr) => {
                    is_array_contents = true;
                    for item in arr {
                        if let Object::Reference(id) = item {
                            stream_ids.push(*id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut deleted_streams: Vec<ObjectId> = Vec::new();

    for stream_id in &stream_ids {
        if let Ok(stream) = doc.get_object(*stream_id).and_then(|o| o.as_stream()) {
            let decompressed = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());

            if let Ok(content) = Content::decode(&decompressed) {
                let (new_ops, modified) = clean_operations(content.operations, &deleted_xobjects, target_text, y0, height, zones, zones.bottom);
                if modified {
                    if new_ops.is_empty() && is_array_contents && stream_ids.len() > 1 {
                        deleted_streams.push(*stream_id);
                    } else if let Ok(stream_mut) = doc.get_object_mut(*stream_id).and_then(|o| o.as_stream_mut()) {
                        let new_content = Content { operations: new_ops };
                        if let Ok(encoded) = new_content.encode() {
                            stream_mut.set_plain_content(encoded);
                            let _ = stream_mut.compress();
                        }
                    }
                }
            }
        }
    }

    // Remove deleted streams from page /Contents
    if !deleted_streams.is_empty() {
        if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            if let Ok(contents) = page_dict.get_mut(b"Contents") {
                if let Object::Array(ref mut arr) = contents {
                    arr.retain(|item| {
                        if let Object::Reference(id) = item {
                            !deleted_streams.contains(id)
                        } else {
                            true
                        }
                    });
                }
            }
        }
        for del_id in deleted_streams {
            doc.delete_object(del_id);
        }
    }

    Ok(())
}

pub fn clean_page_numbers(doc: &mut Document, page_id: ObjectId) -> Result<(), PdfError> {
    let mbox = media_box(doc, page_id);
    let y0 = mbox[1].min(mbox[3]);
    let height = (mbox[3] - mbox[1]).abs();
    let height = if height > 0.0 { height } else { 792.0 };

    let mut stream_ids: Vec<ObjectId> = Vec::new();
    let mut is_array_contents = false;

    if let Ok(page_dict) = doc.get_object(page_id).and_then(|o| o.as_dict()) {
        if let Ok(contents) = page_dict.get(b"Contents") {
            match contents {
                Object::Reference(id) => {
                    stream_ids.push(*id);
                }
                Object::Array(arr) => {
                    is_array_contents = true;
                    for item in arr {
                        if let Object::Reference(id) = item {
                            stream_ids.push(*id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let no_zones = WatermarkZones {
        top: false,
        center: false,
        bottom: false,
    };
    let empty_xobjs: Vec<Vec<u8>> = Vec::new();
    let mut deleted_streams: Vec<ObjectId> = Vec::new();

    for stream_id in &stream_ids {
        if let Ok(stream) = doc.get_object(*stream_id).and_then(|o| o.as_stream()) {
            let decompressed = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());

            if let Ok(content) = Content::decode(&decompressed) {
                let (new_ops, modified) = clean_operations(content.operations, &empty_xobjs, None, y0, height, &no_zones, true);
                if modified {
                    if new_ops.is_empty() && is_array_contents && stream_ids.len() > 1 {
                        deleted_streams.push(*stream_id);
                    } else if let Ok(stream_mut) = doc.get_object_mut(*stream_id).and_then(|o| o.as_stream_mut()) {
                        let new_content = Content { operations: new_ops };
                        if let Ok(encoded) = new_content.encode() {
                            stream_mut.set_plain_content(encoded);
                            let _ = stream_mut.compress();
                        }
                    }
                }
            }
        }
    }

    if !deleted_streams.is_empty() {
        if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            if let Ok(contents) = page_dict.get_mut(b"Contents") {
                if let Object::Array(ref mut arr) = contents {
                    arr.retain(|item| {
                        if let Object::Reference(id) = item {
                            !deleted_streams.contains(id)
                        } else {
                            true
                        }
                    });
                }
            }
        }
        for del_id in deleted_streams {
            doc.delete_object(del_id);
        }
    }

    Ok(())
}

pub fn clean_doc_page_numbers(doc: &mut Document) -> Result<(), PdfError> {
    let page_ids: Vec<ObjectId> = doc.page_iter().collect();
    for page_id in page_ids {
        clean_page_numbers(doc, page_id)?;
    }
    Ok(())
}

pub fn apply_page_numbers(
    input: &Path,
    output: &Path,
    font_size: f64,
    margin: f64,
) -> Result<OpResult, PdfError> {
    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    // 1. Clean existing page numbers from all pages first
    clean_doc_page_numbers(&mut doc)?;

    let page_ids: Vec<lopdf::ObjectId> = doc.page_iter().collect();
    if page_ids.is_empty() {
        return Err(PdfError::EmptyOutput);
    }
    let page_count = page_ids.len() as u32;

    // 2. Add the clean, primary PDFKit page number to each page
    for (index, page_id) in page_ids.into_iter().enumerate() {
        ensure_helvetica(&mut doc, page_id)?;
        let stream = page_number_stream(&mut doc, page_id, index as u32 + 1, page_count, font_size, margin)?;
        doc.add_page_contents(page_id, stream)?;
    }

    doc.compress();

    // Check if input and output point to the same file (safe atomic write)
    let is_same = input == output
        || std::fs::canonicalize(input).ok() == std::fs::canonicalize(output).ok();

    if is_same {
        let temp_output = output.with_file_name(format!(
            ".tmp_num_{}.pdf",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
        ));
        doc.save(&temp_output)?;
        drop(doc);
        if std::fs::rename(&temp_output, output).is_err() {
            std::fs::copy(&temp_output, output)?;
            let _ = std::fs::remove_file(&temp_output);
        }
    } else {
        doc.save(output)?;
        drop(doc);
    }

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: page_count,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    Ok(result)
}

/// Shared scaffolding: load, stamp every page with the stream produced by
/// `make_stream` (receives doc, page id, 1-based index and page count),
/// save, report.
fn annotate<F>(
    input: &Path,
    output: &Path,
    mut make_stream: F,
) -> Result<OpResult, PdfError>
where
    F: FnMut(&mut Document, lopdf::ObjectId, u32, u32) -> Result<Vec<u8>, PdfError>,
{
    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let page_ids: Vec<lopdf::ObjectId> = doc.page_iter().collect();
    if page_ids.is_empty() {
        return Err(PdfError::EmptyOutput);
    }
    let page_count = page_ids.len() as u32;

    for (index, page_id) in page_ids.into_iter().enumerate() {
        ensure_helvetica(&mut doc, page_id)?;
        let stream = make_stream(&mut doc, page_id, index as u32 + 1, page_count)?;
        doc.add_page_contents(page_id, stream)?;
    }

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

fn watermark_stream(
    doc: &mut Document,
    page_id: lopdf::ObjectId,
    text: &str,
    font_size: f64,
    opacity: f64,
) -> Result<Vec<u8>, PdfError> {
    let [x0, y0, x1, y1] = media_box(doc, page_id);
    let (w, h) = (x1 - x0, y1 - y0);
    let cx = x0 + w / 2.0;
    let cy = y0 + h / 2.0;

    // Graphics state for transparency.
    let gs_id = doc.add_object(Dictionary::from_iter(vec![
        ("ca", Object::Real(opacity.clamp(0.0, 1.0) as f32)),
        ("CA", Object::Real(opacity.clamp(0.0, 1.0) as f32)),
    ]));
    doc.add_graphics_state(page_id, "GS1", gs_id)?;

    // Rough Helvetica advance estimate (~0.5 em per char) to center the text.
    let est_width = font_size * 0.5 * text.chars().count() as f64;
    let angle = 45f64.to_radians();
    let (c, s) = (angle.cos(), angle.sin());

    Ok(format!(
        "q\n/GS1 gs\n0.6 g\nBT /PK_Font {size:.1} Tf\n{c:.4} {s:.4} {ns:.4} {nc:.4} {tx:.1} {ty:.1} Tm\n({text}) Tj\nET\nQ\n",
        size = font_size,
        c = c,
        s = s,
        ns = -s,
        nc = c,
        tx = cx - est_width / 2.0,
        ty = cy,
        text = escape_pdf_text(text),
    )
    .into_bytes())
}

fn page_number_stream(
    doc: &mut Document,
    page_id: lopdf::ObjectId,
    number: u32,
    total: u32,
    font_size: f64,
    margin: f64,
) -> Result<Vec<u8>, PdfError> {
    let [x0, y0, x1, _] = media_box(doc, page_id);
    let label = format!("{number} / {total}");
    let est_width = font_size * 0.5 * label.len() as f64;
    let x = x0 + (x1 - x0) / 2.0 - est_width / 2.0;
    let y = y0 + margin;

    Ok(format!(
        "q\n0.35 g\nBT /PK_Font {size:.1} Tf\n{x:.1} {y:.1} Td\n({label}) Tj\nET\nQ\n",
        size = font_size,
        x = x,
        y = y,
        label = label,
    )
    .into_bytes())
}

pub fn apply_overlays(
    input: &Path,
    input_bytes: Option<&[u8]>,
    output: &Path,
    overlays: Vec<PageOverlay>,
    editor_data: Option<String>,
) -> Result<OpResult, PdfError> {
    let is_in_place = input_bytes.is_none()
        && (input == output
            || std::fs::canonicalize(input).ok() == std::fs::canonicalize(output).ok());

    if overlays.is_empty() && editor_data.is_none() {
        if let Some(bytes) = input_bytes {
            std::fs::write(output, bytes)?;
            let total_pages = match Document::load_mem(bytes) {
                Ok(d) => d.get_pages().len() as u32,
                Err(_) => 0,
            };
            return Ok(OpResult {
                outputs: vec![output.to_string_lossy().into_owned()],
                total_pages,
                total_size_bytes: std::fs::metadata(output)?.len(),
            });
        }
        if !is_in_place {
            std::fs::copy(input, output)?;
        }
        return Ok(OpResult {
            outputs: vec![output.to_string_lossy().into_owned()],
            total_pages: 0,
            total_size_bytes: std::fs::metadata(output)?.len(),
        });
    }

    let mut doc = if let Some(bytes) = input_bytes {
        Document::load_mem(bytes)?
    } else {
        Document::load(input)?
    };

    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    // Strip previous PDFKit overlays so edits don't accumulate or ghost
    strip_pdfkit_overlays(&mut doc);

    let page_map = doc.get_pages();
    let total_pages = page_map.len() as u32;
    let mut created_stream_ids = Vec::new();
    let mut has_redactions = false;

    for (idx, overlay) in overlays.into_iter().enumerate() {
        let Some(&page_id) = page_map.get(&overlay.page) else {
            continue;
        };

        if overlay.png_data.is_empty() {
            continue;
        }

        let img_id = build_rgba_image_xobject(&mut doc, &overlay.png_data)?;
        let xobj_name = format!("ImOvl{}", idx);

        doc.get_or_create_resources(page_id)?;
        doc.add_xobject(page_id, xobj_name.as_bytes(), img_id)?;

        let (x, y, w, h) = if let Some([bx, by, bw, bh]) = overlay.bbox {
            (bx, by, bw, bh)
        } else {
            let [x0, y0, x1, y1] = media_box(&doc, page_id);
            (x0, y0, x1 - x0, y1 - y0)
        };

        let content = format!(
            "q\n{w:.2} 0 0 {h:.2} {x:.2} {y:.2} cm\n/{name} Do\nQ\n",
            w = w,
            h = h,
            x = x,
            y = y,
            name = xobj_name
        );

        let mut stream_dict = Dictionary::new();
        stream_dict.set("PDFKit_Overlay", Object::Boolean(true));
        let stream = Stream::new(stream_dict, content.into_bytes());
        let stream_id = doc.add_object(stream);

        let page = doc.get_dictionary_mut(page_id)?;
        if overlay.replace_page_content {
            has_redactions = true;
            // True physical redaction: replace Contents stream entirely with the flattened raster stream.
            // All original vector text operators ((text) Tj / TJ) and glyph streams are permanently destroyed!
            page.set("Contents", Object::Reference(stream_id));
            // Remove any annotations/form fields/links so no hidden searchable text widgets remain
            page.remove(b"Annots");
        } else {
            match page.get_mut(b"Contents") {
                Ok(Object::Array(ref mut arr)) => {
                    arr.push(Object::Reference(stream_id));
                }
                Ok(Object::Reference(existing)) => {
                    let existing = *existing;
                    page.set("Contents", Object::Array(vec![Object::Reference(existing), Object::Reference(stream_id)]));
                }
                _ => {
                    page.set("Contents", Object::Array(vec![Object::Reference(stream_id)]));
                }
            }
            created_stream_ids.push(stream_id);
        }
    }

    if !created_stream_ids.is_empty() {
        let overlay_array = Object::Array(created_stream_ids.into_iter().map(Object::Reference).collect());
        doc.catalog_mut()?.set("PDFKit_OverlayStreams", overlay_array);
    }

    if let Some(json_str) = editor_data {
        let stream = Stream::new(Dictionary::new(), json_str.into_bytes());
        let stream_id = doc.add_object(stream);
        doc.catalog_mut()?.set("PDFKit_EditorData", Object::Reference(stream_id));
    } else if let Ok(cat) = doc.catalog_mut() {
        cat.remove(b"PDFKit_EditorData");
    }

    let target_save = if is_in_place {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        let file_stem = output.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
        parent.join(format!("{}.pdfkit_tmp_{}.pdf", file_stem, std::process::id()))
    } else {
        output.to_path_buf()
    };

    if has_redactions {
        // Physical purge of all orphaned vector streams, text glyphs, and unreferenced resources
        // ensuring zero remnant of redacted text remains in the document object table.
        doc.trailer.remove(b"Info");
        if let Ok(catalog) = doc.catalog_mut() {
            catalog.remove(b"Metadata");
        }
        doc.prune_objects();
    }

    doc.compress();
    doc.save(&target_save)?;

    if is_in_place {
        // On Windows, rename can fail with os error 5 (Access Denied) if target exists or is locked.
        let mut replace_ok = false;
        if std::fs::rename(&target_save, output).is_ok() {
            replace_ok = true;
        } else if std::fs::remove_file(output).is_ok() && std::fs::rename(&target_save, output).is_ok() {
            replace_ok = true;
        } else if std::fs::copy(&target_save, output).is_ok() {
            replace_ok = true;
            let _ = std::fs::remove_file(&target_save);
        }

        if !replace_ok {
            let _ = std::fs::remove_file(&target_save);
            return Err(PdfError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Dosya başka bir programda açık (!). Lütfen dosyayı diğer programda kapatın veya 'Farklı Kaydet' ile kaydedin.",
            )));
        }
    }

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}

pub fn read_editor_document(path: &Path) -> Result<EditorLoadResult, PdfError> {
    let mut doc = Document::load(path)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let annotations_json = get_catalog_stream_string(&doc, b"PDFKit_EditorData");
    if annotations_json.is_none() {
        return Ok(EditorLoadResult {
            annotations_json: None,
            clean_pdf_bytes: None,
        });
    }

    let stripped = strip_pdfkit_overlays(&mut doc);
    let clean_pdf_bytes = if stripped {
        doc.prune_objects();
        let mut buf = Vec::new();
        doc.save_to(&mut buf)?;
        Some(buf)
    } else {
        None
    };

    Ok(EditorLoadResult {
        annotations_json,
        clean_pdf_bytes,
    })
}

pub fn strip_pdfkit_overlays(doc: &mut Document) -> bool {
    let Ok(cat) = doc.catalog() else { return false };
    let overlay_stream_ids: Vec<ObjectId> = match cat.get(b"PDFKit_OverlayStreams") {
        Ok(Object::Array(arr)) => arr.iter().filter_map(|obj| obj.as_reference().ok()).collect(),
        _ => Vec::new(),
    };
    if overlay_stream_ids.is_empty() {
        return false;
    }
    let page_map = doc.get_pages();
    for &page_id in page_map.values() {
        if let Ok(page_dict) = doc.get_dictionary_mut(page_id) {
            if let Ok(contents) = page_dict.get_mut(b"Contents") {
                match contents {
                    Object::Array(ref mut arr) => {
                        let backup = arr.clone();
                        arr.retain(|item| {
                            if let Ok(id) = item.as_reference() {
                                !overlay_stream_ids.contains(&id)
                            } else {
                                true
                            }
                        });
                        // Safety check: Never leave the page with zero contents (which makes it completely blank white)
                        if arr.is_empty() && !backup.is_empty() {
                            *arr = backup;
                        }
                    }
                    Object::Reference(_id) => {
                        // Never remove a single Contents reference, otherwise the page becomes 100% blank!
                    }
                    _ => {}
                }
            }
        }
    }
    for id in &overlay_stream_ids {
        doc.objects.remove(id);
    }
    doc.prune_objects();
    if let Ok(cat_mut) = doc.catalog_mut() {
        cat_mut.remove(b"PDFKit_OverlayStreams");
    }
    true
}

fn get_catalog_stream_string(doc: &Document, key: &[u8]) -> Option<String> {
    let cat = doc.catalog().ok()?;
    let obj = cat.get(key).ok()?;
    match obj {
        Object::Reference(id) => {
            let stream = doc.get_object(*id).ok()?.as_stream().ok()?;
            let content = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
            String::from_utf8(content).ok()
        }
        Object::String(bytes, _) => String::from_utf8(bytes.clone()).ok(),
        _ => None,
    }
}

fn build_rgba_image_xobject(doc: &mut Document, bytes: &[u8]) -> Result<ObjectId, PdfError> {
    let img = image::load_from_memory(bytes).map_err(|e| PdfError::Image(e.to_string()))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.as_raw();

    let mut rgb_bytes = Vec::with_capacity((width * height * 3) as usize);
    let mut alpha_bytes = Vec::with_capacity((width * height) as usize);
    let mut has_transparency = false;

    for chunk in pixels.chunks_exact(4) {
        rgb_bytes.push(chunk[0]);
        rgb_bytes.push(chunk[1]);
        rgb_bytes.push(chunk[2]);
        let a = chunk[3];
        alpha_bytes.push(a);
        if a < 255 {
            has_transparency = true;
        }
    }

    let mut rgb_encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    rgb_encoder.write_all(&rgb_bytes)?;
    let compressed_rgb = rgb_encoder.finish()?;

    let mut image_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"XObject".to_vec())),
        ("Subtype", Object::Name(b"Image".to_vec())),
        ("Width", Object::Integer(i64::from(width))),
        ("Height", Object::Integer(i64::from(height))),
        ("ColorSpace", Object::Name(b"DeviceRGB".to_vec())),
        ("BitsPerComponent", Object::Integer(8)),
        ("Filter", Object::Name(b"FlateDecode".to_vec())),
    ]);

    if has_transparency {
        let mut alpha_encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        alpha_encoder.write_all(&alpha_bytes)?;
        let compressed_alpha = alpha_encoder.finish()?;

        let smask_dict = Dictionary::from_iter(vec![
            ("Type", Object::Name(b"XObject".to_vec())),
            ("Subtype", Object::Name(b"Image".to_vec())),
            ("Width", Object::Integer(i64::from(width))),
            ("Height", Object::Integer(i64::from(height))),
            ("ColorSpace", Object::Name(b"DeviceGray".to_vec())),
            ("BitsPerComponent", Object::Integer(8)),
            ("Filter", Object::Name(b"FlateDecode".to_vec())),
        ]);
        let smask_stream = Stream::new(smask_dict, compressed_alpha);
        let smask_id = doc.add_object(smask_stream);
        image_dict.set("SMask", Object::Reference(smask_id));
    }

    let image_stream = Stream::new(image_dict, compressed_rgb);
    let img_id = doc.add_object(image_stream);
    Ok(img_id)
}
