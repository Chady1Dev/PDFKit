use crate::commands::compare::extractor::{ExtractedPage, ExtractedWord};
use crate::commands::compare::types::{
    BoundingBox, ChangeCategory, ChangeKind, ChangeLayer, CompareMode, CompareOptions,
    DiffChangeItem, NormalizationOptions,
};

#[derive(Debug, Clone)]
struct NormalizedToken {
    raw_text: String,
    norm_text: String,
    bbox: BoundingBox,
    font_name: String,
    font_size: f64,
    color_rgb: [u8; 3],
    line_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffOp {
    Equal(usize, usize),
    Delete(usize),
    Insert(usize),
}

/// Runs text, formatting, and header/footer text comparison for a matched pair of pages.
pub fn compare_page_text(
    left: &ExtractedPage,
    right: &ExtractedPage,
    options: &CompareOptions,
    change_counter: &mut usize,
) -> Vec<DiffChangeItem> {
    let mut changes = Vec::new();

    // When one page was flattened into a full-page raster image (e.g. after physical redaction),
    // its selectable text stream is intentionally empty while the visual text remains in the raster.
    // Do not emit false-positive whole-page text deletion clutter; visual diff handles this pair.
    let one_side_flattened = (left.is_scanned_candidate && left.words.is_empty() && !right.words.is_empty())
        || (right.is_scanned_candidate && right.words.is_empty() && !left.words.is_empty());
    if one_side_flattened {
        return changes;
    }

    // 1. Compare Header / Footer lines separately so they are categorized under HeaderFooter
    if options.mode != CompareMode::VisualOnly {
        compare_header_footer_zones(left, right, &options.normalization, change_counter, &mut changes);
    }

    // 2. Prepare body tokens (excluding header/footer words already handled)
    let left_tokens = build_normalized_tokens(left, false, &options.normalization);
    let right_tokens = build_normalized_tokens(right, false, &options.normalization);

    if left_tokens.is_empty() && right_tokens.is_empty() {
        return changes;
    }

    let ops = compute_token_diff(&left_tokens, &right_tokens);

    // 3. Extract Text Diffs (Added, Deleted, Modified, Moved, Line/Paragraph break changes)
    if matches!(
        options.mode,
        CompareMode::Auto | CompareMode::TextOnly | CompareMode::ContentStructure
    ) {
        let mut raw_text_changes = Vec::new();
        let mut idx = 0;
        while idx < ops.len() {
            match ops[idx] {
                DiffOp::Equal(_, _) => {
                    idx += 1;
                }
                DiffOp::Delete(_) | DiffOp::Insert(_) => {
                    let mut del_indices = Vec::new();
                    let mut ins_indices = Vec::new();
                    while idx < ops.len() {
                        match ops[idx] {
                            DiffOp::Delete(li) => {
                                del_indices.push(li);
                                idx += 1;
                            }
                            DiffOp::Insert(ri) => {
                                ins_indices.push(ri);
                                idx += 1;
                            }
                            DiffOp::Equal(_, _) => break,
                        }
                    }

                    emit_hunk_changes(
                        left.page_number,
                        right.page_number,
                        &left_tokens,
                        &right_tokens,
                        &del_indices,
                        &ins_indices,
                        change_counter,
                        &mut raw_text_changes,
                    );
                }
            }
        }

        // Detect moved text blocks among deleted + added items
        reclassify_moved_text_blocks(&mut raw_text_changes);

        // If ignore_whitespace is FALSE, check if line breaks changed among equal tokens
        if !options.normalization.ignore_whitespace {
            detect_line_break_changes(
                left.page_number,
                right.page_number,
                &left_tokens,
                &right_tokens,
                &ops,
                change_counter,
                &mut raw_text_changes,
            );
        }

        changes.extend(raw_text_changes);
    }

    // 4. Extract Formatting / Style / Position Changes on Equal tokens (when not in TextOnly or VisualOnly mode)
    if matches!(
        options.mode,
        CompareMode::Auto | CompareMode::ContentStructure
    ) {
        detect_formatting_and_layout_changes(
            left.page_number,
            right.page_number,
            &left_tokens,
            &right_tokens,
            &ops,
            change_counter,
            &mut changes,
        );
    }

    changes
}

fn build_normalized_tokens(
    page: &ExtractedPage,
    header_footer_only: bool,
    norm: &NormalizationOptions,
) -> Vec<NormalizedToken> {
    let mut raw_words: Vec<(ExtractedWord, usize)> = Vec::new();
    for (line_idx, line) in page.lines.iter().enumerate() {
        let is_hf = line.in_header_zone || line.in_footer_zone;
        if is_hf != header_footer_only {
            continue;
        }
        for w in &line.words {
            raw_words.push((w.clone(), line_idx));
        }
    }

    // Handle end-of-line hyphenation joining if enabled
    let mut joined_words: Vec<(ExtractedWord, usize)> = Vec::with_capacity(raw_words.len());
    let mut i = 0;
    while i < raw_words.len() {
        let (mut curr_w, curr_line) = raw_words[i].clone();
        if norm.normalize_hyphenation
            && i + 1 < raw_words.len()
            && raw_words[i + 1].1 > curr_line
            && is_hyphenated_line_end(&curr_w.text)
        {
            let (next_w, _) = &raw_words[i + 1];
            let stem = curr_w
                .text
                .trim_end_matches('-')
                .trim_end_matches('\u{00AD}');
            curr_w.text = format!("{}{}", stem, next_w.text);
            curr_w.bbox = curr_w.bbox.union(&next_w.bbox);
            joined_words.push((curr_w, curr_line));
            i += 2;
            continue;
        }
        joined_words.push((curr_w, curr_line));
        i += 1;
    }

    let mut tokens = Vec::with_capacity(joined_words.len());
    for (w, line_idx) in joined_words {
        let norm_text = normalize_token_str(&w.text, norm);
        if norm_text.is_empty() {
            continue;
        }
        tokens.push(NormalizedToken {
            raw_text: w.text,
            norm_text,
            bbox: w.bbox,
            font_name: w.font_name,
            font_size: w.font_size,
            color_rgb: w.color_rgb,
            line_index: line_idx,
        });
    }

    tokens
}

fn is_hyphenated_line_end(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 2 {
        return false;
    }
    let last = chars[chars.len() - 1];
    let prev = chars[chars.len() - 2];
    (last == '-' || last == '\u{00AD}') && prev.is_alphabetic()
}

pub fn normalize_token_str(input: &str, norm: &NormalizationOptions) -> String {
    let mut s = input.to_string();

    if norm.unicode_normalization {
        s = apply_turkish_safe_nfc(&s);
    }

    if norm.normalize_ligatures {
        s = expand_ligatures(&s);
    }

    if norm.ignore_whitespace {
        s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    }

    if norm.ignore_case {
        s = turkish_lowercase(&s);
    }

    s
}

/// Composes combining diacritics into precomposed NFC characters while strictly preserving
/// Turkish dotted `İ` and dotless `ı`.
fn apply_turkish_safe_nfc(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if i + 1 < chars.len() {
            let next = chars[i + 1];
            let composed = match (c, next) {
                ('I', '\u{0307}') | ('i', '\u{0307}') => Some('İ'),
                ('C', '\u{0327}') => Some('Ç'),
                ('c', '\u{0327}') => Some('ç'),
                ('G', '\u{0306}') => Some('Ğ'),
                ('g', '\u{0306}') => Some('ğ'),
                ('O', '\u{0308}') => Some('Ö'),
                ('o', '\u{0308}') => Some('ö'),
                ('S', '\u{0327}') => Some('Ş'),
                ('s', '\u{0327}') => Some('ş'),
                ('U', '\u{0308}') => Some('Ü'),
                ('u', '\u{0308}') => Some('ü'),
                _ => None,
            };
            if let Some(comp) = composed {
                out.push(comp);
                i += 2;
                continue;
            }
        }
        if c != '\u{00AD}' {
            out.push(c);
        }
        i += 1;
    }
    out
}

