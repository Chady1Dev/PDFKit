use serde::{Deserialize, Serialize};

/// High-level comparison mode selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareMode {
    /// Combines Text, Visual, and Structural layers without duplicating text diffs in visual layer.
    Auto,
    /// Only text differences (insertions, deletions, replacements, moves).
    TextOnly,
    /// Emphasizes visual/pixel, layout, image, and graphic differences.
    VisualOnly,
    /// Emphasizes object, formatting, style, annotation, form, and structural differences.
    ContentStructure,
}

impl Default for CompareMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// Text normalization options controlled by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationOptions {
    /// Ignore uppercase/lowercase differences (Turkish-aware: İ/i and I/ı).
    #[serde(default)]
    pub ignore_case: bool,
    /// Collapse multiple spaces, tabs, and line breaks into a single space.
    #[serde(default = "default_true")]
    pub ignore_whitespace: bool,
    /// Normalize word breaks caused by end-of-line hyphens (e.g. "karşı-\nlaştırma" -> "karşılaştırma").
    #[serde(default = "default_true")]
    pub normalize_hyphenation: bool,
    /// Apply Unicode NFC normalization while preserving Turkish characters (İ, ı, Ş, ş, Ğ, ğ, Ç, ç, Ö, ö, Ü, ü).
    #[serde(default = "default_true")]
    pub unicode_normalization: bool,
    /// Expand common typographic ligatures (ﬁ -> fi, ﬂ -> fl, ﬀ -> ff, ﬃ -> ffi, ﬄ -> ffl) and OCR spacing artifacts.
    #[serde(default = "default_true")]
    pub normalize_ligatures: bool,
}

fn default_true() -> bool {
    true
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            ignore_case: false,
            ignore_whitespace: true,
            normalize_hyphenation: true,
            unicode_normalization: true,
            normalize_ligatures: true,
        }
    }
}

/// Options passed to `compare_pdfs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareOptions {
    #[serde(default)]
    pub mode: CompareMode,
    #[serde(default)]
    pub normalization: NormalizationOptions,
    /// Pixel color tolerance (0..255) for visual diff. Default: 24.
    #[serde(default = "default_visual_tolerance")]
    pub visual_tolerance: u8,
    /// Whether to ignore 1px anti-aliasing edge jitter in visual comparison.
    #[serde(default = "default_true")]
    pub ignore_antialiasing: bool,
    /// Minimum similarity score (0.0..1.0) required to pair two pages. Default: 0.28.
    #[serde(default = "default_min_page_similarity")]
    pub min_page_similarity: f64,
}

fn default_visual_tolerance() -> u8 {
    24
}

fn default_min_page_similarity() -> f64 {
    0.28
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            mode: CompareMode::Auto,
            normalization: NormalizationOptions::default(),
            visual_tolerance: 24,
            ignore_antialiasing: true,
            min_page_similarity: 0.28,
        }
    }
}

/// Which of the 3 distinct architectural layers produced this change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeLayer {
    /// Layer 1: Word, character, line, paragraph, or moved text differences.
    Text,
    /// Layer 2: Pixel/visual, graphic, or layout rendering differences.
    Visual,
    /// Layer 3: Document structure (page add/delete/move, geometry, forms, annotations, images, vectors, headers/footers).
    Structure,
}

/// Filterable category for changes in the UI and summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeCategory {
    Text,
    Formatting,
    ImagesGraphics,
    Annotations,
    Forms,
    HeaderFooter,
    Background,
    Pages,
}

