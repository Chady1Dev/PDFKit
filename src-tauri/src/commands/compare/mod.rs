pub mod extractor;
pub mod matcher;
pub mod report;
pub mod structural_diff;
pub mod text_diff;
pub mod types;
pub mod visual_diff;

use crate::error::PdfError;
use crate::types::OpResult;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::Emitter;

pub use types::*;

static CANCEL_COMPARISON: AtomicBool = AtomicBool::new(false);

pub fn request_cancel_comparison() {
    CANCEL_COMPARISON.store(true, Ordering::SeqCst);
}

pub fn reset_cancel_flag() {
    CANCEL_COMPARISON.store(false, Ordering::SeqCst);
}

pub fn is_comparison_cancelled() -> bool {
    CANCEL_COMPARISON.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn cancel_comparison() {
    request_cancel_comparison();
}

#[tauri::command]
pub async fn compare_pdfs(
    app: tauri::AppHandle,
    left_path: String,
    right_path: String,
    options: Option<CompareOptions>,
    left_enriched: Option<Vec<EnrichedPageData>>,
    right_enriched: Option<Vec<EnrichedPageData>>,
) -> Result<ComparisonResult, PdfError> {
    reset_cancel_flag();
    tauri::async_runtime::spawn_blocking(move || {
        compare_pdf_files_with_progress(
            Path::new(&left_path),
            Path::new(&right_path),
            options.unwrap_or_default(),
            left_enriched.as_deref(),
            right_enriched.as_deref(),
            |curr, tot, pct, stage| {
                let _ = app.emit(
                    "compare-progress",
                    CompareProgressEvent {
                        current: curr,
                        total: tot,
                        percent: pct,
                        stage: stage.to_string(),
                    },
                );
            },
        )
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

#[tauri::command]
pub async fn compare_page_rasters(
    request: RasterCompareRequest,
) -> Result<RasterCompareResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || visual_diff::compare_page_pngs(&request))
        .await
        .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

#[tauri::command]
pub async fn export_comparison_report(
    result: ComparisonResult,
    output_path: String,
    format: String,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        report::write_comparison_report(&result, Path::new(&output_path), &format)
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Core synchronous comparison entry point (usable directly from Rust tests and `spawn_blocking`).
pub fn compare_pdf_files(
    left_path: &Path,
    right_path: &Path,
    options: CompareOptions,
) -> Result<ComparisonResult, PdfError> {
    compare_pdf_files_with_progress(left_path, right_path, options, None, None, |_, _, _, _| {})
}

pub fn compare_pdf_files_with_progress<F>(
    left_path: &Path,
    right_path: &Path,
    options: CompareOptions,
    left_enriched: Option<&[EnrichedPageData]>,
    right_enriched: Option<&[EnrichedPageData]>,
    mut on_progress: F,
) -> Result<ComparisonResult, PdfError>
where
    F: FnMut(u32, u32, u32, &str),
{
    let start_time = Instant::now();

    if is_comparison_cancelled() {
        return Err(PdfError::Custom("Karşılaştırma işlemi kullanıcı tarafından iptal edildi.".into()));
    }

    on_progress(0, 100, 5, "Referans PDF analiz ediliyor…");
    let doc_left = extractor::extract_document(left_path, left_enriched)?;

    if is_comparison_cancelled() {
        return Err(PdfError::Custom("Karşılaştırma işlemi kullanıcı tarafından iptal edildi.".into()));
    }

    on_progress(1, 100, 25, "Karşılaştırılan PDF analiz ediliyor…");
    let doc_right = extractor::extract_document(right_path, right_enriched)?;

    if is_comparison_cancelled() {
        return Err(PdfError::Custom("Karşılaştırma işlemi kullanıcı tarafından iptal edildi.".into()));
    }

    on_progress(2, 100, 40, "Sayfalar eşleştiriliyor…");
    let (mut alignments, mut all_changes, mut uncertainties) =
        matcher::match_document_pages(&doc_left.pages, &doc_right.pages, &options);

    let mut change_counter = all_changes.len();

    // Document-wide form field comparison
    let form_changes = structural_diff::compare_document_forms(
        &doc_left.form_fields,
        &doc_right.form_fields,
        &options,
        &mut change_counter,
    );
    all_changes.extend(form_changes);

    // Per-matched-page text, formatting, and structural comparison
    let total_pairs = alignments.len().max(1) as u32;
    for (idx, align) in alignments.iter_mut().enumerate() {
        if is_comparison_cancelled() {
            return Err(PdfError::Custom("Karşılaştırma işlemi kullanıcı tarafından iptal edildi.".into()));
        }

        let pct = 45 + ((idx as u32 * 50) / total_pairs);
        on_progress(
            idx as u32 + 1,
            total_pairs,
            pct,
            &format!("Sayfa çifti {} / {} karşılaştırılıyor…", idx + 1, total_pairs),
        );

        if let (Some(lp_num), Some(rp_num)) = (align.left_page, align.right_page) {
            let Some(lp) = doc_left.pages.get((lp_num - 1) as usize) else {
                continue;
            };
            let Some(rp) = doc_right.pages.get((rp_num - 1) as usize) else {
                continue;
            };

            let mut pair_changes = Vec::new();
            pair_changes.extend(text_diff::compare_page_text(
                lp,
                rp,
                &options,
                &mut change_counter,
            ));
            pair_changes.extend(structural_diff::compare_page_structure(
                lp,
                rp,
                &options,
                &mut change_counter,
            ));

            // Update alignment status based on whether any actual differences were found on this pair
            if align.status != PageMatchStatus::Moved {
                if pair_changes.is_empty() && !align.geometry_differs {
                    align.status = PageMatchStatus::Identical;
                    align.similarity = 1.0;
                } else {
                    align.status = PageMatchStatus::Modified;
                }
            }

            all_changes.extend(pair_changes);
        }
    }

    // Filter changes according to selected CompareMode
    all_changes.retain(|c| match options.mode {
        CompareMode::Auto => true,
        CompareMode::TextOnly => c.category == ChangeCategory::Text,
        CompareMode::VisualOnly => matches!(
            c.category,
            ChangeCategory::ImagesGraphics
                | ChangeCategory::Background
                | ChangeCategory::Formatting
                | ChangeCategory::Pages
        ),
        CompareMode::ContentStructure => true,
    });

    // Collect explicit limitations & heuristic disclosures
    let mut limitations = Vec::new();
    for feat in doc_left
        .unsupported_features
        .iter()
        .chain(doc_right.unsupported_features.iter())
    {
        if !limitations.contains(feat) {
            limitations.push(feat.clone());
        }
    }
    if all_changes.iter().any(|c| c.is_heuristic) {
        limitations.push(
            "Üstbilgi, altbilgi ve arka plan sınıflandırmaları sayfa koordinatları ve çoklu sayfa tekrar sezgisellerine (heuristics) dayanır.".to_string(),
        );
    }
    if alignments.iter().any(|a| a.is_scanned) {
        let msg = "Bazı sayfalar taranmış görüntü (metin katmansız) içeriyor; bu sayfalarda metin farkı yerine görsel karşılaştırma uygulanır.".to_string();
        if !uncertainties.contains(&msg) {
            uncertainties.push(msg);
        }
    }

    let summary = build_summary(&all_changes, &alignments);
    let duration_ms = start_time.elapsed().as_millis() as u64;

    on_progress(total_pairs, total_pairs, 100, "Karşılaştırma tamamlandı.");

    Ok(ComparisonResult {
        left_path: doc_left.path,
        right_path: doc_right.path,
        left_page_count: doc_left.page_count,
        right_page_count: doc_right.page_count,
        mode: options.mode,
        options,
        alignments,
        changes: all_changes,
        summary,
        uncertainties,
        limitations,
        duration_ms,
    })
}

pub fn build_summary(
    changes: &[DiffChangeItem],
    alignments: &[PageAlignment],
) -> ComparisonSummary {
    let mut s = ComparisonSummary::default();
    s.total_changes = changes.len();

    let mut changed_left = BTreeSet::new();
    let mut changed_right = BTreeSet::new();

    for c in changes {
        if let Some(lp) = c.left_page {
            changed_left.insert(lp);
        }
        if let Some(rp) = c.right_page {
            changed_right.insert(rp);
        }

        match c.kind {
            ChangeKind::TextAdded => s.text_added_count += 1,
            ChangeKind::TextDeleted => s.text_deleted_count += 1,
            ChangeKind::TextModified | ChangeKind::LineParagraphChanged => s.text_modified_count += 1,
            ChangeKind::TextMoved => s.text_moved_count += 1,
            ChangeKind::FormattingChanged | ChangeKind::LayoutShifted => s.formatting_changes_count += 1,
            ChangeKind::VisualRegionDiff => s.visual_changes_count += 1,
            ChangeKind::ImageAdded
            | ChangeKind::ImageDeleted
            | ChangeKind::ImageModified
            | ChangeKind::ImageMoved
            | ChangeKind::VectorChanged
            | ChangeKind::TableChanged => s.image_graphic_changes_count += 1,
            ChangeKind::AnnotationAdded
            | ChangeKind::AnnotationDeleted
            | ChangeKind::AnnotationModified => s.annotation_changes_count += 1,
            ChangeKind::FormFieldAdded
            | ChangeKind::FormFieldDeleted
            | ChangeKind::FormFieldModified => s.form_changes_count += 1,
            ChangeKind::HeaderFooterChanged => s.header_footer_changes_count += 1,
            ChangeKind::BackgroundChanged => s.background_changes_count += 1,
            ChangeKind::PageAdded | ChangeKind::PageDeleted | ChangeKind::PageMoved | ChangeKind::PageGeometryChanged => {}
        }
    }

    for a in alignments {
        match a.status {
            PageMatchStatus::Added => s.pages_added_count += 1,
            PageMatchStatus::Deleted => s.pages_deleted_count += 1,
            PageMatchStatus::Moved => s.pages_moved_count += 1,
            PageMatchStatus::Modified => s.pages_modified_count += 1,
            PageMatchStatus::Identical => s.pages_identical_count += 1,
        }
    }

    s.changed_left_pages = changed_left.into_iter().collect();
    s.changed_right_pages = changed_right.into_iter().collect();
    s
}
