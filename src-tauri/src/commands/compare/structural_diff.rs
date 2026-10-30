use crate::commands::compare::extractor::{ExtractedFormField, ExtractedPage};
use crate::commands::compare::matcher::page_geometry_differs;
use crate::commands::compare::text_diff::truncate_preview;
use crate::commands::compare::types::{
    BoundingBox, ChangeCategory, ChangeKind, ChangeLayer, CompareMode, CompareOptions,
    DiffChangeItem,
};
use std::collections::{HashMap, HashSet};

/// Compares page-level structural objects: geometry, images, vectors/tables, background, and annotations.
pub fn compare_page_structure(
    left: &ExtractedPage,
    right: &ExtractedPage,
    options: &CompareOptions,
    change_counter: &mut usize,
) -> Vec<DiffChangeItem> {
    if options.mode == CompareMode::TextOnly {
        return Vec::new();
    }

    let mut changes = Vec::new();

    // 1. Page Geometry (Size, Rotation, CropBox)
    if page_geometry_differs(left, right) {
        let mut details = Vec::new();
        if (left.width_pt - right.width_pt).abs() > 1.5
            || (left.height_pt - right.height_pt).abs() > 1.5
        {
            details.push(format!(
                "Boyut: {:.0}x{:.0} pt → {:.0}x{:.0} pt",
                left.width_pt, left.height_pt, right.width_pt, right.height_pt
            ));
        }
        if left.rotation != right.rotation {
            details.push(format!("Döndürme: {}° → {}°", left.rotation, right.rotation));
        }
        if left.crop_box != right.crop_box {
            details.push("CropBox (Kırpma Kutusu) değişti".to_string());
        }

        *change_counter += 1;
        changes.push(DiffChangeItem {
            id: format!("geom_{}_{}_{}", left.page_number, right.page_number, *change_counter),
            layer: ChangeLayer::Structure,
            category: ChangeCategory::Pages,
            kind: ChangeKind::PageGeometryChanged,
            left_page: Some(left.page_number),
            right_page: Some(right.page_number),
            left_rect: Some(BoundingBox::new(0.0, 0.0, left.width_pt, left.height_pt)),
            right_rect: Some(BoundingBox::new(0.0, 0.0, right.width_pt, right.height_pt)),
            title: format!("Sayfa Geometrisi Değişti ({})", details.join(", ")),
            detail: format!(
                "Sayfa {} / {} geometrik özelliklerinde fark tespit edildi: {}.",
                left.page_number,
                right.page_number,
                details.join("; ")
            ),
            before_text: Some(format!(
                "{:.0}x{:.0}pt, {}°",
                left.width_pt, left.height_pt, left.rotation
            )),
            after_text: Some(format!(
                "{:.0}x{:.0}pt, {}°",
                right.width_pt, right.height_pt, right.rotation
            )),
            is_heuristic: false,
            confidence: 1.0,
        });
    }

    // When one page was flattened into a full-page raster (e.g., redaction flattening),
    // original vectors and images are baked into the page raster; let pixel-level visual diff
    // isolate only the actual edited/redacted regions instead of reporting whole-page object disappearance.
    let one_side_flattened =
        (left.is_scanned_candidate && left.words.is_empty() && !right.words.is_empty())
            || (right.is_scanned_candidate && right.words.is_empty() && !left.words.is_empty());

    if !one_side_flattened {
        // 2. Embedded Images (XObjects)
        compare_page_images(left, right, change_counter, &mut changes);

        // 3. Vector Shapes, Background Fills & Table Grids
        compare_page_vectors(left, right, change_counter, &mut changes);
    }

    // 4. Page Annotations (/Annots)
    compare_page_annotations(left, right, change_counter, &mut changes);

    changes
}

