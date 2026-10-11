use serde::{Deserialize, Serialize};

/// Metadata about a PDF document, returned by `get_pdf_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfInfo {
    pub path: String,
    pub page_count: u32,
    pub version: String,
    pub encrypted: bool,
    pub file_size_bytes: u64,
}

/// Result of a write operation (merge / split / rotate / organize / ...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpResult {
    /// Files written to disk (split may produce several).
    pub outputs: Vec<String>,
    /// Total pages across all outputs.
    pub total_pages: u32,
    /// Combined size of the outputs in bytes.
    pub total_size_bytes: u64,
}

/// Result of the compress operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressResult {
    pub output: String,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub already_optimized: bool,
}

/// One AcroForm field, returned by `list_form_fields`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub value: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub export_value: Option<String>,
}