fn expand_ligatures(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' | '\u{FB06}' => out.push_str("st"),
            '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push('-'),
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201C}' | '\u{201D}' => out.push('"'),
            '\u{00A0}' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// Converts string to lowercase using Turkish casing rules (`İ -> i`, `I -> ı`).
pub fn turkish_lowercase(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            'İ' => out.push('i'),
            'I' => out.push('ı'),
            'Ş' => out.push('ş'),
            'Ğ' => out.push('ğ'),
            'Ç' => out.push('ç'),
            'Ö' => out.push('ö'),
            'Ü' => out.push('ü'),
            _ => {
                for lc in c.to_lowercase() {
                    if lc != '\u{0307}' {
                        out.push(lc);
                    }
                }
            }
        }
    }
    out
}

/// Computes word-level diff operations using LCS with prefix/suffix trimming and windowed alignment.
fn compute_token_diff(left: &[NormalizedToken], right: &[NormalizedToken]) -> Vec<DiffOp> {
    let n = left.len();
    let m = right.len();

    // Trim common prefix
    let mut prefix = 0;
    while prefix < n && prefix < m && left[prefix].norm_text == right[prefix].norm_text {
        prefix += 1;
    }

    // Trim common suffix
    let mut suffix = 0;
    while suffix < (n - prefix)
        && suffix < (m - prefix)
        && left[n - 1 - suffix].norm_text == right[m - 1 - suffix].norm_text
    {
        suffix += 1;
    }

    let mut ops = Vec::with_capacity(n + m);
    for i in 0..prefix {
        ops.push(DiffOp::Equal(i, i));
    }

    let mid_l = &left[prefix..n - suffix];
    let mid_r = &right[prefix..m - suffix];

    if !mid_l.is_empty() || !mid_r.is_empty() {
        let mid_ops = lcs_diff_slice(mid_l, mid_r, prefix, prefix);
        ops.extend(mid_ops);
    }

    for k in 0..suffix {
        ops.push(DiffOp::Equal(n - suffix + k, m - suffix + k));
    }

    ops
}

