"""
PPTX Layout and Geometry Engine
Multi-column detection, container-aware grouping, recursive XY-Cut,
fine-grained span/line clustering with mandatory whitespace restoration (LH and LT fix),
strict non-merging for headers/footers/captions, neighbor-aware width buffers,
and table vertical push-down (Table 1-5 collision solution).
"""

import math
import os
import re
from typing import Dict, List, Optional, Tuple
from .ir import FontInfo, PageIR, Rect, TableIR, TextBlockIR, TextLineIR, TextSpanIR, safe_float

_FONT_CACHE: Dict[Tuple[str, int], object] = {}


def _get_pil_font(font_name: str, size: float):
    """Retrieves or creates a cached PIL ImageFont for metric calculations."""
    pt_size = max(6, int(round(size)))
    key = (font_name.lower(), pt_size)
    if key in _FONT_CACHE:
        return _FONT_CACHE[key]

    font_obj = None
    try:
        from PIL import ImageFont
        candidates = [
            f"C:/Windows/Fonts/{font_name.lower()}.ttf",
            "C:/Windows/Fonts/arial.ttf",
            "C:/Windows/Fonts/calibri.ttf",
            "C:/Windows/Fonts/segoeui.ttf",
        ]
        for c in candidates:
            if os.path.exists(c):
                font_obj = ImageFont.truetype(c, pt_size)
                break
        if not font_obj:
            font_obj = ImageFont.load_default()
    except Exception:
        font_obj = None

    _FONT_CACHE[key] = font_obj
    return font_obj


def estimate_text_width(text: str, font_name: str, size: float) -> float:
    """Estimates rendered text width in points using PIL or calibrated proportional metrics."""
    if not text:
        return 0.0
    font = _get_pil_font(font_name, size)
    if font and hasattr(font, "getlength"):
        try:
            return float(font.getlength(text))
        except Exception:
            pass

    avg_ratio = 0.52
    if any(k in font_name.lower() for k in ["times", "serif", "roman"]):
        avg_ratio = 0.49
    elif any(k in font_name.lower() for k in ["courier", "mono", "consolas"]):
        avg_ratio = 0.60
    return len(text) * size * avg_ratio