fn compare_page_images(
    left: &ExtractedPage,
    right: &ExtractedPage,
    change_counter: &mut usize,
    out: &mut Vec<DiffChangeItem>,
) {
    // Filter out synthetic transparent annotation overlay containers (ImOvl*) added by PDFKit editor
    // so they are not falsely matched against unrelated document images; their exact strokes are detected visually.
    let left_imgs: Vec<_> = left
        .images
        .iter()
        .filter(|img| !img.name.starts_with("ImOvl"))
        .collect();
    let right_imgs: Vec<_> = right
        .images
        .iter()
        .filter(|img| !img.name.starts_with("ImOvl"))
        .collect();

    let mut matched_l = vec![false; left_imgs.len()];
    let mut matched_r = vec![false; right_imgs.len()];

    // Pass 1: Match by identical content_hash
    for (li, l_img) in left_imgs.iter().enumerate() {
        for (ri, r_img) in right_imgs.iter().enumerate() {
            if matched_r[ri] {
                continue;
            }
            if l_img.content_hash == r_img.content_hash {
                matched_l[li] = true;
                matched_r[ri] = true;

                let dx = (l_img.bbox.x - r_img.bbox.x).abs();
                let dy = (l_img.bbox.y - r_img.bbox.y).abs();
                let dw = (l_img.bbox.width - r_img.bbox.width).abs();
                let dh = (l_img.bbox.height - r_img.bbox.height).abs();

                if dx > 3.0 || dy > 3.0 || dw > 3.0 || dh > 3.0 {
                    *change_counter += 1;
                    out.push(DiffChangeItem {
                        id: format!("img_move_{}_{}_{}", left.page_number, right.page_number, *change_counter),
                        layer: ChangeLayer::Visual,
                        category: ChangeCategory::ImagesGraphics,
                        kind: ChangeKind::ImageMoved,
                        left_page: Some(left.page_number),
                        right_page: Some(right.page_number),
                        left_rect: Some(l_img.bbox),
                        right_rect: Some(r_img.bbox),
                        title: format!("Görsel Konumu/Boyutu Değişti ({})", l_img.name),
                        detail: format!(
                            "Görsel ({}x{} px) sayfa üzerinde ({:.0},{:.0}) konumundan ({:.0},{:.0}) konumuna taşındı veya yeniden boyutlandırıldı.",
                            l_img.pixel_width, l_img.pixel_height, l_img.bbox.x, l_img.bbox.y, r_img.bbox.x, r_img.bbox.y
                        ),
                        before_text: Some(format!("({:.0}, {:.0}) {:.0}x{:.0}pt", l_img.bbox.x, l_img.bbox.y, l_img.bbox.width, l_img.bbox.height)),
                        after_text: Some(format!("({:.0}, {:.0}) {:.0}x{:.0}pt", r_img.bbox.x, r_img.bbox.y, r_img.bbox.width, r_img.bbox.height)),
                        is_heuristic: false,
                        confidence: 1.0,
                    });
                }
                break;
            }
        }
    }

    // Pass 2: Match remaining images by overlapping bounding box with similar placement -> ImageModified
    for (li, l_img) in left_imgs.iter().enumerate() {
        if matched_l[li] {
            continue;
        }
        for (ri, r_img) in right_imgs.iter().enumerate() {
            if matched_r[ri] {
                continue;
            }
            let max_area = l_img.bbox.area().max(r_img.bbox.area()).max(1.0);
            let min_area = l_img.bbox.area().min(r_img.bbox.area());
            let similar_scale = min_area / max_area >= 0.25;
            if similar_scale
                && (l_img.bbox.overlap_ratio(&r_img.bbox) >= 0.45
                    || ((l_img.bbox.x - r_img.bbox.x).abs() <= 15.0
                        && (l_img.bbox.y - r_img.bbox.y).abs() <= 15.0))
            {
                matched_l[li] = true;
                matched_r[ri] = true;
                *change_counter += 1;

                let is_bg = l_img.is_background && r_img.is_background && !left.is_scanned_candidate;
                out.push(DiffChangeItem {
                    id: format!("img_mod_{}_{}_{}", left.page_number, right.page_number, *change_counter),
                    layer: ChangeLayer::Visual,
                    category: if is_bg {
                        ChangeCategory::Background
                    } else {
                        ChangeCategory::ImagesGraphics
                    },
                    kind: if is_bg {
                        ChangeKind::BackgroundChanged
                    } else {
                        ChangeKind::ImageModified
                    },
                    left_page: Some(left.page_number),
                    right_page: Some(right.page_number),
                    left_rect: Some(l_img.bbox),
                    right_rect: Some(r_img.bbox),
                    title: if left.is_scanned_candidate || right.is_scanned_candidate {
                        "Taranmış Sayfa Görsel Farkı".to_string()
                    } else if is_bg {
                        "Arka Plan Görseli Değiştirildi".to_string()
                    } else {
                        format!("Görsel İçeriği Değiştirildi ({})", l_img.name)
                    },
                    detail: format!(
                        "Aynı bölgedeki görsel içeriği değişti ({}x{} px → {}x{} px).",
                        l_img.pixel_width, l_img.pixel_height, r_img.pixel_width, r_img.pixel_height
                    ),
                    before_text: Some(format!("{} ({}x{} px)", l_img.name, l_img.pixel_width, l_img.pixel_height)),
                    after_text: Some(format!("{} ({}x{} px)", r_img.name, r_img.pixel_width, r_img.pixel_height)),
                    is_heuristic: is_bg,
                    confidence: 1.0,
                });
                break;
            }
        }
    }

    // Pass 3: Unmatched left images -> ImageDeleted
    for (li, l_img) in left_imgs.iter().enumerate() {
        if !matched_l[li] {
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("img_del_{}_{}", left.page_number, *change_counter),
                layer: ChangeLayer::Visual,
                category: ChangeCategory::ImagesGraphics,
                kind: ChangeKind::ImageDeleted,
                left_page: Some(left.page_number),
                right_page: Some(right.page_number),
                left_rect: Some(l_img.bbox),
                right_rect: None,
                title: format!("Görsel Silindi ({})", l_img.name),
                detail: format!(
                    "Referans belgedeki {}x{} px boyutlu görsel kaldırıldı.",
                    l_img.pixel_width, l_img.pixel_height
                ),
                before_text: Some(format!("{} ({}x{} px)", l_img.name, l_img.pixel_width, l_img.pixel_height)),
                after_text: None,
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }

    // Pass 4: Unmatched right images -> ImageAdded
    for (ri, r_img) in right_imgs.iter().enumerate() {
        if !matched_r[ri] {
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("img_add_{}_{}", right.page_number, *change_counter),
                layer: ChangeLayer::Visual,
                category: ChangeCategory::ImagesGraphics,
                kind: ChangeKind::ImageAdded,
                left_page: Some(left.page_number),
                right_page: Some(right.page_number),
                left_rect: None,
                right_rect: Some(r_img.bbox),
                title: format!("Yeni Görsel Eklendi ({})", r_img.name),
                detail: format!(
                    "Yeni belgeye {}x{} px boyutlu görsel eklendi.",
                    r_img.pixel_width, r_img.pixel_height
                ),
                before_text: None,
                after_text: Some(format!("{} ({}x{} px)", r_img.name, r_img.pixel_width, r_img.pixel_height)),
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }
}

fn compare_page_vectors(
    left: &ExtractedPage,
    right: &ExtractedPage,
    change_counter: &mut usize,
    out: &mut Vec<DiffChangeItem>,
) {
    // 1. Compare background vector fills
    let bg_l: Vec<_> = left.vectors.iter().filter(|v| v.is_background).collect();
    let bg_r: Vec<_> = right.vectors.iter().filter(|v| v.is_background).collect();
    if bg_l.len() != bg_r.len()
        || bg_l
            .iter()
            .zip(bg_r.iter())
            .any(|(a, b)| a.fill_rgb != b.fill_rgb)
    {
        let l_color = bg_l
            .first()
            .and_then(|v| v.fill_rgb)
            .map(|c| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
        let r_color = bg_r
            .first()
            .and_then(|v| v.fill_rgb)
            .map(|c| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));

        *change_counter += 1;
        out.push(DiffChangeItem {
            id: format!("bg_vec_{}_{}_{}", left.page_number, right.page_number, *change_counter),
            layer: ChangeLayer::Visual,
            category: ChangeCategory::Background,
            kind: ChangeKind::BackgroundChanged,
            left_page: Some(left.page_number),
            right_page: Some(right.page_number),
            left_rect: bg_l.first().map(|v| v.bbox),
            right_rect: bg_r.first().map(|v| v.bbox),
            title: "Sayfa Arka Plan Dolgusu Değişti".to_string(),
            detail: format!(
                "Sayfa arka plan vektör dolgusu değişti ({} → {}).",
                l_color.as_deref().unwrap_or("Yok"),
                r_color.as_deref().unwrap_or("Yok")
            ),
            before_text: l_color,
            after_text: r_color,
            is_heuristic: true,
            confidence: 0.9,
        });
    }

    // 2. Compare foreground vector shapes & lines
    let fg_l: Vec<_> = left.vectors.iter().filter(|v| !v.is_background).collect();
    let fg_r: Vec<_> = right.vectors.iter().filter(|v| !v.is_background).collect();

    if fg_l.is_empty() && fg_r.is_empty() {
        return;
    }

    let mut matched_l = vec![false; fg_l.len()];
    let mut matched_r = vec![false; fg_r.len()];

    for (li, lv) in fg_l.iter().enumerate() {
        for (ri, rv) in fg_r.iter().enumerate() {
            if matched_r[ri] {
                continue;
            }
            let same_pos = (lv.bbox.x - rv.bbox.x).abs() <= 2.5
                && (lv.bbox.y - rv.bbox.y).abs() <= 2.5
                && (lv.bbox.width - rv.bbox.width).abs() <= 2.5
                && (lv.bbox.height - rv.bbox.height).abs() <= 2.5;
            if same_pos
                && lv.kind == rv.kind
                && lv.fill_rgb == rv.fill_rgb
                && lv.stroke_rgb == rv.stroke_rgb
                && (lv.line_width - rv.line_width).abs() <= 0.3
            {
                matched_l[li] = true;
                matched_r[ri] = true;
                break;
            }
        }
    }

    let unmatched_l: Vec<_> = fg_l
        .iter()
        .enumerate()
        .filter(|&(i, _)| !matched_l[i])
        .map(|(_, &v)| v)
        .collect();
    let unmatched_r: Vec<_> = fg_r
        .iter()
        .enumerate()
        .filter(|&(i, _)| !matched_r[i])
        .map(|(_, &v)| v)
        .collect();

    if !unmatched_l.is_empty() || !unmatched_r.is_empty() {
        let l_rect = unmatched_l
            .iter()
            .map(|v| v.bbox)
            .reduce(|a, b| a.union(&b));
        let r_rect = unmatched_r
            .iter()
            .map(|v| v.bbox)
            .reduce(|a, b| a.union(&b));

        let is_table = left.has_table_grid || right.has_table_grid;
        *change_counter += 1;
        out.push(DiffChangeItem {
            id: format!("vec_{}_{}_{}", left.page_number, right.page_number, *change_counter),
            layer: ChangeLayer::Visual,
            category: ChangeCategory::ImagesGraphics,
            kind: if is_table {
                ChangeKind::TableChanged
            } else {
                ChangeKind::VectorChanged
            },
            left_page: Some(left.page_number),
            right_page: Some(right.page_number),
            left_rect: l_rect,
            right_rect: r_rect,
            title: if is_table {
                "Tablo / Çizgi Izgarası Değişikliği".to_string()
            } else {
                format!(
                    "Vektörel Çizim / Şekil Değişikliği ({} → {} öğe)",
                    fg_l.len(),
                    fg_r.len()
                )
            },
            detail: format!(
                "Sayfadaki vektörel şekil, çizgi veya renk özelliklerinde değişiklik tespit edildi (Eski: {} şekil, Yeni: {} şekil).",
                fg_l.len(),
                fg_r.len()
            ),
            before_text: Some(format!("{} vektör öğesi", fg_l.len())),
            after_text: Some(format!("{} vektör öğesi", fg_r.len())),
            is_heuristic: false,
            confidence: 0.95,
        });
    }
}

fn compare_page_annotations(
    left: &ExtractedPage,
    right: &ExtractedPage,
    change_counter: &mut usize,
    out: &mut Vec<DiffChangeItem>,
) {
    let mut matched_l = vec![false; left.annotations.len()];
    let mut matched_r = vec![false; right.annotations.len()];

    // Match identical annotations first
    for (li, la) in left.annotations.iter().enumerate() {
        for (ri, ra) in right.annotations.iter().enumerate() {
            if matched_r[ri] {
                continue;
            }
            let pos_close = (la.bbox.x - ra.bbox.x).abs() <= 4.0
                && (la.bbox.y - ra.bbox.y).abs() <= 4.0;
            if la.subtype == ra.subtype
                && la.contents == ra.contents
                && la.color_rgb == ra.color_rgb
                && pos_close
            {
                matched_l[li] = true;
                matched_r[ri] = true;
                break;
            }
        }
    }

    // Match modified annotations at same position
    for (li, la) in left.annotations.iter().enumerate() {
        if matched_l[li] {
            continue;
        }
        for (ri, ra) in right.annotations.iter().enumerate() {
            if matched_r[ri] {
                continue;
            }
            if la.subtype == ra.subtype && la.bbox.intersects_or_near(&ra.bbox, 10.0) {
                matched_l[li] = true;
                matched_r[ri] = true;
                *change_counter += 1;
                out.push(DiffChangeItem {
                    id: format!("annot_mod_{}_{}_{}", left.page_number, right.page_number, *change_counter),
                    layer: ChangeLayer::Structure,
                    category: ChangeCategory::Annotations,
                    kind: ChangeKind::AnnotationModified,
                    left_page: Some(left.page_number),
                    right_page: Some(right.page_number),
                    left_rect: Some(la.bbox),
                    right_rect: Some(ra.bbox),
                    title: format!("Anotasyon Değiştirildi ({})", la.subtype),
                    detail: format!(
                        "{} anotasyonunun içeriği veya görünümü değiştirildi.",
                        la.subtype
                    ),
                    before_text: Some(truncate_preview(&la.contents, 80)),
                    after_text: Some(truncate_preview(&ra.contents, 80)),
                    is_heuristic: false,
                    confidence: 1.0,
                });
                break;
            }
        }
    }

    for (li, la) in left.annotations.iter().enumerate() {
        if !matched_l[li] {
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("annot_del_{}_{}", left.page_number, *change_counter),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::Annotations,
                kind: ChangeKind::AnnotationDeleted,
                left_page: Some(left.page_number),
                right_page: Some(right.page_number),
                left_rect: Some(la.bbox),
                right_rect: None,
                title: format!("Anotasyon Silindi ({})", la.subtype),
                detail: format!(
                    "Referans belgedeki {} anotasyonu ({}) kaldırıldı.",
                    la.subtype,
                    if la.contents.is_empty() { "boş" } else { &la.contents }
                ),
                before_text: Some(la.contents.clone()),
                after_text: None,
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }

    for (ri, ra) in right.annotations.iter().enumerate() {
        if !matched_r[ri] {
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("annot_add_{}_{}", right.page_number, *change_counter),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::Annotations,
                kind: ChangeKind::AnnotationAdded,
                left_page: Some(left.page_number),
                right_page: Some(right.page_number),
                left_rect: None,
                right_rect: Some(ra.bbox),
                title: format!("Yeni Anotasyon Eklendi ({})", ra.subtype),
                detail: format!(
                    "Yeni belgeye {} anotasyonu ({}) eklendi.",
                    ra.subtype,
                    if ra.contents.is_empty() { "boş" } else { &ra.contents }
                ),
                before_text: None,
                after_text: Some(ra.contents.clone()),
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }
}

/// Compares document-wide AcroForm fields between the two PDFs.
pub fn compare_document_forms(
    left_fields: &[ExtractedFormField],
    right_fields: &[ExtractedFormField],
    options: &CompareOptions,
    change_counter: &mut usize,
) -> Vec<DiffChangeItem> {
    if options.mode == CompareMode::TextOnly || options.mode == CompareMode::VisualOnly {
        return Vec::new();
    }

    let mut out = Vec::new();
    let map_r: HashMap<&str, &ExtractedFormField> =
        right_fields.iter().map(|f| (f.name.as_str(), f)).collect();

    let mut seen: HashSet<&str> = HashSet::new();

    for lf in left_fields {
        seen.insert(lf.name.as_str());
        if let Some(rf) = map_r.get(lf.name.as_str()) {
            if lf.value != rf.value
                || lf.field_type != rf.field_type
                || lf.read_only != rf.read_only
            {
                *change_counter += 1;
                out.push(DiffChangeItem {
                    id: format!("form_mod_{}", *change_counter),
                    layer: ChangeLayer::Structure,
                    category: ChangeCategory::Forms,
                    kind: ChangeKind::FormFieldModified,
                    left_page: Some(lf.page.max(1)),
                    right_page: Some(rf.page.max(1)),
                    left_rect: lf.bbox,
                    right_rect: rf.bbox,
                    title: format!("Form Alanı Değişti: \"{}\"", lf.name),
                    detail: format!(
                        "\"{}\" ({}) form alanının değeri değişti: \"{}\" → \"{}\".",
                        lf.name, lf.field_type, lf.value, rf.value
                    ),
                    before_text: Some(lf.value.clone()),
                    after_text: Some(rf.value.clone()),
                    is_heuristic: false,
                    confidence: 1.0,
                });
            }
        } else {
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("form_del_{}", *change_counter),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::Forms,
                kind: ChangeKind::FormFieldDeleted,
                left_page: Some(lf.page.max(1)),
                right_page: None,
                left_rect: lf.bbox,
                right_rect: None,
                title: format!("Form Alanı Silindi: \"{}\"", lf.name),
                detail: format!(
                    "Referans belgedeki \"{}\" ({}) form alanı kaldırıldı.",
                    lf.name, lf.field_type
                ),
                before_text: Some(lf.value.clone()),
                after_text: None,
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }

    for rf in right_fields {
        if !seen.contains(rf.name.as_str()) {
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("form_add_{}", *change_counter),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::Forms,
                kind: ChangeKind::FormFieldAdded,
                left_page: None,
                right_page: Some(rf.page.max(1)),
                left_rect: None,
                right_rect: rf.bbox,
                title: format!("Yeni Form Alanı Eklendi: \"{}\"", rf.name),
                detail: format!(
                    "Yeni belgeye \"{}\" ({}) form alanı eklendi (Değer: \"{}\").",
                    rf.name, rf.field_type, rf.value
                ),
                before_text: None,
                after_text: Some(rf.value.clone()),
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }

    out
}