/// Specific kind of change detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    // Text layer
    TextAdded,
    TextDeleted,
    TextModified,
    TextMoved,
    LineParagraphChanged,
    // Formatting & layout
    FormattingChanged,
    LayoutShifted,
    // Images & graphics & tables
    ImageAdded,
    ImageDeleted,
    ImageModified,
    ImageMoved,
    VectorChanged,
    TableChanged,
    VisualRegionDiff,
    // Annotations & forms
    AnnotationAdded,
    AnnotationDeleted,
    AnnotationModified,
    FormFieldAdded,
    FormFieldDeleted,
    FormFieldModified,
    // Header / Footer / Background
    HeaderFooterChanged,
    BackgroundChanged,
    // Page structure & geometry
    PageAdded,
    PageDeleted,
    PageMoved,
    PageGeometryChanged,
}

/// Bounding box in normalized PDF page coordinates (points, origin = top-left of visible page).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BoundingBox {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width: width.max(1.0),
            height: height.max(1.0),
        }
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn area(&self) -> f64 {
        self.width.max(0.0) * self.height.max(0.0)
    }

    pub fn union(&self, other: &Self) -> Self {
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = self.right().max(other.right());
        let max_y = self.bottom().max(other.bottom());
        Self::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn intersects_or_near(&self, other: &Self, margin: f64) -> bool {
        !(self.right() + margin < other.x
            || other.right() + margin < self.x
            || self.bottom() + margin < other.y
            || other.bottom() + margin < self.y)
    }

    pub fn overlap_ratio(&self, other: &Self) -> f64 {
        let ix0 = self.x.max(other.x);
        let iy0 = self.y.max(other.y);
        let ix1 = self.right().min(other.right());
        let iy1 = self.bottom().min(other.bottom());
        if ix1 <= ix0 || iy1 <= iy0 {
            return 0.0;
        }
        let inter = (ix1 - ix0) * (iy1 - iy0);
        let min_area = self.area().min(other.area()).max(1.0);
        inter / min_area
    }
}

/// One detected difference item in the unified change model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffChangeItem {
    pub id: String,
    pub layer: ChangeLayer,
    pub category: ChangeCategory,
    pub kind: ChangeKind,
    /// 1-based page number in the left (reference/old) document, if applicable.
    pub left_page: Option<u32>,
    /// 1-based page number in the right (compared/new) document, if applicable.
    pub right_page: Option<u32>,
    /// Bounding box on the left page (in PDF points, top-left origin).
    pub left_rect: Option<BoundingBox>,
    /// Bounding box on the right page (in PDF points, top-left origin).
    pub right_rect: Option<BoundingBox>,
    /// Human-readable title in Turkish.
    pub title: String,
    /// Detailed explanation of the change.
    pub detail: String,
    /// Previous text / value (in reference document).
    pub before_text: Option<String>,
    /// New text / value (in compared document).
    pub after_text: Option<String>,
    /// True if classification relies on coordinate/repetition heuristics (e.g. header/footer).
    #[serde(default)]
    pub is_heuristic: bool,
    /// Severity / significance score (0.0..1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    1.0
}

/// Alignment status of a page pair between Document 1 and Document 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageMatchStatus {
    /// Same relative order, no changes detected.
    Identical,
    /// Same relative order, content/visual/structural changes detected.
    Modified,
    /// Page was moved to a different position in the new document.
    Moved,
    /// Page exists only in the old (left) document.
    Deleted,
    /// Page exists only in the new (right) document.
    Added,
}

/// Represents how a page in Document 1 aligns with a page in Document 2.
/// Used by synchronized scrolling, side-by-side display, and change navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageAlignment {
    /// 1-based page in Left PDF (`None` if page was added in Right PDF).
    pub left_page: Option<u32>,
    /// 1-based page in Right PDF (`None` if page was deleted from Left PDF).
    pub right_page: Option<u32>,
    pub status: PageMatchStatus,
    /// Content & structural similarity score (0.0..1.0).
    pub similarity: f64,
    /// True when the page pairing similarity was borderline and may have uncertainty.
    pub low_confidence: bool,
    /// True if geometry (MediaBox / CropBox / Rotate) differs between the paired pages.
    pub geometry_differs: bool,
    /// True if either page appears to be a scanned / image-only page without reliable text layer.
    pub is_scanned: bool,
    /// Left page dimensions in points `[width, height]` and rotation.
    pub left_dimensions: Option<[f64; 2]>,
    pub right_dimensions: Option<[f64; 2]>,
    pub left_rotation: i64,
    pub right_rotation: i64,
}