fn lcs_diff_slice(
    left: &[NormalizedToken],
    right: &[NormalizedToken],
    offset_l: usize,
    offset_r: usize,
) -> Vec<DiffOp> {
    let n = left.len();
    let m = right.len();

    if n == 0 {
        return (0..m).map(|j| DiffOp::Insert(offset_r + j)).collect();
    }
    if m == 0 {
        return (0..n).map(|i| DiffOp::Delete(offset_l + i)).collect();
    }

    // If slice is very large (> 600 tokens on each side), anchor on unique common tokens first
    if n * m > 360_000 {
        if let Some((split_l, split_r)) = find_unique_anchor(left, right) {
            let mut out = lcs_diff_slice(&left[..split_l], &right[..split_r], offset_l, offset_r);
            out.push(DiffOp::Equal(offset_l + split_l, offset_r + split_r));
            out.extend(lcs_diff_slice(
                &left[split_l + 1..],
                &right[split_r + 1..],
                offset_l + split_l + 1,
                offset_r + split_r + 1,
            ));
            return out;
        }
    }

    let cols = m + 1;
    let mut dp = vec![0u32; (n + 1) * cols];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * cols + j] = if left[i].norm_text == right[j].norm_text {
                1 + dp[(i + 1) * cols + (j + 1)]
            } else {
                dp[(i + 1) * cols + j].max(dp[i * cols + (j + 1)])
            };
        }
    }

    let mut ops = Vec::with_capacity(n + m);
    let mut i = 0;
    let mut j = 0;
    while i < n && j < m {
        if left[i].norm_text == right[j].norm_text {
            ops.push(DiffOp::Equal(offset_l + i, offset_r + j));
            i += 1;
            j += 1;
        } else if dp[(i + 1) * cols + j] >= dp[i * cols + (j + 1)] {
            ops.push(DiffOp::Delete(offset_l + i));
            i += 1;
        } else {
            ops.push(DiffOp::Insert(offset_r + j));
            j += 1;
        }
    }
    while i < n {
        ops.push(DiffOp::Delete(offset_l + i));
        i += 1;
    }
    while j < m {
        ops.push(DiffOp::Insert(offset_r + j));
        j += 1;
    }

    ops
}