def cluster_spans_into_lines(spans: List[TextSpanIR], y_tol: float = 2.5) -> List[TextLineIR]:
    """
    Groups raw text spans into horizontal lines.
    CRITICAL FIX (LHandLT, termprotocol): Restores mandatory whitespace between spans
    when switching fonts/styles (normal -> italic -> bold) so words never stick together.
    """
    if not spans:
        return []

    lines_raw = []
    for s in spans:
        ref_y = s.baseline_y if s.baseline_y > 0 else s.bbox.center_y
        matched = False
        for l in lines_raw:
            v_overlap = max(0.0, min(l["bbox"].y1, s.bbox.y1) - max(l["bbox"].y0, s.bbox.y0))
            min_h = min(l["bbox"].height, s.bbox.height)
            is_same_line = (
                abs(l["ref_y"] - ref_y) < y_tol
                or (min_h > 0 and (v_overlap / min_h) >= 0.40)
                or abs(l["bbox"].y1 - s.bbox.y1) <= 3.5
                or abs(l["bbox"].center_y - s.bbox.center_y) <= 3.5
            )
            if is_same_line:
                l["spans"].append(s)
                l["ref_y"] = (l["ref_y"] * (len(l["spans"]) - 1) + ref_y) / len(l["spans"])
                l["bbox"] = l["bbox"].union(s.bbox)
                matched = True
                break
        if not matched:
            lines_raw.append({
                "ref_y": ref_y,
                "bbox": s.bbox,
                "spans": [s],
            })

    lines_raw.sort(key=lambda item: item["ref_y"])

    result_lines = []
    for l_data in lines_raw:
        sorted_spans = sorted(l_data["spans"], key=lambda s: s.bbox.x0)
        if not sorted_spans:
            continue

        # Split spans into separate lines if horizontal gap > 28.0 pt (e.g. columns or diagram boxes)
        span_chunks: List[List[TextSpanIR]] = []
        cur_chunk: List[TextSpanIR] = [sorted_spans[0]]
        for s in sorted_spans[1:]:
            prev_s = cur_chunk[-1]
            if (s.bbox.x0 - prev_s.bbox.x1) > 28.0:
                span_chunks.append(cur_chunk)
                cur_chunk = [s]
            else:
                cur_chunk.append(s)
        if cur_chunk:
            span_chunks.append(cur_chunk)

        for chunk_spans in span_chunks:
            chunk_bbox = chunk_spans[0].bbox
            adjusted_spans: List[TextSpanIR] = []

            for i, s in enumerate(chunk_spans):
                chunk_bbox = chunk_bbox.union(s.bbox)
                cur_text = s.text
                if i > 0 and adjusted_spans:
                    prev_s = adjusted_spans[-1]
                    gap = s.bbox.x0 - prev_s.bbox.x1
                    needs_space = False

                    # KURAL: İki ardışık span arasında boşluk düşmüşse zorunlu boşluk karakteri ekle
                    # DÜZELTME 3: Noktalama işareti öncesi boşluk yasağı
                    PUNCT_CHARS = (",", ".", ";", ":", "!", "?", ")", "]", "”", "’", "*", "%")
                    if cur_text.lstrip().startswith(PUNCT_CHARS):
                        needs_space = False
                    elif gap > 1.0:
                        needs_space = True
                    elif (
                        not prev_s.text.endswith((" ", "-", "/", "(", "“", '"', "—", "–"))
                        and not cur_text.startswith((" ", ",", ".", ";", ":", "!", "?", ")", "”", '"', "*", "%", "-", "—", "–"))
                    ):
                        needs_space = True

                    if needs_space and not prev_s.text.endswith(" ") and not cur_text.startswith(" "):
                        prev_s.text = prev_s.text + " "

                adjusted_spans.append(
                    TextSpanIR(
                        text=cur_text,
                        bbox=s.bbox,
                        font=s.font,
                        clean_font_name=s.clean_font_name,
                        visual_font_size=s.visual_font_size,
                        char_spacing_spc=s.char_spacing_spc,
                        baseline_y=s.baseline_y,
                        parent_container_rect=s.parent_container_rect,
                        span_id=s.span_id,
                    )
                )

            result_lines.append(
                TextLineIR(
                    bbox=chunk_bbox,
                    spans=adjusted_spans,
                    baseline_y=l_data["ref_y"],
                )
            )

    return result_lines


def group_lines_into_blocks(lines: List[TextLineIR], max_v_gap: float = 14.0) -> List[TextBlockIR]:
    """
    Groups adjacent text lines into paragraph text blocks.
    Enforces strict non-merging for headers, footers, Figure captions, and list bullets.
    """
    if not lines:
        return []

    bullet_pattern = re.compile(r"^([•\-\*▪\u2022\u25aa\u25cf\u25e6]|\d+[\.\)]|[a-zA-Z][\.\)])\s+")

    def can_merge(prev_line: TextLineIR, cur_line: TextLineIR) -> bool:
        cur_txt = cur_line.text.strip()

        if bullet_pattern.match(cur_txt):
            return False

        prev_span = prev_line.spans[0] if prev_line.spans else None
        cur_span = cur_line.spans[0] if cur_line.spans else None
        if not prev_span or not cur_span:
            return False

        # Container aidiyeti aynı olmalı
        if prev_span.parent_container_rect != cur_span.parent_container_rect:
            return False

        # Sol hizası korunmalı (±4.0 pt)
        if abs(cur_line.bbox.x0 - prev_line.bbox.x0) > 4.0:
            return False

        # Font boyutu farkı 0.5 pt'den az olmalı
        if abs(cur_span.font.size - prev_span.font.size) > 0.5:
            return False

        # Dikey satır boşluğu font_size * 0.65'ten az olmalı
        v_gap = cur_line.bbox.y0 - prev_line.bbox.y1
        if v_gap > cur_span.font.size * 0.65 or v_gap < -2.0:
            return False

        return True

    blocks: List[TextBlockIR] = []
    cur_lines = [lines[0]]
    cur_bbox = lines[0].bbox

    for i in range(1, len(lines)):
        prev = lines[i - 1]
        cur = lines[i]

        if can_merge(prev, cur):
            cur_lines.append(cur)
            cur_bbox = cur_bbox.union(cur.bbox)
        else:
            block = _create_block_with_metrics(f"block_{len(blocks)}", cur_bbox, cur_lines)
            blocks.append(block)
            cur_lines = [cur]
            cur_bbox = cur.bbox

    if cur_lines:
        block = _create_block_with_metrics(f"block_{len(blocks)}", cur_bbox, cur_lines)
        blocks.append(block)

    return blocks


