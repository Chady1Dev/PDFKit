use crate::commands::compare::types::{BoundingBox, RasterCompareRequest, RasterCompareResult};
use crate::error::PdfError;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

/// Compares two PNG-encoded page renders and returns merged visual difference bounding boxes
/// in PDF point coordinates along with an optional difference heatmap PNG.
pub fn compare_page_pngs(req: &RasterCompareRequest) -> Result<RasterCompareResult, PdfError> {
    let img_l = image::load_from_memory(&req.left_png)
        .map_err(|e| PdfError::Image(format!("Sol sayfa görseli çözümlenemedi: {}", e)))?
        .to_rgba8();
    let img_r = image::load_from_memory(&req.right_png)
        .map_err(|e| PdfError::Image(format!("Sağ sayfa görseli çözümlenemedi: {}", e)))?
        .to_rgba8();

    Ok(compare_rgba_images(
        req.left_page,
        req.right_page,
        &img_l,
        &img_r,
        req.left_dims_pt,
        req.right_dims_pt,
        req.tolerance,
        req.ignore_antialiasing,
        &req.exclude_rects_pt,
        true,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn compare_rgba_images(
    left_page: u32,
    right_page: u32,
    img_l: &RgbaImage,
    img_r: &RgbaImage,
    left_dims_pt: [f64; 2],
    right_dims_pt: [f64; 2],
    tolerance: u8,
    ignore_antialiasing: bool,
    exclude_rects_pt: &[BoundingBox],
    build_heatmap: bool,
) -> RasterCompareResult {
    let wl = img_l.width();
    let hl = img_l.height();
    let wr = img_r.width();
    let hr = img_r.height();

    // Check geometry compatibility (page dimensions & aspect ratio)
    let lw_pt = left_dims_pt[0].max(1.0);
    let lh_pt = left_dims_pt[1].max(1.0);
    let rw_pt = right_dims_pt[0].max(1.0);
    let rh_pt = right_dims_pt[1].max(1.0);

    let w_rel_diff = (lw_pt - rw_pt).abs() / lw_pt.max(rw_pt);
    let h_rel_diff = (lh_pt - rh_pt).abs() / lh_pt.max(rh_pt);
    let geometry_compatible = w_rel_diff <= 0.025 && h_rel_diff <= 0.025;
    let geometry_warning = if !geometry_compatible {
        Some(format!(
            "Sayfa boyutları farklı ({:.0}x{:.0} pt vs {:.0}x{:.0} pt). Yanlış hizalamayı önlemek için ölçekleme yapılmadan ortak kesişim alanı karşılaştırıldı.",
            lw_pt, lh_pt, rw_pt, rh_pt
        ))
    } else {
        None
    };

    let cmp_w = wl.min(wr);
    let cmp_h = hl.min(hr);
    if cmp_w == 0 || cmp_h == 0 {
        return RasterCompareResult {
            left_page,
            right_page,
            diff_ratio: 0.0,
            changed_pixel_count: 0,
            total_pixel_count: 0,
            geometry_compatible,
            geometry_warning,
            regions_pt: Vec::new(),
            heatmap_png: None,
        };
    }

    let scale_x_pt = lw_pt / (wl as f64);
    let scale_y_pt = lh_pt / (hl as f64);

    let cell_size: u32 = 8;
    let grid_cols = ((cmp_w + cell_size - 1) / cell_size) as usize;
    let grid_rows = ((cmp_h + cell_size - 1) / cell_size) as usize;
    let mut grid_counts = vec![0u16; grid_cols * grid_rows];

    let mut heatmap = if build_heatmap {
        Some(RgbaImage::new(cmp_w, cmp_h))
    } else {
        None
    };

    let tol_i32 = tolerance as i32;
    let mut changed_pixel_count: usize = 0;
    let total_pixel_count = (cmp_w as usize) * (cmp_h as usize);

    for y in 0..cmp_h {
        for x in 0..cmp_w {
            let p1 = img_l.get_pixel(x, y);
            let p2 = img_r.get_pixel(x, y);

            let dr = (p1[0] as i32 - p2[0] as i32).abs();
            let dg = (p1[1] as i32 - p2[1] as i32).abs();
            let db = (p1[2] as i32 - p2[2] as i32).abs();
            let da = (p1[3] as i32 - p2[3] as i32).abs();
            let max_delta = dr.max(dg).max(db).max(da);

            if max_delta <= tol_i32 {
                continue;
            }

            if ignore_antialiasing && is_antialiased_edge_jitter(img_l, img_r, x, y, cmp_w, cmp_h, tol_i32) {
                continue;
            }

            changed_pixel_count += 1;
            let gx = (x / cell_size) as usize;
            let gy = (y / cell_size) as usize;
            grid_counts[gy * grid_cols + gx] = grid_counts[gy * grid_cols + gx].saturating_add(1);

            if let Some(ref mut hm) = heatmap {
                // Thermal / Magenta-Orange heatmap intensity based on max_delta
                let intensity = ((max_delta as f32 / 255.0).clamp(0.25, 1.0) * 230.0) as u8;
                let r = 255;
                let g = if max_delta > 140 { 30 } else { 140 };
                let b = if max_delta > 140 { 110 } else { 20 };
                hm.put_pixel(x, y, Rgba([r, g, b, intensity]));
            }
        }
    }

    // Extract active grid cells (require at least 6 changed pixels and genuine cell mean/ink change)
    let mut raw_boxes: Vec<BoundingBox> = Vec::new();
    for gy in 0..grid_rows {
        for gx in 0..grid_cols {
            let count = grid_counts[gy * grid_cols + gx];
            if count >= 6 {
                let start_x = gx as u32 * cell_size;
                let start_y = gy as u32 * cell_size;
                if ignore_antialiasing
                    && !has_genuine_cell_visual_change(img_l, img_r, start_x, start_y, cell_size, cmp_w, cmp_h)
                {
                    continue;
                }
                let px_x = start_x as f64;
                let px_y = start_y as f64;
                let px_w = cell_size as f64;
                let px_h = cell_size as f64;
                raw_boxes.push(BoundingBox::new(
                    px_x * scale_x_pt,
                    px_y * scale_y_pt,
                    px_w * scale_x_pt,
                    px_h * scale_y_pt,
                ));
            }
        }
    }

    // Merge nearby boxes iteratively (within 10pt margin)
    let merged_boxes = merge_bounding_boxes(raw_boxes, 10.0);

    // Filter out boxes already covered by text diff exclusion rects (in Auto mode)
    let filtered_boxes: Vec<BoundingBox> = merged_boxes
        .into_iter()
        .filter(|b| {
            !exclude_rects_pt
                .iter()
                .any(|ex| b.overlap_ratio(ex) >= 0.45 || b.intersects_or_near(ex, 2.0))
        })
        .collect();

    let heatmap_png = if changed_pixel_count > 0 {
        if let Some(hm) = heatmap {
            let mut buf = Cursor::new(Vec::new());
            if hm.write_to(&mut buf, ImageFormat::Png).is_ok() {
                Some(buf.into_inner())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    RasterCompareResult {
        left_page,
        right_page,
        diff_ratio: if total_pixel_count > 0 {
            changed_pixel_count as f64 / total_pixel_count as f64
        } else {
            0.0
        },
        changed_pixel_count,
        total_pixel_count,
        geometry_compatible,
        geometry_warning,
        regions_pt: filtered_boxes,
        heatmap_png,
    }
}

/// Checks if a pixel difference at `(x, y)` is caused by 1-pixel anti-aliasing / sub-pixel shift
/// along a high-contrast edge rather than a genuine visual change.
fn is_antialiased_edge_jitter(
    img_l: &RgbaImage,
    img_r: &RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    tol: i32,
) -> bool {
    if x == 0 || y == 0 || x + 1 >= w || y + 1 >= h {
        return false;
    }

    let c_l = img_l.get_pixel(x, y);
    let c_r = img_r.get_pixel(x, y);

    // Check if c_l matches any 3x3 neighbor in img_r AND c_r matches any 3x3 neighbor in img_l,
    // while the 3x3 neighborhood has a high internal gradient (it's an edge, not a solid fill change!).
    let mut l_matches_r_neighbor = false;
    let mut r_matches_l_neighbor = false;
    let mut max_grad_l = 0i32;

    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = (x as i32 + dx) as u32;
            let ny = (y as i32 + dy) as u32;
            let nl = img_l.get_pixel(nx, ny);
            let nr = img_r.get_pixel(nx, ny);

            let grad = (c_l[0] as i32 - nl[0] as i32)
                .abs()
                .max((c_l[1] as i32 - nl[1] as i32).abs())
                .max((c_l[2] as i32 - nl[2] as i32).abs());
            if grad > max_grad_l {
                max_grad_l = grad;
            }

            let dist_l_nr = (c_l[0] as i32 - nr[0] as i32)
                .abs()
                .max((c_l[1] as i32 - nr[1] as i32).abs())
                .max((c_l[2] as i32 - nr[2] as i32).abs());
            if dist_l_nr <= tol {
                l_matches_r_neighbor = true;
            }

            let dist_r_nl = (c_r[0] as i32 - nl[0] as i32)
                .abs()
                .max((c_r[1] as i32 - nl[1] as i32).abs())
                .max((c_r[2] as i32 - nl[2] as i32).abs());
            if dist_r_nl <= tol {
                r_matches_l_neighbor = true;
            }
        }
    }

    max_grad_l > 65 && l_matches_r_neighbor && r_matches_l_neighbor
}

fn has_genuine_cell_visual_change(
    img_l: &RgbaImage,
    img_r: &RgbaImage,
    start_x: u32,
    start_y: u32,
    cell_size: u32,
    w: u32,
    h: u32,
) -> bool {
    let end_x = w.min(start_x + cell_size);
    let end_y = h.min(start_y + cell_size);
    let mut sum_l = [0i64; 3];
    let mut sum_r = [0i64; 3];
    let mut min_lum_l = 255i32;
    let mut max_lum_l = 0i32;
    let mut min_lum_r = 255i32;
    let mut max_lum_r = 0i32;
    let mut count = 0i64;

    for y in start_y..end_y {
        for x in start_x..end_x {
            let pl = img_l.get_pixel(x, y);
            let pr = img_r.get_pixel(x, y);
            sum_l[0] += pl[0] as i64;
            sum_l[1] += pl[1] as i64;
            sum_l[2] += pl[2] as i64;
            sum_r[0] += pr[0] as i64;
            sum_r[1] += pr[1] as i64;
            sum_r[2] += pr[2] as i64;

            let lum_l = ((pl[0] as i32) * 77 + (pl[1] as i32) * 150 + (pl[2] as i32) * 29) >> 8;
            let lum_r = ((pr[0] as i32) * 77 + (pr[1] as i32) * 150 + (pr[2] as i32) * 29) >> 8;
            min_lum_l = min_lum_l.min(lum_l);
            max_lum_l = max_lum_l.max(lum_l);
            min_lum_r = min_lum_r.min(lum_r);
            max_lum_r = max_lum_r.max(lum_r);
            count += 1;
        }
    }

    if count == 0 {
        return false;
    }

    let mean_delta = (sum_l[0] - sum_r[0])
        .abs()
        .max((sum_l[1] - sum_r[1]).abs())
        .max((sum_l[2] - sum_r[2]).abs()) as f64
        / (count as f64);
    let range_lum_l = max_lum_l - min_lum_l;
    let range_lum_r = max_lum_r - min_lum_r;
    let one_side_flat_and_other_has_content =
        (range_lum_l <= 18 && range_lum_r >= 75) || (range_lum_r <= 18 && range_lum_l >= 75);
    let background_covered = (max_lum_l - max_lum_r).abs() >= 48;

    mean_delta >= 28.0
        || (mean_delta >= 14.0 && (one_side_flat_and_other_has_content || background_covered))
}

pub fn merge_bounding_boxes(mut boxes: Vec<BoundingBox>, margin: f64) -> Vec<BoundingBox> {
    if boxes.len() <= 1 {
        return boxes;
    }

    loop {
        let mut merged_any = false;
        let mut next: Vec<BoundingBox> = Vec::with_capacity(boxes.len());
        let mut used = vec![false; boxes.len()];

        for i in 0..boxes.len() {
            if used[i] {
                continue;
            }
            let mut acc = boxes[i];
            used[i] = true;
            for j in (i + 1)..boxes.len() {
                if !used[j] && acc.intersects_or_near(&boxes[j], margin) {
                    acc = acc.union(&boxes[j]);
                    used[j] = true;
                    merged_any = true;
                }
            }
            next.push(acc);
        }

        boxes = next;
        if !merged_any {
            break;
        }
    }

    boxes
}