fn find_unique_anchor(left: &[NormalizedToken], right: &[NormalizedToken]) -> Option<(usize, usize)> {
    use std::collections::HashMap;
    let mut count_l: HashMap<&str, (usize, usize)> = HashMap::new();
    for (i, t) in left.iter().enumerate() {
        let entry = count_l.entry(t.norm_text.as_str()).or_insert((0, i));
        entry.0 += 1;
    }
    let mut count_r: HashMap<&str, (usize, usize)> = HashMap::new();
    for (j, t) in right.iter().enumerate() {
        let entry = count_r.entry(t.norm_text.as_str()).or_insert((0, j));
        entry.0 += 1;
    }

    let mid_l = left.len() / 2;
    let mut best: Option<(usize, usize, usize)> = None;
    for (word, &(cl, il)) in &count_l {
        if cl == 1 {
            if let Some(&(cr, jr)) = count_r.get(word) {
                if cr == 1 {
                    let dist = il.abs_diff(mid_l);
                    if best.map_or(true, |b| dist < b.2) {
                        best = Some((il, jr, dist));
                    }
                }
            }
        }
    }
    best.map(|(il, jr, _)| (il, jr))
}

#[allow(clippy::too_many_arguments)]
fn emit_hunk_changes(
    left_page: u32,
    right_page: u32,
    left_tokens: &[NormalizedToken],
    right_tokens: &[NormalizedToken],
    del_indices: &[usize],
    ins_indices: &[usize],
    change_counter: &mut usize,
    out: &mut Vec<DiffChangeItem>,
) {
    if del_indices.is_empty() && ins_indices.is_empty() {
        return;
    }

    if !del_indices.is_empty() && ins_indices.is_empty() {
        // Group deleted tokens by line so multi-line deletions have tight per-line or block rects
        let del_text = join_tokens(left_tokens, del_indices);
        let del_rect = union_token_rects(left_tokens, del_indices);
        *change_counter += 1;
        out.push(DiffChangeItem {
            id: format!("txt_del_{}_{}", left_page, *change_counter),
            layer: ChangeLayer::Text,
            category: ChangeCategory::Text,
            kind: ChangeKind::TextDeleted,
            left_page: Some(left_page),
            right_page: Some(right_page),
            left_rect: del_rect,
            right_rect: None,
            title: format!("Metin Silindi: \"{}\"", truncate_preview(&del_text, 42)),
            detail: format!("Referans belgedeki \"{}\" ifadesi kaldırıldı.", truncate_preview(&del_text, 120)),
            before_text: Some(del_text),
            after_text: None,
            is_heuristic: false,
            confidence: 1.0,
        });
    } else if del_indices.is_empty() && !ins_indices.is_empty() {
        let ins_text = join_tokens(right_tokens, ins_indices);
        let ins_rect = union_token_rects(right_tokens, ins_indices);
        *change_counter += 1;
        out.push(DiffChangeItem {
            id: format!("txt_add_{}_{}", right_page, *change_counter),
            layer: ChangeLayer::Text,
            category: ChangeCategory::Text,
            kind: ChangeKind::TextAdded,
            left_page: Some(left_page),
            right_page: Some(right_page),
            left_rect: None,
            right_rect: ins_rect,
            title: format!("Metin Eklendi: \"{}\"", truncate_preview(&ins_text, 42)),
            detail: format!("Yeni belgeye \"{}\" ifadesi eklendi.", truncate_preview(&ins_text, 120)),
            before_text: None,
            after_text: Some(ins_text),
            is_heuristic: false,
            confidence: 1.0,
        });
    } else {
        // Both deletions and insertions at this hunk -> TextModified
        let del_text = join_tokens(left_tokens, del_indices);
        let ins_text = join_tokens(right_tokens, ins_indices);
        let del_rect = union_token_rects(left_tokens, del_indices);
        let ins_rect = union_token_rects(right_tokens, ins_indices);
        *change_counter += 1;
        out.push(DiffChangeItem {
            id: format!("txt_mod_{}_{}_{}", left_page, right_page, *change_counter),
            layer: ChangeLayer::Text,
            category: ChangeCategory::Text,
            kind: ChangeKind::TextModified,
            left_page: Some(left_page),
            right_page: Some(right_page),
            left_rect: del_rect,
            right_rect: ins_rect,
            title: format!(
                "Metin Değiştirildi: \"{}\" → \"{}\"",
                truncate_preview(&del_text, 24),
                truncate_preview(&ins_text, 24)
            ),
            detail: format!(
                "\"{}\" ifadesi \"{}\" olarak değiştirildi.",
                truncate_preview(&del_text, 80),
                truncate_preview(&ins_text, 80)
            ),
            before_text: Some(del_text),
            after_text: Some(ins_text),
            is_heuristic: false,
            confidence: 1.0,
        });
    }
}