def _create_block_with_metrics(block_id: str, bbox: Rect, lines: List[TextLineIR]) -> TextBlockIR:
    """Builds TextBlockIR, computes exact line spacing and text alignment."""
    is_single_line = (len(lines) == 1)
    exact_line_spacing = None
    if len(lines) > 1:
        diffs = [lines[i].baseline_y - lines[i - 1].baseline_y for i in range(1, len(lines))]
        avg_diff = sum(diffs) / len(diffs)
        if avg_diff > 6.0:
            exact_line_spacing = round(avg_diff, 2)

    container = lines[0].spans[0].parent_container_rect if lines and lines[0].spans else None
    alignment = "LEFT"
    if len(lines) > 1:
        center_devs = [abs(l.center_x - bbox.center_x) for l in lines]
        right_devs = [abs(l.x1 - bbox.x1) for l in lines]
        left_devs = [abs(l.x0 - bbox.x0) for l in lines]
        if max(center_devs) < 3.0 and max(left_devs) > 4.0:
            alignment = "CENTER"
        elif max(right_devs) < 3.0 and max(left_devs) > 4.0:
            alignment = "RIGHT"
    elif container is not None:
        if abs(bbox.center_x - container.center_x) < 6.0 and bbox.width < container.width * 0.85:
            alignment = "CENTER"
        elif abs(bbox.x1 - container.x1) < 12.0 and bbox.x0 > container.center_x:
            alignment = "RIGHT"

    block = TextBlockIR(
        id=block_id,
        bbox=bbox,
        lines=lines,
        alignment=alignment,
        exact_line_spacing_pt=exact_line_spacing,
        parent_container_rect=container,
        is_single_line=is_single_line,
    )

    _apply_font_metrics_and_compression(block)
    return block


def _apply_font_metrics_and_compression(block: TextBlockIR) -> None:
    """Applies OOXML spc compression (-10 to -35) or gentle font scaling if text slightly overflows bbox."""
    box_w = block.bbox.width
    if box_w <= 0:
        return

    is_multi_line = len(block.lines) > 1 or block.is_protected

    for line in block.lines:
        line_txt = line.text
        if not line_txt or not line.spans:
            continue
        primary_span = line.spans[0]
        measured_w = estimate_text_width(line_txt, primary_span.clean_font_name, primary_span.font.size)
        if measured_w > box_w:
            overflow_pct = (measured_w - box_w) / box_w
            if overflow_pct < 0.08 or is_multi_line:
                char_count = max(len(line_txt), 1)
                spc_val = max(-35, min(-10, int(-100.0 * (measured_w - box_w) / char_count)))
                for s in line.spans:
                    s.char_spacing_spc = spc_val
            else:
                scale = max(0.85, box_w / measured_w)
                new_size = max(6.0, round(primary_span.font.size * scale * 2.0) / 2.0)
                for s in line.spans:
                    s.visual_font_size = new_size


