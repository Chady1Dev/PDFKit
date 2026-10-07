use serde::Serialize;

/// Unified error type for all PDF operations.
///
/// Serialized to a plain string for the frontend so the IPC boundary stays
/// simple and type-safe on the Rust side.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("{}", format_io_error(.0))]
    Io(#[from] std::io::Error),

    #[error("PDF error: {0}")]
    Lopdf(#[from] lopdf::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("document is password-protected")]
    Encrypted,

    #[error("wrong password")]
    WrongPassword,

    #[error("invalid page range: start {start}, end {end}, page count {page_count}")]
    InvalidRange {
        start: u32,
        end: u32,
        page_count: u32,
    },

    #[error("invalid rotation angle {0}; allowed: 90, 180, 270, -90, -180, -270")]
    InvalidAngle(i64),

    #[error("page {page} does not exist (document has {page_count} pages)")]
    PageOutOfBounds { page: u32, page_count: u32 },

    #[error("at least {0} input files are required")]
    NotEnoughInputs(usize),

    #[error("no valid pages to write")]
    EmptyOutput,

    #[error("image decode error: {0}")]
    Image(String),

    #[error("background task failed: {0}")]
    TaskJoin(String),

    #[error("{0}")]
    Custom(String),
}

impl Serialize for PdfError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn format_io_error(err: &std::io::Error) -> String {
    #[cfg(target_os = "windows")]
    {
        if err.raw_os_error() == Some(32) {
            return "Dosya başka bir programda açık (!)".to_string();
        }
    }
    if err.kind() == std::io::ErrorKind::PermissionDenied {
        return "Dosya başka bir programda açık (!)".to_string();
    }
    format!("Dosya okuma/yazma hatası: {}", err)
}