fn reclassify_moved_text_blocks(changes: &mut Vec<DiffChangeItem>) {
    // Look for a TextDeleted item and a TextAdded item with identical or near-identical text
    // (at least 2 words or >= 12 chars) and merge them into a single TextMoved item.
    let mut paired_remove_indices: Vec<usize> = Vec::new();
    let mut moves_to_add: Vec<DiffChangeItem> = Vec::new();

    for i in 0..changes.len() {
        if changes[i].kind != ChangeKind::TextDeleted || paired_remove_indices.contains(&i) {
            continue;
        }
        let Some(ref del_str) = changes[i].before_text else {
            continue;
        };
        if del_str.split_whitespace().count() < 2 && del_str.chars().count() < 12 {
            continue;
        }
        let del_norm = del_str.to_lowercase();

        for j in 0..changes.len() {
            if i == j || changes[j].kind != ChangeKind::TextAdded || paired_remove_indices.contains(&j) {
                continue;
            }
            let Some(ref add_str) = changes[j].after_text else {
                continue;
            };
            if add_str.to_lowercase() == del_norm {
                paired_remove_indices.push(i);
                paired_remove_indices.push(j);
                moves_to_add.push(DiffChangeItem {
                    id: format!("txt_move_{}_{}", i, j),
                    layer: ChangeLayer::Text,
                    category: ChangeCategory::Text,
                    kind: ChangeKind::TextMoved,
                    left_page: changes[i].left_page,
                    right_page: changes[j].right_page,
                    left_rect: changes[i].left_rect,
                    right_rect: changes[j].right_rect,
                    title: format!("Metin Bloğu Taşındı: \"{}\"", truncate_preview(del_str, 38)),
                    detail: format!(
                        "\"{}\" metin bloğunun sayfa içindeki konumu değiştirildi.",
                        truncate_preview(del_str, 100)
                    ),
                    before_text: Some(del_str.clone()),
                    after_text: Some(add_str.clone()),
                    is_heuristic: false,
                    confidence: 0.95,
                });
                break;
            }
        }
    }

    if !paired_remove_indices.is_empty() {
        paired_remove_indices.sort_unstable();
        for &idx in paired_remove_indices.iter().rev() {
            changes.remove(idx);
        }
        changes.extend(moves_to_add);
    }
}

fn detect_line_break_changes(
    left_page: u32,
    right_page: u32,
    left_tokens: &[NormalizedToken],
    right_tokens: &[NormalizedToken],
    ops: &[DiffOp],
    change_counter: &mut usize,
    out: &mut Vec<DiffChangeItem>,
) {
    let equal_pairs: Vec<(usize, usize)> = ops
        .iter()
        .filter_map(|op| match *op {
            DiffOp::Equal(li, ri) => Some((li, ri)),
            _ => None,
        })
        .collect();

    for w in equal_pairs.windows(2) {
        let (l1, r1) = w[0];
        let (l2, r2) = w[1];
        if l2 == l1 + 1 && r2 == r1 + 1 {
            let line_jump_l = left_tokens[l2].line_index != left_tokens[l1].line_index;
            let line_jump_r = right_tokens[r2].line_index != right_tokens[r1].line_index;
            if line_jump_l != line_jump_r {
                *change_counter += 1;
                let pair_str = format!("{} {}", left_tokens[l1].raw_text, left_tokens[l2].raw_text);
                out.push(DiffChangeItem {
                    id: format!("txt_linebreak_{}_{}", left_page, *change_counter),
                    layer: ChangeLayer::Text,
                    category: ChangeCategory::Text,
                    kind: ChangeKind::LineParagraphChanged,
                    left_page: Some(left_page),
                    right_page: Some(right_page),
                    left_rect: Some(left_tokens[l1].bbox.union(&left_tokens[l2].bbox)),
                    right_rect: Some(right_tokens[r1].bbox.union(&right_tokens[r2].bbox)),
                    title: format!("Satır Sonu Değişikliği: \"{}\"", truncate_preview(&pair_str, 36)),
                    detail: format!(
                        "\"{}\" kelimeleri arasındaki satır/paragraf kırılımı değişti.",
                        pair_str
                    ),
                    before_text: Some(if line_jump_l {
                        format!("{}\n{}", left_tokens[l1].raw_text, left_tokens[l2].raw_text)
                    } else {
                        pair_str.clone()
                    }),
                    after_text: Some(if line_jump_r {
                        format!("{}\n{}", right_tokens[r1].raw_text, right_tokens[r2].raw_text)
                    } else {
                        pair_str
                    }),
                    is_heuristic: false,
                    confidence: 0.9,
                });
            }
        }
    }
}