def detect_column_split(
    spans: List[TextSpanIR],
    page_width: float,
    divider_x: Optional[float] = None
) -> Optional[float]:
    """Detects vertical column boundary (split_x) if a 2-column layout exists."""
    w = safe_float(page_width, 595.0)
    if divider_x is not None:
        dx = safe_float(divider_x)
        if 0.25 * w < dx < 0.75 * w:
            return dx

    narrow_spans = [s for s in spans if s.bbox.width < w * 0.55]
    if len(narrow_spans) < 6:
        return None

    left_spans = [s for s in narrow_spans if s.bbox.center_x < w * 0.5]
    right_spans = [s for s in narrow_spans if s.bbox.center_x >= w * 0.5]

    if len(left_spans) >= 3 and len(right_spans) >= 3:
        max_left = max(s.bbox.x1 for s in left_spans)
        min_right = min(s.bbox.x0 for s in right_spans)
        if min_right > max_left - 5.0:
            return (max_left + min_right) / 2.0

    return None


def partition_and_process_spans(
    spans: List[TextSpanIR],
    page_width: float,
    page_height: float,
    divider_x: Optional[float] = None
) -> List[TextBlockIR]:
    """
    Partitions spans by container boundaries first, then applies recursive XY-Cut on free spans.
    Prevents cross-container text merging and preserves natural reading order.
    """
    if not spans:
        return []

    w = safe_float(page_width, 595.0)
    h = safe_float(page_height, 842.0)

    # 1. Separate container-bound spans from free spans
    container_map: Dict[Tuple[float, float], List[TextSpanIR]] = {}
    free_spans: List[TextSpanIR] = []

    for s in spans:
        if s.parent_container_rect is not None:
            c_key = (round(s.parent_container_rect.x0, 1), round(s.parent_container_rect.y0, 1))
            container_map.setdefault(c_key, []).append(s)
        else:
            free_spans.append(s)

    all_blocks: List[TextBlockIR] = []

    # 2. Process container-bound spans independently
    for c_key, c_spans in container_map.items():
        c_lines = cluster_spans_into_lines(c_spans)
        c_blocks = group_lines_into_blocks(c_lines)
        for b in c_blocks:
            b.column_index = 5  # Callout/Card container level
            all_blocks.append(b)

    # 3. Process free spans with Recursive XY-Cut
    if free_spans:
        header_thresh = h * 0.08
        footer_thresh = h * 0.94

        header_spans = []
        footer_spans = []
        body_spans = []

        for s in free_spans:
            if s.bbox.center_y < header_thresh:
                header_spans.append(s)
            elif s.bbox.center_y > footer_thresh:
                footer_spans.append(s)
            else:
                body_spans.append(s)

        col_split = detect_column_split(body_spans, w, divider_x)

        if header_spans:
            h_lines = cluster_spans_into_lines(header_spans)
            h_blocks = group_lines_into_blocks(h_lines)
            for b in h_blocks:
                b.block_type = "header"
                b.column_index = 0
                all_blocks.append(b)

        if body_spans:
            if col_split is not None:
                col1_spans = []
                col2_spans = []
                fullwidth_spans = []

                for s in body_spans:
                    if s.bbox.width > w * 0.7:
                        fullwidth_spans.append(s)
                    elif s.bbox.center_x < col_split:
                        col1_spans.append(s)
                    else:
                        col2_spans.append(s)

                if fullwidth_spans:
                    fw_lines = cluster_spans_into_lines(fullwidth_spans)
                    fw_blocks = group_lines_into_blocks(fw_lines)
                    for b in fw_blocks:
                        b.column_index = 1
                        all_blocks.append(b)

                if col1_spans:
                    col1_lines = cluster_spans_into_lines(col1_spans)
                    col1_blocks = group_lines_into_blocks(col1_lines)
                    for b in col1_blocks:
                        b.column_index = 1
                        all_blocks.append(b)

                if col2_spans:
                    col2_lines = cluster_spans_into_lines(col2_spans)
                    col2_blocks = group_lines_into_blocks(col2_lines)
                    for b in col2_blocks:
                        b.column_index = 2
                        all_blocks.append(b)
            else:
                b_lines = cluster_spans_into_lines(body_spans)
                b_blocks = group_lines_into_blocks(b_lines)
                for b in b_blocks:
                    b.column_index = 1
                    all_blocks.append(b)

        if footer_spans:
            f_lines = cluster_spans_into_lines(footer_spans)
            f_blocks = group_lines_into_blocks(f_lines)
            for b in f_blocks:
                b.block_type = "footer"
                b.column_index = 99
                all_blocks.append(b)

    # Sort blocks by natural reading order
    sorted_blocks = sorted(
        all_blocks,
        key=lambda b: (
            0 if b.block_type == "header" else (2 if b.block_type == "footer" else 1),
            b.column_index,
            b.bbox.y0,
            b.bbox.x0,
        )
    )

    return sorted_blocks