/// Summary statistics for the comparison result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub total_changes: usize,
    // Text breakdown
    pub text_added_count: usize,
    pub text_deleted_count: usize,
    pub text_modified_count: usize,
    pub text_moved_count: usize,
    // Visual & formatting breakdown
    pub formatting_changes_count: usize,
    pub visual_changes_count: usize,
    pub image_graphic_changes_count: usize,
    // Structure / annotations / forms
    pub annotation_changes_count: usize,
    pub form_changes_count: usize,
    pub header_footer_changes_count: usize,
    pub background_changes_count: usize,
    // Page counts
    pub pages_added_count: usize,
    pub pages_deleted_count: usize,
    pub pages_moved_count: usize,
    pub pages_modified_count: usize,
    pub pages_identical_count: usize,
    /// 1-based left page numbers that contain at least one change.
    pub changed_left_pages: Vec<u32>,
    /// 1-based right page numbers that contain at least one change.
    pub changed_right_pages: Vec<u32>,
}

/// Optional text/style items extracted by `pdf.js` in the frontend to enrich comparison
/// when a PDF uses custom font encodings not embedded in ToUnicode maps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedTextSpan {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub font_name: String,
    #[serde(default)]
    pub font_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedPageData {
    pub page: u32,
    pub spans: Vec<EnrichedTextSpan>,
}

/// Complete response from `compare_pdfs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub left_path: String,
    pub right_path: String,
    pub left_page_count: u32,
    pub right_page_count: u32,
    pub mode: CompareMode,
    pub options: CompareOptions,
    pub alignments: Vec<PageAlignment>,
    pub changes: Vec<DiffChangeItem>,
    pub summary: ComparisonSummary,
    /// Warnings about low-confidence page matches or scanned pages without text layers.
    pub uncertainties: Vec<String>,
    /// Explicit disclosures of PDF features or heuristic limitations encountered.
    pub limitations: Vec<String>,
    pub duration_ms: u64,
}

/// Request payload for comparing two rendered page images (RGBA or PNG bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RasterCompareRequest {
    pub left_page: u32,
    pub right_page: u32,
    /// PNG encoded bytes of left page render.
    pub left_png: Vec<u8>,
    /// PNG encoded bytes of right page render.
    pub right_png: Vec<u8>,
    /// Page point dimensions `[width_pt, height_pt]` of left page.
    pub left_dims_pt: [f64; 2],
    /// Page point dimensions `[width_pt, height_pt]` of right page.
    pub right_dims_pt: [f64; 2],
    #[serde(default = "default_visual_tolerance")]
    pub tolerance: u8,
    #[serde(default = "default_true")]
    pub ignore_antialiasing: bool,
    /// Optional text diff bounding boxes on this page pair to exclude in Auto mode.
    #[serde(default)]
    pub exclude_rects_pt: Vec<BoundingBox>,
}

/// Response from `compare_page_rasters`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RasterCompareResult {
    pub left_page: u32,
    pub right_page: u32,
    pub diff_ratio: f64,
    pub changed_pixel_count: usize,
    pub total_pixel_count: usize,
    pub geometry_compatible: bool,
    pub geometry_warning: Option<String>,
    /// Merged visual difference regions in PDF point coordinates (top-left origin).
    pub regions_pt: Vec<BoundingBox>,
    /// PNG encoded heatmap / difference image (if differences exist).
    pub heatmap_png: Option<Vec<u8>>,
}

/// Progress event payload emitted during comparison (`compare-progress`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareProgressEvent {
    pub current: u32,
    pub total: u32,
    pub percent: u32,
    pub stage: String,
}