fn detect_formatting_and_layout_changes(
    left_page: u32,
    right_page: u32,
    left_tokens: &[NormalizedToken],
    right_tokens: &[NormalizedToken],
    ops: &[DiffOp],
    change_counter: &mut usize,
    out: &mut Vec<DiffChangeItem>,
) {
    let mut idx = 0;
    while idx < ops.len() {
        let DiffOp::Equal(li, ri) = ops[idx] else {
            idx += 1;
            continue;
        };

        let lt = &left_tokens[li];
        let rt = &right_tokens[ri];
        let style_diff = describe_style_diff(lt, rt);

        if let Some(reason) = style_diff {
            let mut l_rect = lt.bbox;
            let mut r_rect = rt.bbox;
            let mut words_vec = vec![lt.raw_text.clone()];
            let before_style = format!("{} {:.1}pt #{:02X}{:02X}{:02X}", lt.font_name, lt.font_size, lt.color_rgb[0], lt.color_rgb[1], lt.color_rgb[2]);
            let after_style = format!("{} {:.1}pt #{:02X}{:02X}{:02X}", rt.font_name, rt.font_size, rt.color_rgb[0], rt.color_rgb[1], rt.color_rgb[2]);

            idx += 1;
            while idx < ops.len() {
                if let DiffOp::Equal(nli, nri) = ops[idx] {
                    let nlt = &left_tokens[nli];
                    let nrt = &right_tokens[nri];
                    if describe_style_diff(nlt, nrt).as_deref() == Some(reason.as_str())
                        && nlt.line_index == lt.line_index
                    {
                        l_rect = l_rect.union(&nlt.bbox);
                        r_rect = r_rect.union(&nrt.bbox);
                        words_vec.push(nlt.raw_text.clone());
                        idx += 1;
                        continue;
                    }
                }
                break;
            }

            let sample = words_vec.join(" ");
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("fmt_{}_{}_{}", left_page, right_page, *change_counter),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::Formatting,
                kind: ChangeKind::FormattingChanged,
                left_page: Some(left_page),
                right_page: Some(right_page),
                left_rect: Some(l_rect),
                right_rect: Some(r_rect),
                title: format!(
                    "Biçim Değişikliği ({}): \"{}\"",
                    reason,
                    truncate_preview(&sample, 30)
                ),
                detail: format!(
                    "\"{}\" metninin biçimi değişti ({}): {} → {}.",
                    truncate_preview(&sample, 60),
                    reason,
                    before_style,
                    after_style
                ),
                before_text: Some(before_style),
                after_text: Some(after_style),
                is_heuristic: false,
                confidence: 0.95,
            });
            continue;
        }

        idx += 1;
    }
}

fn is_generic_font_label(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "" | "default" | "sans-serif" | "serif" | "monospace"
    )
}

