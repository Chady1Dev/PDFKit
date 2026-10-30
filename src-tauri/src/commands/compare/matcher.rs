use crate::commands::compare::extractor::ExtractedPage;
use crate::commands::compare::types::{
    BoundingBox, ChangeCategory, ChangeKind, ChangeLayer, CompareOptions, DiffChangeItem,
    PageAlignment, PageMatchStatus,
};
use std::collections::{HashMap, HashSet};

/// Computes the optimal page alignment between Left (old/reference) and Right (new/compared) pages,
/// plus structural page-level changes (Added, Deleted, Moved) and uncertainty warnings.
pub fn match_document_pages(
    left_pages: &[ExtractedPage],
    right_pages: &[ExtractedPage],
    options: &CompareOptions,
) -> (Vec<PageAlignment>, Vec<DiffChangeItem>, Vec<String>) {
    let n_left = left_pages.len();
    let n_right = right_pages.len();

    // 1. Build pairwise similarity matrix S[i][j] in 0.0..=1.0
    let mut sim_matrix = vec![vec![0.0f64; n_right]; n_left];
    for (i, lp) in left_pages.iter().enumerate() {
        for (j, rp) in right_pages.iter().enumerate() {
            sim_matrix[i][j] = compute_page_similarity(lp, rp);
        }
    }

    let min_sim = options.min_page_similarity.clamp(0.15, 0.85);

    // 2. Find primary monotonic alignment via weighted dynamic programming (Needleman-Wunsch / LCS)
    // We give a small positional continuity bonus (+0.03) to preserve natural order on ties.
    let mut dp = vec![vec![0.0f64; n_right + 1]; n_left + 1];
    for i in (0..n_left).rev() {
        for j in (0..n_right).rev() {
            let skip_left = dp[i + 1][j];
            let skip_right = dp[i][j + 1];
            let mut best = skip_left.max(skip_right);
            let s = sim_matrix[i][j];
            if s >= min_sim {
                let pos_bonus = if i == j { 0.02 } else { 0.0 };
                let take = s + pos_bonus + dp[i + 1][j + 1];
                if take > best {
                    best = take;
                }
            }
            dp[i][j] = best;
        }
    }

    let mut matched_left_to_right: HashMap<usize, (usize, bool)> = HashMap::new(); // left_idx -> (right_idx, is_moved)
    let mut matched_right: HashSet<usize> = HashSet::new();

    let mut i = 0;
    let mut j = 0;
    while i < n_left && j < n_right {
        let s = sim_matrix[i][j];
        let pos_bonus = if i == j { 0.02 } else { 0.0 };
        let take = if s >= min_sim {
            s + pos_bonus + dp[i + 1][j + 1]
        } else {
            -1.0
        };
        if take >= dp[i + 1][j] - 1e-9 && take >= dp[i][j + 1] - 1e-9 && s >= min_sim {
            matched_left_to_right.insert(i, (j, false));
            matched_right.insert(j);
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }

    // 3. Second pass: check remaining unmatched pages for out-of-order MOVED pages!
    // Collect candidate pairs (left_idx, right_idx, sim) where both are currently unmatched
    // OR where an out-of-order match is dramatically stronger than a weak monotonic match.
    let mut move_candidates: Vec<(usize, usize, f64)> = Vec::new();
    for li in 0..n_left {
        if matched_left_to_right.contains_key(&li) {
            continue;
        }
        for rj in 0..n_right {
            if matched_right.contains(&rj) {
                continue;
            }
            let s = sim_matrix[li][rj];
            if s >= (min_sim + 0.18).min(0.50) {
                move_candidates.push((li, rj, s));
            }
        }
    }
    move_candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    for (li, rj, _s) in move_candidates {
        if !matched_left_to_right.contains_key(&li) && !matched_right.contains(&rj) {
            matched_left_to_right.insert(li, (rj, true));
            matched_right.insert(rj);
        }
    }

    // Also detect if two matched pages crossed order or shifted significantly when reordered
    // (e.g., if DP matched [0->0] and move pass matched [1->2, 2->1])
    let mut alignments: Vec<PageAlignment> = Vec::new();
    let mut page_changes: Vec<DiffChangeItem> = Vec::new();
    let mut uncertainties: Vec<String> = Vec::new();

    // Build unified alignment timeline so every left and right page appears in a logical order
    let mut emitted_right: HashSet<usize> = HashSet::new();

    for li in 0..n_left {
        let lp = &left_pages[li];
        if let Some(&(rj, is_moved_pass)) = matched_left_to_right.get(&li) {
            // Before emitting this matched pair, emit any pure Added right pages before `rj`
            // that are not matched to any left page
            for prev_r in 0..rj {
                if !matched_right.contains(&prev_r) && !emitted_right.contains(&prev_r) {
                    emitted_right.insert(prev_r);
                    let rp = &right_pages[prev_r];
                    alignments.push(PageAlignment {
                        left_page: None,
                        right_page: Some(rp.page_number),
                        status: PageMatchStatus::Added,
                        similarity: 0.0,
                        low_confidence: false,
                        geometry_differs: false,
                        is_scanned: rp.is_scanned_candidate,
                        left_dimensions: None,
                        right_dimensions: Some([rp.width_pt, rp.height_pt]),
                        left_rotation: 0,
                        right_rotation: rp.rotation,
                    });
                    page_changes.push(DiffChangeItem {
                        id: format!("page_add_{}", rp.page_number),
                        layer: ChangeLayer::Structure,
                        category: ChangeCategory::Pages,
                        kind: ChangeKind::PageAdded,
                        left_page: None,
                        right_page: Some(rp.page_number),
                        left_rect: None,
                        right_rect: Some(BoundingBox::new(0.0, 0.0, rp.width_pt, rp.height_pt)),
                        title: format!("Yeni Sayfa Eklendi (Sayfa {})", rp.page_number),
                        detail: format!(
                            "İkinci belgede {}. sayfa yeni olarak eklendi ({:.0}x{:.0} pt).",
                            rp.page_number, rp.width_pt, rp.height_pt
                        ),
                        before_text: None,
                        after_text: Some(format!("Sayfa {}", rp.page_number)),
                        is_heuristic: false,
                        confidence: 1.0,
                    });
                }
            }

            emitted_right.insert(rj);
            let rp = &right_pages[rj];
            let sim = sim_matrix[li][rj];
            let low_conf = sim < 0.52;
            let geom_diff = page_geometry_differs(lp, rp);
            let is_scanned = lp.is_scanned_candidate || rp.is_scanned_candidate;

            // Check if page was reordered relative to other matched pages
            let is_reordered = is_moved_pass || is_out_of_monotonic_order(li, rj, &matched_left_to_right);

            let status = if is_reordered {
                PageMatchStatus::Moved
            } else if (sim - 1.0).abs() < 1e-6 && !geom_diff {
                PageMatchStatus::Identical
            } else {
                PageMatchStatus::Modified
            };

            if low_conf {
                uncertainties.push(format!(
                    "Sol Sayfa {} ile Sağ Sayfa {} arasındaki eşleşme güveni düşük (%{:.0} benzerlik); içerik büyük ölçüde değişmiş olabilir.",
                    lp.page_number,
                    rp.page_number,
                    sim * 100.0
                ));
            }

            if is_reordered {
                page_changes.push(DiffChangeItem {
                    id: format!("page_move_{}_{}", lp.page_number, rp.page_number),
                    layer: ChangeLayer::Structure,
                    category: ChangeCategory::Pages,
                    kind: ChangeKind::PageMoved,
                    left_page: Some(lp.page_number),
                    right_page: Some(rp.page_number),
                    left_rect: Some(BoundingBox::new(0.0, 0.0, lp.width_pt, lp.height_pt)),
                    right_rect: Some(BoundingBox::new(0.0, 0.0, rp.width_pt, rp.height_pt)),
                    title: format!(
                        "Sayfa Taşındı (Sayfa {} → Sayfa {})",
                        lp.page_number, rp.page_number
                    ),
                    detail: format!(
                        "Referans belgedeki {}. sayfa, yeni belgede {}. sıraya taşındı (%{:.0} benzerlik).",
                        lp.page_number,
                        rp.page_number,
                        sim * 100.0
                    ),
                    before_text: Some(format!("Sayfa {}", lp.page_number)),
                    after_text: Some(format!("Sayfa {}", rp.page_number)),
                    is_heuristic: false,
                    confidence: sim,
                });
            }

            alignments.push(PageAlignment {
                left_page: Some(lp.page_number),
                right_page: Some(rp.page_number),
                status,
                similarity: sim,
                low_confidence: low_conf,
                geometry_differs: geom_diff,
                is_scanned,
                left_dimensions: Some([lp.width_pt, lp.height_pt]),
                right_dimensions: Some([rp.width_pt, rp.height_pt]),
                left_rotation: lp.rotation,
                right_rotation: rp.rotation,
            });
        } else {
            // Unmatched left page -> Deleted
            alignments.push(PageAlignment {
                left_page: Some(lp.page_number),
                right_page: None,
                status: PageMatchStatus::Deleted,
                similarity: 0.0,
                low_confidence: false,
                geometry_differs: false,
                is_scanned: lp.is_scanned_candidate,
                left_dimensions: Some([lp.width_pt, lp.height_pt]),
                right_dimensions: None,
                left_rotation: lp.rotation,
                right_rotation: 0,
            });
            page_changes.push(DiffChangeItem {
                id: format!("page_del_{}", lp.page_number),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::Pages,
                kind: ChangeKind::PageDeleted,
                left_page: Some(lp.page_number),
                right_page: None,
                left_rect: Some(BoundingBox::new(0.0, 0.0, lp.width_pt, lp.height_pt)),
                right_rect: None,
                title: format!("Sayfa Silindi (Sayfa {})", lp.page_number),
                detail: format!(
                    "Referans belgedeki {}. sayfa yeni belgede bulunmuyor.",
                    lp.page_number
                ),
                before_text: Some(format!("Sayfa {}", lp.page_number)),
                after_text: None,
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }

    // Emit any remaining unmatched right pages at the end
    for rj in 0..n_right {
        if !emitted_right.contains(&rj) {
            let rp = &right_pages[rj];
            alignments.push(PageAlignment {
                left_page: None,
                right_page: Some(rp.page_number),
                status: PageMatchStatus::Added,
                similarity: 0.0,
                low_confidence: false,
                geometry_differs: false,
                is_scanned: rp.is_scanned_candidate,
                left_dimensions: None,
                right_dimensions: Some([rp.width_pt, rp.height_pt]),
                left_rotation: 0,
                right_rotation: rp.rotation,
            });
            page_changes.push(DiffChangeItem {
                id: format!("page_add_{}", rp.page_number),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::Pages,
                kind: ChangeKind::PageAdded,
                left_page: None,
                right_page: Some(rp.page_number),
                left_rect: None,
                right_rect: Some(BoundingBox::new(0.0, 0.0, rp.width_pt, rp.height_pt)),
                title: format!("Yeni Sayfa Eklendi (Sayfa {})", rp.page_number),
                detail: format!(
                    "İkinci belgede {}. sayfa yeni olarak eklendi ({:.0}x{:.0} pt).",
                    rp.page_number, rp.width_pt, rp.height_pt
                ),
                before_text: None,
                after_text: Some(format!("Sayfa {}", rp.page_number)),
                is_heuristic: false,
                confidence: 1.0,
            });
        }
    }

    (alignments, page_changes, uncertainties)
}

fn is_out_of_monotonic_order(
    li: usize,
    rj: usize,
    matched: &HashMap<usize, (usize, bool)>,
) -> bool {
    for (&other_l, &(other_r, is_moved)) in matched {
        if is_moved || other_l == li {
            continue;
        }
        if (li < other_l && rj > other_r) || (li > other_l && rj < other_r) {
            return true;
        }
    }
    false
}

pub fn page_geometry_differs(lp: &ExtractedPage, rp: &ExtractedPage) -> bool {
    let w_diff = (lp.width_pt - rp.width_pt).abs() > 1.5;
    let h_diff = (lp.height_pt - rp.height_pt).abs() > 1.5;
    let rot_diff = lp.rotation != rp.rotation;
    let crop_diff = match (lp.crop_box, rp.crop_box) {
        (Some(a), Some(b)) => a.iter().zip(b.iter()).any(|(x, y)| (x - y).abs() > 1.5),
        (Some(_), None) | (None, Some(_)) => true,
        (None, None) => false,
    };
    w_diff || h_diff || rot_diff || crop_diff
}

fn compute_page_similarity(lp: &ExtractedPage, rp: &ExtractedPage) -> f64 {
    let has_text_l = !lp.words.is_empty();
    let has_text_r = !rp.words.is_empty();
    let one_flattened =
        (lp.is_scanned_candidate && !has_text_l && has_text_r)
            || (rp.is_scanned_candidate && !has_text_r && has_text_l);

    if one_flattened && !page_geometry_differs(lp, rp) {
        return if lp.page_number == rp.page_number {
            0.82
        } else {
            0.58
        };
    }

    let text_sim = if has_text_l && has_text_r {
        compute_word_similarity(lp, rp)
    } else if !has_text_l && !has_text_r {
        1.0
    } else {
        0.0
    };

    let struct_sim = compute_structural_similarity(lp, rp);

    if has_text_l || has_text_r {
        (text_sim * 0.78 + struct_sim * 0.22).clamp(0.0, 1.0)
    } else {
        struct_sim.clamp(0.0, 1.0)
    }
}

fn compute_word_similarity(lp: &ExtractedPage, rp: &ExtractedPage) -> f64 {
    let mut freq_l: HashMap<String, usize> = HashMap::new();
    let mut freq_r: HashMap<String, usize> = HashMap::new();

    for w in &lp.words {
        if w.in_header_zone || w.in_footer_zone {
            continue;
        }
        let k = w.text.to_lowercase();
        *freq_l.entry(k).or_insert(0) += 1;
    }
    for w in &rp.words {
        if w.in_header_zone || w.in_footer_zone {
            continue;
        }
        let k = w.text.to_lowercase();
        *freq_r.entry(k).or_insert(0) += 1;
    }

    // If all words on either page were in header/footer zones, fall back to all words
    if freq_l.is_empty() || freq_r.is_empty() {
        freq_l.clear();
        freq_r.clear();
        for w in &lp.words {
            *freq_l.entry(w.text.to_lowercase()).or_insert(0) += 1;
        }
        for w in &rp.words {
            *freq_r.entry(w.text.to_lowercase()).or_insert(0) += 1;
        }
    }

    if freq_l.is_empty() && freq_r.is_empty() {
        return 1.0;
    }
    if freq_l.is_empty() || freq_r.is_empty() {
        return 0.0;
    }

    let mut intersection = 0usize;
    let mut total = 0usize;
    let all_keys: HashSet<&String> = freq_l.keys().chain(freq_r.keys()).collect();
    for k in all_keys {
        let c1 = freq_l.get(k).copied().unwrap_or(0);
        let c2 = freq_r.get(k).copied().unwrap_or(0);
        intersection += c1.min(c2);
        total += c1.max(c2);
    }

    if total == 0 {
        1.0
    } else {
        intersection as f64 / total as f64
    }
}

fn compute_structural_similarity(lp: &ExtractedPage, rp: &ExtractedPage) -> f64 {
    // 1. Aspect / Dimension similarity
    let w_ratio = lp.width_pt.min(rp.width_pt) / lp.width_pt.max(rp.width_pt).max(1.0);
    let h_ratio = lp.height_pt.min(rp.height_pt) / lp.height_pt.max(rp.height_pt).max(1.0);
    let geom_sim = (w_ratio + h_ratio) * 0.5;

    // 2. Image hash similarity
    let img_sim = if lp.images.is_empty() && rp.images.is_empty() {
        1.0
    } else if lp.images.is_empty() || rp.images.is_empty() {
        0.2
    } else {
        let hashes_l: HashSet<u64> = lp.images.iter().map(|i| i.content_hash).collect();
        let hashes_r: HashSet<u64> = rp.images.iter().map(|i| i.content_hash).collect();
        let inter = hashes_l.intersection(&hashes_r).count();
        let union = hashes_l.union(&hashes_r).count().max(1);
        0.3 + 0.7 * (inter as f64 / union as f64)
    };

    // 3. Vector count similarity
    let v_l = lp.vectors.len();
    let v_r = rp.vectors.len();
    let vec_sim = if v_l == 0 && v_r == 0 {
        1.0
    } else {
        let min_v = v_l.min(v_r) as f64;
        let max_v = v_l.max(v_r) as f64;
        0.4 + 0.6 * (min_v / max_v)
    };

    geom_sim * 0.4 + img_sim * 0.35 + vec_sim * 0.25
}