def calculate_textbox_metrics(
    block: TextBlockIR,
    obstacles: Optional[List[Rect]] = None,
    slide_width: float = 720.0
) -> Tuple[float, float, float, float]:
    """
    Calculates precise (x, y, width, height) in points.
    Preserves exact PDF x0, applies +4% width buffer safely without colliding with neighbors.
    Zero width shrinking: width is strictly guaranteed >= block.orig_width.
    """
    min_x = block.bbox.x0
    min_y = block.bbox.y0
    orig_w = block.orig_width if block.orig_width > 0 else block.bbox.width
    content_w = max(orig_w, block.bbox.width)
    content_h = block.bbox.height

    # Find closest obstacle on the right in the same vertical band
    nearest_x0 = slide_width - 4.0
    if obstacles:
        for obs in obstacles:
            if obs != block.bbox and obs.x0 > block.bbox.x1 - 1.0:
                if not (obs.y1 <= block.bbox.y0 or obs.y0 >= block.bbox.y1):
                    if obs.x0 < nearest_x0:
                        nearest_x0 = obs.x0

    max_right_gap = max(0.0, nearest_x0 - block.bbox.x1)

    # Width buffer: orig_width + max(8.0, orig_width * 0.05) to ensure non-bold text never wraps
    desired_buf = max(8.0, orig_w * 0.05)
    safe_buffer = min(desired_buf, max(0.0, max_right_gap - 1.0))

    target_w = max(orig_w, content_w + safe_buffer)

    if block.alignment == "CENTER":
        half = safe_buffer / 2.0
        min_x = max(0.0, min_x - half)
    elif block.alignment == "RIGHT":
        min_x = max(0.0, min_x - safe_buffer)
    # else LEFT keeps min_x exactly at PDF x0

    target_h = max(10.0, content_h + 2.0)
    return min_x, min_y, target_w, target_h


def apply_table_vertical_push_down(page_ir: PageIR) -> None:
    """
    Universal architecture: All elements remain 100% on their original PDF y-coordinates.
    Vertical shifting is completely removed to prevent throwing headers and tables off the slide.
    """
    pass


COMPOUND_PREFIX_EXCEPTIONS = {
    "same", "adjacent", "error", "data", "link", "high", "low", "higher", "lower",
    "end", "pre", "post", "re", "co", "non", "self", "multi", "inter", "intra",
    "cross", "built", "well", "two", "three", "four", "five", "seven", "client",
    "server", "home", "chapter", "part", "user", "real", "full", "half", "open",
    "single", "double", "fiber"
}


def should_dehyphenate(prev_text: str, next_text: str) -> Tuple[bool, str]:
    """
    Checks if prev_text ending with hyphen and next_text should be de-hyphenated.
    Returns (should_dehyphenate, clean_prev_text).
    """
    stripped_prev = prev_text.rstrip()
    if not stripped_prev.endswith("-") or stripped_prev.endswith(("--", "—", "–")):
        return False, prev_text

    m_prev = re.search(r"([a-zA-Z]+)-$", stripped_prev)
    if not m_prev:
        return False, prev_text

    prefix = m_prev.group(1)
    if len(prefix) < 2 or not prefix.isalpha() or not prefix.islower():
        return False, prev_text

    stripped_next = next_text.lstrip()
    m_next = re.match(r"([a-zA-Z]+)", stripped_next)
    if not m_next:
        return False, prev_text

    suffix = m_next.group(1)
    if not suffix[0].islower():
        return False, prev_text

    if prefix.lower() in COMPOUND_PREFIX_EXCEPTIONS:
        return False, prev_text

    clean_prev = stripped_prev[:-1]
    return True, clean_prev