fn describe_style_diff(lt: &NormalizedToken, rt: &NormalizedToken) -> Option<String> {
    let mut reasons = Vec::new();
    let l_font = crate::commands::compare::extractor::clean_font_name(&lt.font_name);
    let r_font = crate::commands::compare::extractor::clean_font_name(&rt.font_name);
    let both_specific_or_both_generic =
        is_generic_font_label(&l_font) == is_generic_font_label(&r_font);
    if both_specific_or_both_generic
        && !l_font.eq_ignore_ascii_case(&r_font)
        && l_font != "Default"
        && r_font != "Default"
    {
        reasons.push(format!("Font: {} → {}", l_font, r_font));
    }
    if (lt.font_size - rt.font_size).abs() >= 0.8 {
        reasons.push(format!("Boyut: {:.1}pt → {:.1}pt", lt.font_size, rt.font_size));
    }
    let color_dist = ((lt.color_rgb[0] as i32 - rt.color_rgb[0] as i32).pow(2)
        + (lt.color_rgb[1] as i32 - rt.color_rgb[1] as i32).pow(2)
        + (lt.color_rgb[2] as i32 - rt.color_rgb[2] as i32).pow(2)) as f64;
    if color_dist.sqrt() >= 25.0 {
        reasons.push(format!(
            "Renk: #{:02X}{:02X}{:02X} → #{:02X}{:02X}{:02X}",
            lt.color_rgb[0],
            lt.color_rgb[1],
            lt.color_rgb[2],
            rt.color_rgb[0],
            rt.color_rgb[1],
            rt.color_rgb[2]
        ));
    }
    if reasons.is_empty() {
        None
    } else {
        Some(reasons.join(", "))
    }
}

fn compare_header_footer_zones(
    left: &ExtractedPage,
    right: &ExtractedPage,
    norm: &NormalizationOptions,
    change_counter: &mut usize,
    out: &mut Vec<DiffChangeItem>,
) {
    for is_header in [true, false] {
        let l_lines: Vec<_> = left
            .lines
            .iter()
            .filter(|l| if is_header { l.in_header_zone } else { l.in_footer_zone })
            .collect();
        let r_lines: Vec<_> = right
            .lines
            .iter()
            .filter(|l| if is_header { l.in_header_zone } else { l.in_footer_zone })
            .collect();

        if l_lines.is_empty() && r_lines.is_empty() {
            continue;
        }

        let l_text = l_lines
            .iter()
            .map(|l| normalize_token_str(&l.text, norm))
            .collect::<Vec<_>>()
            .join(" ");
        let r_text = r_lines
            .iter()
            .map(|l| normalize_token_str(&l.text, norm))
            .collect::<Vec<_>>()
            .join(" ");

        if l_text != r_text {
            let l_raw = l_lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" ");
            let r_raw = r_lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" ");
            let l_rect = l_lines
                .iter()
                .map(|l| l.bbox)
                .reduce(|a, b| a.union(&b));
            let r_rect = r_lines
                .iter()
                .map(|l| l.bbox)
                .reduce(|a, b| a.union(&b));

            let zone_label = if is_header { "Üstbilgi" } else { "Altbilgi" };
            *change_counter += 1;
            out.push(DiffChangeItem {
                id: format!("hf_{}_{}_{}", left.page_number, right.page_number, *change_counter),
                layer: ChangeLayer::Structure,
                category: ChangeCategory::HeaderFooter,
                kind: ChangeKind::HeaderFooterChanged,
                left_page: Some(left.page_number),
                right_page: Some(right.page_number),
                left_rect: l_rect,
                right_rect: r_rect,
                title: format!("{} Değişikliği", zone_label),
                detail: format!(
                    "{} bölgesinde metin farkı tespit edildi (koordinat ve sayfa tekrarı sezgiseli ile sınıflandırıldı).",
                    zone_label
                ),
                before_text: if l_raw.is_empty() { None } else { Some(l_raw) },
                after_text: if r_raw.is_empty() { None } else { Some(r_raw) },
                is_heuristic: true,
                confidence: 0.85,
            });
        }
    }
}

fn join_tokens(tokens: &[NormalizedToken], indices: &[usize]) -> String {
    indices
        .iter()
        .map(|&i| tokens[i].raw_text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn union_token_rects(tokens: &[NormalizedToken], indices: &[usize]) -> Option<BoundingBox> {
    indices
        .iter()
        .map(|&i| tokens[i].bbox)
        .reduce(|a, b| a.union(&b))
}

pub fn truncate_preview(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}