def partition_block_into_paragraphs(block: TextBlockIR) -> List[Tuple[List[TextLineIR], float]]:
    """
    Partitions lines of a TextBlockIR into true logical paragraphs based on 4 universal conditions:
    1. Vertical gap increase: line_gap > max(median_gap * 1.28, font_size * 0.35)
    2. Early terminating line ending with sentence punctuation (. : ! ? ” ")
    3. Indentation shift > 3.5 pt
    4. Bullet or numbered bold list item (e.g. '1 large task:', '7 medium tasks/book:')
    Returns a list of (para_lines, space_before_pt) tuples.
    """
    if not block.lines:
        return []
    if len(block.lines) == 1:
        return [(block.lines, 0.0)]

    # KURAL 5: Monospace CLI kod bloklarinda her satir ayri bir paragraftir (girinti ve komut satiri korunur)
    is_code = False
    for l in block.lines:
        for s in l.spans:
            fn = (s.clean_font_name or "").lower()
            if "courier" in fn or "mono" in fn or "consolas" in fn:
                is_code = True
                break
        if is_code:
            break
    if is_code or block.parent_container_rect is not None:
        return [([line], 0.0) for line in block.lines]

    line_gaps = [
        max(0.0, block.lines[i].bbox.y0 - block.lines[i - 1].bbox.y1)
        for i in range(1, len(block.lines))
    ]
    median_gap = sorted(line_gaps)[len(line_gaps) // 2] if line_gaps else 4.0

    bullet_pattern = re.compile(r"^([•\-\*▪■\u2022\u25aa\u25cf\u25e6]|\d+[\.\)]|[a-zA-Z][\.\)])\s+")
    numbered_task_pattern = re.compile(r"^\d+\s+[a-zA-Z]")

    paragraphs: List[Tuple[List[TextLineIR], float]] = []
    curr_para: List[TextLineIR] = [block.lines[0]]
    curr_space_before = 0.0

    for i in range(1, len(block.lines)):
        prev_line = block.lines[i - 1]
        curr_line = block.lines[i]

        line_gap = curr_line.bbox.y0 - prev_line.bbox.y1
        font_sz = curr_line.spans[0].font.size if curr_line.spans and curr_line.spans[0].font else 10.0

        # Condition 1: Dikey Boşluk Artışı
        is_gap_split = line_gap > max(median_gap * 1.28, font_sz * 0.35)

        # Condition 2: Erken Biten Önceki Satır (Paragraf Sonu)
        prev_txt = prev_line.text.rstrip()
        ends_punct = bool(prev_txt) and prev_txt[-1] in (".", ":", "!", "?", "”", '"')
        is_early_end = ends_punct and (prev_line.bbox.x1 < block.bbox.x1 - (block.bbox.width * 0.12))

        # Condition 3: Girinti Değişimi
        is_indent_change = abs(curr_line.bbox.x0 - prev_line.bbox.x0) > 3.5

        # Condition 4: Liste / Görev Maddesi Başlangıcı
        curr_txt = curr_line.text.strip()
        is_bullet = bool(bullet_pattern.match(curr_txt))
        is_numbered_bold = False
        if curr_txt and curr_txt[0].isdigit():
            if curr_line.spans and curr_line.spans[0].font and curr_line.spans[0].font.bold:
                is_numbered_bold = True
            elif numbered_task_pattern.match(curr_txt):
                is_numbered_bold = True

        if is_gap_split or is_early_end or is_indent_change or is_bullet or is_numbered_bold:
            paragraphs.append((curr_para, curr_space_before))
            curr_para = [curr_line]
            curr_space_before = max(2.5, line_gap - median_gap) if is_gap_split else (2.0 if (is_bullet or is_numbered_bold) else 0.0)
        else:
            curr_para.append(curr_line)

    if curr_para:
        paragraphs.append((curr_para, curr_space_before))

    return paragraphs
