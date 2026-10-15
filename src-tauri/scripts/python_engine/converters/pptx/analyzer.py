"""
PPTX Page Analysis and Intermediate Representation Builder
Extracts tables, callout banners, vector diagrams, images, and text into a structured PageIR model.
Strictly implements:
- MASTER GÖREV: /MCID and /ActualText stream cleaning, preventing ghost alt-texts (Figure 3 'e Quiz...' fix)
- DÜZELTME 1: Universal font bold and italic detection by name parsing + intra-paragraph font consistency
- DÜZELTME 2: Protected text zones, diagram expansion exclusion, boundary clamping, and duplicate suppression immunity
- DÜZELTME 3: Table-first pipeline with rich cell span preservation (TableCellIR.spans)
- DÜZELTME 4: Header chapter tab and left-margin "Key Topic" badge isolation with zero outward bleed
- 1:1 Universal coordinate fidelity, zero vertical shift, and slide boundary clamping
- 288 DPI high-resolution crisp pixmap rendering for diagrams and badges
"""

import re
from typing import Dict, List, Optional, Tuple
import pymupdf
from python_engine.common.progress import clean_xml_text
from .ir import (
    CalloutIR,
    DiagramIR,
    FontInfo,
    ImageIR,
    PageIR,
    Rect,
    ShapeIR,
    TableCellIR,
    TableIR,
    TextBlockIR,
    TextLineIR,
    TextSpanIR,
    safe_color_rgb,
    safe_float,
    safe_int,
    safe_rect,
)
from .layout import (
    cluster_spans_into_lines,
    group_lines_into_blocks,
    partition_and_process_spans,
)

# Document-wide cache for margin badge template matching (DÜZELTME 4)
_GLOBAL_BADGE_CACHE: Dict[Tuple[int, int], bytes] = {}


def normalize_font_name(font_name: str) -> str:
    """Cleans subset prefixes (ABCDEF+) and suffixes (-Bold), maps to standard Office fonts."""
    if not font_name:
        return "Calibri"
    if "+" in font_name:
        font_name = font_name.split("+", 1)[1]
    clean = font_name.replace("-BoldItalic", "").replace("-Bold", "").replace("-Italic", "")
    clean = clean.replace(",BoldItalic", "").replace(",Bold", "").replace(",Italic", "")
    clean = clean.replace("MT", "").strip()

    clean_lower = clean.lower()
    if "arial" in clean_lower:
        return "Arial"
    if "times" in clean_lower or "serif" in clean_lower or "roman" in clean_lower:
        return "Times New Roman"
    if "courier" in clean_lower or "mono" in clean_lower or "consolas" in clean_lower:
        return "Courier New"
    if "segoe" in clean_lower:
        return "Segoe UI"
    if "tahoma" in clean_lower:
        return "Tahoma"
    if "verdana" in clean_lower:
        return "Verdana"
    if "helvetica" in clean_lower:
        return "Arial"
    if "calibri" in clean_lower:
        return "Calibri"
    return clean or "Calibri"


def detect_font_weight_and_style(span: dict) -> Tuple[bool, bool]:
    """
    Universally detects bold and italic styling based on font name parsing
    with fallback to flags. Fixes PyMuPDF StemV=80 false bold bug (DÜZELTME 1).
    Safely handles None attributes.
    """
    raw_font_lower = str(span.get("font") or "").lower()
    clean_lower = raw_font_lower.split("+")[-1]

    bold_keywords = ("bold", "black", "heavy", "demi", "semibold", "extrabold", "ultrabold", "-bd", "medi")
    non_bold_keywords = ("regular", "roman", "light", "thin", "book", "normal", "italic", "-it", "oblique")

    flags = safe_int(span.get("flags"), 0)

    if any(k in clean_lower for k in bold_keywords):
        is_bold = True
    elif any(k in clean_lower for k in non_bold_keywords):
        is_bold = False
    else:
        if clean_lower in ("helvetica", "arial", "times", "timesnewroman", "calibri", "courier"):
            is_bold = False
        else:
            is_bold = bool(flags & 16)

    italic_keywords = ("italic", "oblique", "-it", "inclined", "slanted")
    is_italic = any(k in clean_lower for k in italic_keywords) or bool(flags & 2)

    return is_bold, is_italic


class PageAnalyzer:
    """Analyzes a PyMuPDF PDF page and constructs an independent PageIR representation."""

    def __init__(self, page, page_number: int, scale: float = 1.0):
        self.page = page
        self.page_number = page_number
        self.scale = scale
        try:
            self.origin_x = float(page.rect.x0)
            self.origin_y = float(page.rect.y0)
            self.raw_w = max(1.0, float(page.rect.width))
            self.raw_h = max(1.0, float(page.rect.height))
            self.width = self.raw_w * scale
            self.height = self.raw_h * scale
            self.rotation = safe_int(page.rotation, 0) if hasattr(page, "rotation") else 0
        except Exception:
            self.origin_x = 0.0
            self.origin_y = 0.0
            self.raw_w = 595.0
            self.raw_h = 842.0
            self.width = 595.0
            self.height = 842.0
            self.rotation = 0

        # Neutralize /MCID and /ActualText in page content streams to eliminate ghost alt-text (Figure 3 fix)
        try:
            doc = getattr(self.page, "parent", None)
            if doc:
                for c_xref in self.page.get_contents():
                    try:
                        stream_bytes = doc.xref_stream(c_xref)
                        if stream_bytes and (b"/MCID" in stream_bytes or b"/ActualText" in stream_bytes):
                            cleaned = (
                                stream_bytes
                                .replace(b"/MCID", b"/IGNR")
                                .replace(b"/ActualText", b"/IGNR_AT")
                            )
                            doc.update_stream(c_xref, cleaned)
                    except Exception:
                        pass
        except Exception:
            pass

    def to_rect(self, coords) -> Optional[Rect]:
        return safe_rect(coords, origin_x=self.origin_x, origin_y=self.origin_y, scale=self.scale)

    def to_pdf_clip(self, rect: Rect) -> pymupdf.Rect:
        """Converts slide Rect back to original PyMuPDF page coordinates for pixmap clipping."""
        pdf_x0 = rect.x0 / self.scale + self.origin_x
        pdf_y0 = rect.y0 / self.scale + self.origin_y
        pdf_x1 = rect.x1 / self.scale + self.origin_x
        pdf_y1 = rect.y1 / self.scale + self.origin_y
        return pymupdf.Rect(pdf_x0, pdf_y0, pdf_x1, pdf_y1)

    def analyze(self) -> PageIR:
        page_ir = PageIR(
            page_number=self.page_number,
            width=self.width,
            height=self.height,
            rotation=self.rotation,
        )

        try:
            drawings = self.page.get_drawings()
        except Exception:
            drawings = []

        # =========================================================================
        # ADIM 1: ÖNCE TÜM METİN SPAN'LERİNİ AYIKLA
        # =========================================================================
        try:
            page_dict = self.page.get_text("dict", flags=pymupdf.TEXTFLAGS_DICT & ~pymupdf.TEXT_PRESERVE_IMAGES)
        except Exception:
            try:
                page_dict = self.page.get_text("dict")
            except Exception:
                page_dict = {}

        raw_parsed_spans: List[TextSpanIR] = []
        raw_text_lines: List[Tuple[Rect, str]] = []
        next_span_id = 1
        consumed_span_ids: set[int] = set()

        for b in page_dict.get("blocks", []):
            if b.get("type") != 0 or not b.get("bbox"):
                continue
            b_rect = self.to_rect(b.get("bbox"))
            if not b_rect:
                continue
            # Sayfa dışı metin filtresi (basım kodları vb.)
            if (b_rect.x0 >= self.width - 2.0 or b_rect.x1 <= 2.0 or
                b_rect.y0 >= self.height - 2.0 or b_rect.y1 <= 2.0):
                continue

            for l in b.get("lines", []):
                dir_v = l.get("dir", (1.0, 0.0))
                dir_y = safe_float(dir_v[1]) if dir_v and len(dir_v) >= 2 else 0.0
                if abs(dir_y) > 0.05:  # Rotated text
                    continue
                l_rect = self.to_rect(l.get("bbox"))
                l_text = clean_xml_text("".join(s.get("text", "") for s in l.get("spans", []))).strip()
                if l_rect and l_text:
                    raw_text_lines.append((l_rect, l_text))

                for s in l.get("spans", []):
                    raw_text = clean_xml_text(s.get("text", "")).strip()
                    if not raw_text or not s.get("bbox"):
                        continue
                    s_bbox = self.to_rect(s.get("bbox"))
                    if not s_bbox:
                        continue
                    if (s_bbox.x0 >= self.width - 2.0 or s_bbox.x1 <= 2.0 or
                        s_bbox.y0 >= self.height - 2.0 or s_bbox.y1 <= 2.0):
                        continue
                    raw_sz = s.get("size")
                    size = safe_float(raw_sz, 10.0) * self.scale
                    if size < 3.5:
                        continue

                    # DÜZELTME 1: Evrensel bold & italic
                    is_b, is_i = detect_font_weight_and_style(s)
                    color_int = safe_int(s.get("color"), 0)
                    color_rgb = (
                        (color_int >> 16) & 0xFF,
                        (color_int >> 8) & 0xFF,
                        color_int & 0xFF,
                    )
                    font_name = str(s.get("font") or "Calibri")
                    clean_font = normalize_font_name(font_name)
                    origin = s.get("origin")
                    if origin and len(origin) >= 2:
                        baseline_y = (safe_float(origin[1]) - self.origin_y) * self.scale
                    else:
                        baseline_y = s_bbox.y1 - s_bbox.height * 0.15

                    font_info = FontInfo(name=font_name, size=size, bold=is_b, italic=is_i, color=color_rgb)
                    raw_parsed_spans.append(
                        TextSpanIR(
                            text=raw_text,
                            bbox=s_bbox,
                            font=font_info,
                            clean_font_name=clean_font,
                            visual_font_size=size,
                            baseline_y=baseline_y,
                            span_id=next_span_id,
                        )
                    )
                    next_span_id += 1

        # =========================================================================
        # ADIM 2: TEK SÜTUNLU KAPSAYICI BİRLEŞTİRME (CONTAINER / CALLOUT CONSOLIDATION - KURAL 2)
        # =========================================================================
        confirmed_callouts: List[CalloutIR] = []
        c_strips = []
        for d in drawings:
            r_raw = d.get("rect")
            if not r_raw or not d.get("fill"):
                continue
            r_rect = self.to_rect(r_raw)
            if not r_rect:
                continue
            if r_rect.width >= self.width * 0.45 and 6.0 <= r_rect.height <= 75.0:
                fill_rgb = safe_color_rgb(d.get("fill"))
                if fill_rgb and fill_rgb != (255, 255, 255):
                    c_strips.append({"rect": r_rect, "fill": fill_rgb, "drawing": d})

        c_strips.sort(key=lambda s: s["rect"].y0)
        multi_containers = []
        for s in c_strips:
            matched = False
            for c in multi_containers:
                if c["fill"] == s["fill"]:
                    v_gap = s["rect"].y0 - c["rect"].y1
                    h_overlap = (abs(s["rect"].x0 - c["rect"].x0) <= 6.0 and abs(s["rect"].x1 - c["rect"].x1) <= 6.0)
                    if h_overlap and -1.0 <= v_gap <= 4.0:
                        c["rect"] = c["rect"].union(s["rect"])
                        c["count"] += 1
                        c["drawings"].append(s["drawing"])
                        matched = True
                        break
            if not matched:
                multi_containers.append({"rect": s["rect"], "fill": s["fill"], "count": 1, "drawings": [s["drawing"]]})

        code_containers = [c for c in multi_containers if c["count"] >= 3]

        accent_drawings = []
        for d in drawings:
            r_raw = d.get("rect")
            if not r_raw or not d.get("fill"):
                continue
            r_rect = self.to_rect(r_raw)
            if not r_rect:
                continue
            if r_rect.width <= 8.0 and r_rect.height >= 5.0:
                acc_rgb = safe_color_rgb(d.get("fill"))
                if acc_rgb:
                    accent_drawings.append({"rect": r_rect, "fill": acc_rgb, "drawing": d})

        for ci, c in enumerate(code_containers):
            c_rect = c["rect"]
            acc_rect: Optional[Rect] = None
            acc_color: Optional[Tuple[int, int, int]] = None
            for ab in accent_drawings:
                ar = ab["rect"]
                if (c_rect.x0 - 8.0 <= ar.x0 <= c_rect.x0 + 4.0) and (ar.y0 >= c_rect.y0 - 5.0 and ar.y1 <= c_rect.y1 + 5.0):
                    if acc_rect is None:
                        acc_rect = ar
                        acc_color = ab["fill"]
                    else:
                        acc_rect = acc_rect.union(ar)
                    c["drawings"].append(ab["drawing"])

            if acc_rect:
                c_rect = c_rect.union(acc_rect)

            confirmed_callouts.append(
                CalloutIR(
                    id=f"callout_code_{ci}",
                    bbox=c_rect,
                    bg_color=c["fill"],
                    border_color=None,
                    border_width=0.0,
                    is_rounded=False,
                    accent_bar_rect=acc_rect,
                    accent_bar_color=acc_color,
                )
            )

        if confirmed_callouts:
            cleaned_drawings = []
            for d in drawings:
                r_d = self.to_rect(d.get("rect"))
                if r_d and any(
                    cb.bbox.expanded(4.0, 4.0).contains(r_d)
                    or (cb.bbox.expanded(4.0, 4.0).intersects(r_d) and cb.bbox.expanded(4.0, 4.0).intersection_area(r_d) > 0.4 * r_d.area)
                    for cb in confirmed_callouts
                ):
                    continue
                cleaned_drawings.append(d)
            drawings = cleaned_drawings

        # =========================================================================
        # ADIM 3: TABLO ÖNCELİĞİ VE ÇOK SÜTUNLU ZEBRA YENİDEN YAPILANDIRMA (KURAL 1 & 3)
        # =========================================================================
        valid_tables: List[TableIR] = []
        valid_table_bboxes: List[Rect] = []
        try:
            table_finder = getattr(self.page, "find_tables", None)
            if callable(table_finder):
                raw_tables = table_finder().tables
                candidate_frags = []
                for t in raw_tables:
                    t_rect = self.to_rect(getattr(t, "bbox", None))
                    if not t_rect or t_rect.width < self.width * 0.40:
                        continue
                    if any(t_rect.intersects(cb.bbox) and (t_rect.intersection_area(cb.bbox) > 0.30 * t_rect.area) for cb in confirmed_callouts):
                        continue
                    has_curves = False
                    t_exp = t_rect.expanded(4.0, 4.0)
                    for d in drawings:
                        r_d = self.to_rect(d.get("rect"))
                        if not r_d or not t_exp.intersects(r_d):
                            continue
                        for item in d.get("items", []):
                            if item and item[0] == "c":
                                has_curves = True
                                break
                        if has_curves:
                            break
                    if has_curves:
                        continue
                    candidate_frags.append((t, t_rect))

                grouped_tables = []
                for t, tr in candidate_frags:
                    matched = False
                    for g in grouped_tables:
                        gr = g["rect"]
                        h_align = (abs(gr.x0 - tr.x0) <= 8.0 and abs(gr.x1 - tr.x1) <= 8.0)
                        top_r = gr if gr.y0 <= tr.y0 else tr
                        bot_r = tr if gr.y0 <= tr.y0 else gr
                        v_gap = bot_r.y0 - top_r.y1
                        if h_align and 0.0 <= v_gap <= 45.0:
                            g["rect"] = g["rect"].union(tr)
                            g["frags"].append(t)
                            matched = True
                            break
                    if not matched:
                        grouped_tables.append({"rect": tr, "frags": [t]})

                for gi, g in enumerate(grouped_tables):
                    table_ir = self._build_reconstructed_table_ir(
                        f"table_{gi}",
                        g["rect"],
                        g["frags"],
                        raw_parsed_spans,
                        consumed_span_ids,
                    )
                    if table_ir:
                        valid_tables.append(table_ir)
                        valid_table_bboxes.append(g["rect"])
        except Exception:
            pass

        page_ir.tables = valid_tables

        if valid_table_bboxes:
            cleaned_drawings = []
            for d in drawings:
                r_d = self.to_rect(d.get("rect"))
                if r_d and any(
                    tb.expanded(4.0, 4.0).contains(r_d)
                    or (tb.expanded(4.0, 4.0).intersects(r_d) and tb.expanded(4.0, 4.0).intersection_area(r_d) > 0.4 * r_d.area)
                    for tb in valid_table_bboxes
                ):
                    continue
                cleaned_drawings.append(d)
            drawings = cleaned_drawings

        # =========================================================================
        # ADIM 4: KORUMALI METİN BÖLGELERİNİ (PROTECTED TEXT ZONES) BELİRLE
        # =========================================================================
        non_table_spans = [s for s in raw_parsed_spans if s.span_id not in consumed_span_ids]

        initial_lines = cluster_spans_into_lines(non_table_spans)
        initial_blocks = group_lines_into_blocks(initial_lines)

        bullet_re = re.compile(r"^([•\-\*▪■\u2022\u25aa\u25cf\u25e6]|\d+[\.\)]|[a-zA-Z][\.\)])\s+")
        caption_prefixes = ("figure ", "table ", "şekil ", "tablo ", "note", "tip", "step ")

        protected_blocks: List[TextBlockIR] = []
        protected_line_rects: List[Rect] = []

        for block in initial_blocks:
            is_prot = False
            b_txt = block.text.strip()
            b_txt_lower = b_txt.lower()
            b_w = block.bbox.width

            # 1. Çok satırlı gövde paragrafları (satır sayısı >= 2 ve genişlik >= page_w * 0.40)
            if len(block.lines) >= 2 and b_w >= self.width * 0.40:
                is_prot = True
            # 2. Geniş tek satırlık cümleler (genişlik >= page_w * 0.44)
            elif b_w >= self.width * 0.44:
                is_prot = True
            # 3. Figure, Table, NOTE, TIP, Step veya madde işareti ile başlayan başlık/altyazılar
            elif any(b_txt_lower.startswith(pfx) for pfx in caption_prefixes) or bool(bullet_re.match(b_txt)):
                is_prot = True
            # 4. Üstbilgi (%8) ve altbilgi (%6) şeridindeki metinler
            elif block.bbox.y1 <= self.height * 0.08 or block.bbox.y0 < self.height * 0.08 or block.bbox.y0 >= self.height * 0.94:
                is_prot = True
            # 5. Bölüm başlıkları (font_size >= 10.0 veya is_bold)
            elif block.lines and block.lines[0].spans and (block.lines[0].spans[0].font.size >= 10.0 or block.lines[0].spans[0].font.bold) and block.alignment == "LEFT":
                is_prot = True

            block.is_protected = is_prot
            if is_prot:
                protected_blocks.append(block)
                for l in block.lines:
                    protected_line_rects.append(l.bbox)

        # =========================================================================
        # ADIM 4: KALIN YATAY ÇİZGİLER (ŞERİT DÖNÜŞTÜRME) & CALLOUT AYRIMI
        # =========================================================================
        for d in drawings:
            try:
                r_raw = d.get("rect")
                if not r_raw:
                    continue
                rx0 = float(r_raw[0] if isinstance(r_raw, (list, tuple)) else r_raw.x0)
                ry0 = float(r_raw[1] if isinstance(r_raw, (list, tuple)) else r_raw.y0)
                rx1 = float(r_raw[2] if isinstance(r_raw, (list, tuple)) else r_raw.x1)
                ry1 = float(r_raw[3] if isinstance(r_raw, (list, tuple)) else r_raw.y1)
                rh = abs(ry1 - ry0)
                rw = abs(rx1 - rx0)
                sw = safe_float(d.get("width"), 0.0)
                if rh < 3.0 and sw >= 10.0 and rw >= self.raw_w * 0.20:
                    y_mid = (ry0 + ry1) / 2.0
                    d["rect"] = [rx0, y_mid - sw / 2.0, rx1, y_mid + sw / 2.0]
                    d["fill"] = d.get("color") or d.get("stroke")
                    d["_is_straight_strip"] = True
            except Exception:
                continue

        callout_candidates: List[CalloutIR] = []
        diagram_box_candidates: List[Rect] = []

        filled_drawings = []
        stroke_drawings = []
        for d in drawings:
            r = self.to_rect(d.get("rect"))
            if not r:
                continue
            if d.get("fill"):
                filled_drawings.append((d, r))
            else:
                stroke_drawings.append((d, r))

        image_boxes: List[Rect] = []
        for d, r in filled_drawings:
            # DÜZELTME 5: Altbilgi (%94+) şeridindeki beyaz arka plan kutularını atla
            if r.y0 >= self.height * 0.94:
                continue

            if any(cb.bbox.contains(r) or (cb.bbox.intersects(r) and cb.bbox.intersection_area(r) > 0.5 * r.area) for cb in confirmed_callouts):
                continue
            if any(tb.contains(r) or (tb.intersects(r) and tb.intersection_area(r) > 0.5 * r.area) for tb in valid_table_bboxes):
                continue

            cond1 = (r.area < self.width * self.height * 0.60) and (r.width >= self.width * 0.25)
            expanded_r = r.expanded(4.0, 4.0)
            has_sub_elements = False
            for od, o_r in filled_drawings:
                if od is not d and expanded_r.intersects(o_r):
                    has_sub_elements = True
                    break
            if not has_sub_elements:
                for od, o_r in stroke_drawings:
                    if abs(o_r.x0 - r.x0) < 3.0 and abs(o_r.y0 - r.y0) < 3.0 and abs(o_r.x1 - r.x1) < 3.0 and abs(o_r.y1 - r.y1) < 3.0:
                        continue
                    if expanded_r.intersects(o_r):
                        has_sub_elements = True
                        break

            has_adjacent_box = False
            for od, o_r in filled_drawings:
                if od is not d:
                    if max(r.y0 - 6.0, o_r.y0) <= min(r.y1 + 6.0, o_r.y1):
                        has_adjacent_box = True
                        break

            if cond1 and not has_sub_elements and not has_adjacent_box:
                bg_rgb = safe_color_rgb(d.get("fill"), (241, 245, 249))
                border_rgb = safe_color_rgb(d.get("color"))
                # Filter out pure white callouts without border
                if bg_rgb == (255, 255, 255) and not border_rgb:
                    continue
                raw_w = d.get("width")
                border_w = safe_float(raw_w, 0.75) if raw_w is not None else 0.75
                # DÜZELTME 5: Köşeleri yuvarlak NOTE kutuları için is_rounded=True, düz şeritler için False
                has_curves = any(it and it[0] == "c" for it in d.get("items", []))
                is_round = False if d.get("_is_straight_strip") else has_curves
                confirmed_callouts.append(
                    CalloutIR(
                        id=f"callout_{len(confirmed_callouts)}",
                        bbox=r,
                        bg_color=bg_rgb,
                        border_color=border_rgb,
                        border_width=border_w,
                        is_rounded=is_round,
                    )
                )
            else:
                if r.area < self.width * self.height * 0.85:
                    diagram_box_candidates.append(r)

        for od, o_r in stroke_drawings:
            is_divider = (
                (o_r.height <= 4.5 and o_r.width >= self.width * 0.15) or
                (o_r.width <= 4.5 and o_r.height >= self.height * 0.10)
            )
            if not is_divider and o_r.area < self.width * self.height * 0.85 and (o_r.width > 2.0 or o_r.height > 2.0):
                diagram_box_candidates.append(o_r)

        try:
            for img_info in self.page.get_images(full=True):
                xref = img_info[0]
                for r_fitz in self.page.get_image_rects(xref):
                    r_img = self.to_rect(r_fitz)
                    if r_img and r_img.area < self.width * self.height * 0.90:
                        diagram_box_candidates.append(r_img)
                        image_boxes.append(r_img)
        except Exception:
            pass

        # =========================================================================
        # ADIM 5: KENAR SEKİLERİ ("1") VE KENAR ROZETLERİ ("Key Topic") İZOLASYONU
        # Sıkı Karmaşıklık Kapısı: 9 pt ve 14.6 pt şeritler asla rozet/sekme olamaz!
        # =========================================================================
        final_diagram_bboxes: List[Rect] = []
        remaining_candidates: List[Rect] = []

        margin_badge_rects: List[Rect] = []
        header_tab_rects: List[Rect] = []

        for box in diagram_box_candidates:
            is_left_badge = (
                box.x1 <= self.width * 0.15
                and 18.0 <= box.width < self.width * 0.16
                and 14.0 <= box.height < self.height * 0.12
                and 0.3 <= (box.width / max(box.height, 1.0)) <= 3.0
            )
            is_right_tab = (
                box.x0 >= self.width * 0.85
                and box.y0 <= self.height * 0.12
                and 12.0 <= box.width < self.width * 0.16
                and box.height < self.height * 0.12
            )
            if is_left_badge:
                margin_badge_rects.append(box)
            elif is_right_tab:
                header_tab_rects.append(box)
            else:
                remaining_candidates.append(box)

        # 1. Sol Marj Rozetleri (Key Topic vb.): vektör kutusu + iç metinler ("Key", "Topic") + 2.5 pt pay
        if margin_badge_rects:
            m_clusters: List[Rect] = []
            for mb in margin_badge_rects:
                matched = False
                for mi, mc in enumerate(m_clusters):
                    if mc.intersects(mb.expanded(2.0, 4.0)):
                        m_clusters[mi] = mc.union(mb)
                        matched = True
                        break
                if not matched:
                    m_clusters.append(mb)

            page_badges = []
            for mi, mc in enumerate(m_clusters):
                badge_union = mc
                badge_text_parts = []
                for s in raw_parsed_spans:
                    if mc.expanded(2.0, 2.0).contains(Rect(s.bbox.center_x, s.bbox.center_y, s.bbox.center_x + 1, s.bbox.center_y + 1)):
                        badge_union = badge_union.union(s.bbox)
                        badge_text_parts.append(s.text.strip())
                        consumed_span_ids.add(s.span_id)

                badge_clip = badge_union.expanded(2.5, 2.5).clamp_to_slide(self.width, self.height)
                final_diagram_bboxes.append(badge_clip)
                try:
                    pix = self.page.get_pixmap(clip=self.to_pdf_clip(badge_clip), dpi=288, alpha=False)
                    img_bytes = pix.tobytes("png")
                    page_badges.append({
                        "rect": badge_clip,
                        "bytes": img_bytes,
                        "text_len": len(" ".join(badge_text_parts)),
                        "bytes_len": len(img_bytes),
                    })
                except Exception:
                    pass

            # DÜZELTME 4: Aynı boyutlu (±3.0 pt) rozetler arasında en tam olan rozetin image_bytes verisini kullan
            for bi, b_item in enumerate(page_badges):
                dim_key = (int(round(b_item["rect"].width)), int(round(b_item["rect"].height)))
                best_bytes = b_item["bytes"]
                best_metric = (b_item["text_len"], b_item["bytes_len"])

                # Bu sayfadaki rozetleri kıyasla
                for other in page_badges:
                    if abs(other["rect"].width - b_item["rect"].width) <= 3.0 and abs(other["rect"].height - b_item["rect"].height) <= 3.0:
                        other_metric = (other["text_len"], other["bytes_len"])
                        if other_metric > best_metric:
                            best_metric = other_metric
                            best_bytes = other["bytes"]

                # Belge genelindeki önbelleği kontrol et
                for cached_dim, cached_bytes in _GLOBAL_BADGE_CACHE.items():
                    if abs(cached_dim[0] - dim_key[0]) <= 3 and abs(cached_dim[1] - dim_key[1]) <= 3:
                        if len(cached_bytes) > len(best_bytes):
                            best_bytes = cached_bytes

                # Önbelleği güncelle
                if dim_key not in _GLOBAL_BADGE_CACHE or len(best_bytes) > len(_GLOBAL_BADGE_CACHE[dim_key]):
                    _GLOBAL_BADGE_CACHE[dim_key] = best_bytes

                page_ir.diagrams.append(
                    DiagramIR(
                        id=f"badge_{len(page_ir.diagrams)}",
                        bbox=b_item["rect"],
                        image_bytes=best_bytes,
                        element_count=1,
                    )
                )

        # 2. Sağ Kenar Sekmeleri ("1" vb.): sayfa sınırıyla kesiştir (clip_rect & page.rect)
        if header_tab_rects:
            tab_union = header_tab_rects[0]
            for tb in header_tab_rects[1:]:
                tab_union = tab_union.union(tb)
            for s in raw_parsed_spans:
                if tab_union.expanded(2.0, 2.0).contains(Rect(s.bbox.center_x, s.bbox.center_y, s.bbox.center_x + 1, s.bbox.center_y + 1)):
                    tab_union = tab_union.union(s.bbox)
                    consumed_span_ids.add(s.span_id)

            tab_clip = tab_union.expanded(2.5, 2.5)
            # Slayt sınırı kilidi: x1 asla self.width'i aşamaz (bleed önleme)
            tab_clip = Rect(
                max(0.0, tab_clip.x0),
                max(0.0, tab_clip.y0),
                min(self.width, tab_clip.x1),
                min(self.height, tab_clip.y1),
            )
            final_diagram_bboxes.append(tab_clip)
            try:
                pix = self.page.get_pixmap(clip=self.to_pdf_clip(tab_clip), dpi=288, alpha=False)
                page_ir.diagrams.append(
                    DiagramIR(
                        id=f"tab_{len(page_ir.diagrams)}",
                        bbox=tab_clip,
                        image_bytes=pix.tobytes("png"),
                        element_count=1,
                    )
                )
            except Exception:
                pass

        # =========================================================================
        # ADIM 6: SAYFA ORANLI DİYAGRAM KÜMELEME (UNION-FIND) - ANA DİYAGRAMLAR
        # =========================================================================
        h_tol = self.width * 0.085
        v_tol = self.height * 0.028

        clusters: List[Rect] = []
        for box in remaining_candidates:
            matched_indices = []
            for ci, c in enumerate(clusters):
                # Boundary isolation: Top header band (y1 <= 0.08) never merges with body
                if (c.y1 <= self.height * 0.08 and box.y0 > self.height * 0.08) or (box.y1 <= self.height * 0.08 and c.y0 > self.height * 0.08):
                    continue
                # Left margin isolation
                if (c.x1 < self.width * 0.15 and box.x0 >= self.width * 0.14) or (box.x1 < self.width * 0.15 and c.x0 >= self.width * 0.14):
                    continue

                h_gap = max(0.0, max(c.x0 - box.x1, box.x0 - c.x1))
                v_gap = max(0.0, max(c.y0 - box.y1, box.y0 - c.y1))
                if h_gap <= h_tol and v_gap <= v_tol:
                    matched_indices.append(ci)
            if not matched_indices:
                clusters.append(box)
            else:
                first_idx = matched_indices[0]
                merged_rect = clusters[first_idx].union(box)
                for other_idx in matched_indices[1:]:
                    merged_rect = merged_rect.union(clusters[other_idx])
                clusters[first_idx] = merged_rect
                for other_idx in reversed(matched_indices[1:]):
                    clusters.pop(other_idx)

        # KARMAŞIKLIK KAPISI (KURAL 4):
        valid_clusters = []
        for c in clusters:
            ar = c.width / max(c.height, 1.0)
            if c.width >= 24.0 and c.height >= 18.0 and 0.25 <= ar <= 8.0:
                has_comp = False
                for ib in image_boxes:
                    if c.intersects(ib):
                        has_comp = True
                        break
                if not has_comp:
                    inside_items = 0
                    for d in drawings:
                        rd = self.to_rect(d.get("rect"))
                        if rd and c.contains(rd):
                            inside_items += len(d.get("items", []))
                            if any(it and it[0] == "c" for it in d.get("items", [])):
                                has_comp = True
                                break
                    if inside_items >= 2:
                        has_comp = True
                if has_comp:
                    valid_clusters.append(c)
        clusters = valid_clusters

        # =========================================================================
        # ADIM 7: ETİKET GENİŞLETMESİ (LABEL EXPANSION) - KORUMALI METİNLER ASLA KATILMAZ!
        # =========================================================================
        v_label_dist = self.height * 0.022
        h_label_dist = self.width * 0.12

        for _ in range(2):
            for ci, c in enumerate(clusters):
                for l_r, l_txt in raw_text_lines:
                    if not l_txt:
                        continue
                    # Kural 1: Korumalı bir satır ise (ör. gövde paragrafının kısa son satırı) ASLA diyagrama katma!
                    if any(l_r.intersects(pr) and l_r.intersection_area(pr) > 0.3 * l_r.area for pr in protected_line_rects):
                        continue
                    # Kural 2: Yalnızca kısa ve izole diyagram etiketlerini kapsayabilir
                    if l_r.width < self.width * 0.35 and len(l_txt) < 80:
                        if c.contains(l_r):
                            continue
                        h_dist = max(0.0, max(c.x0 - l_r.x1, l_r.x0 - c.x1))
                        v_dist = max(0.0, max(c.y0 - l_r.y1, l_r.y0 - c.y1))
                        if h_dist <= h_label_dist and v_dist <= v_label_dist:
                            clusters[ci] = clusters[ci].union(l_r)

        # Kesişen / iç içe geçen diyagramları tekilleştir
        merged = True
        while merged:
            merged = False
            new_clusters: List[Rect] = []
            used = set()
            for i in range(len(clusters)):
                if i in used:
                    continue
                cur_c = clusters[i]
                for j in range(i + 1, len(clusters)):
                    if j in used:
                        continue
                    other_c = clusters[j]
                    h_dist = max(0.0, max(cur_c.x0 - other_c.x1, other_c.x0 - cur_c.x1))
                    v_dist = max(0.0, max(cur_c.y0 - other_c.y1, other_c.y0 - cur_c.y1))
                    if cur_c.intersects(other_c) or cur_c.contains(other_c) or other_c.contains(cur_c) or (h_dist < 6.0 and v_dist < 6.0):
                        cur_c = cur_c.union(other_c)
                        used.add(j)
                        merged = True
                new_clusters.append(cur_c)
            clusters = new_clusters

        # DÜZELTME 2.1: Aynı Diyagram Bandındaki Parçaları Tekilleştir (Same-Figure Band Consolidation)
        # Kenar rozetleri hariç (width > page_w * 0.14)
        # h_dist <= page_w * 0.16 (~85 pt) and v_dist <= page_h * 0.045 (~30 pt)
        merged_bands = True
        while merged_bands:
            merged_bands = False
            new_clusters = []
            used = set()
            for i in range(len(clusters)):
                if i in used:
                    continue
                cur_c = clusters[i]
                for j in range(i + 1, len(clusters)):
                    if j in used:
                        continue
                    other_c = clusters[j]
                    if cur_c.width <= self.width * 0.14 or other_c.width <= self.width * 0.14:
                        continue
                    h_dist = max(0.0, max(cur_c.x0 - other_c.x1, other_c.x0 - cur_c.x1))
                    v_dist = max(0.0, max(cur_c.y0 - other_c.y1, other_c.y0 - cur_c.y1))

                    if h_dist <= self.width * 0.16 and v_dist <= self.height * 0.045:
                        between_y0 = min(cur_c.y1, other_c.y1)
                        between_y1 = max(cur_c.y0, other_c.y0)
                        has_divider_text = False
                        if between_y1 > between_y0 + 2.0:
                            for pb in protected_blocks:
                                is_body_or_cap = (
                                    (len(pb.lines) >= 2 and pb.bbox.width >= self.width * 0.40)
                                    or pb.text.strip().lower().startswith(("figure ", "şekil ", "figür "))
                                )
                                if is_body_or_cap:
                                    if pb.bbox.y0 >= between_y0 - 2.0 and pb.bbox.y1 <= between_y1 + 2.0:
                                        has_divider_text = True
                                        break
                        if not has_divider_text:
                            cur_c = cur_c.union(other_c)
                            used.add(j)
                            merged_bands = True
                new_clusters.append(cur_c)
            clusters = new_clusters

        # DÜZELTME 2.2: Diyagram Üst/İç Grup Başlıklarını Diyagram Kutusuna Kat (Diagram Header Inclusion)
        absorbed_diagram_texts: List[Rect] = []
        caption_skip_pfx = ("figure ", "şekil ", "figür ", "table ", "tablo ", "step ")
        for ci in range(len(clusters)):
            c = clusters[ci]
            if c.width <= self.width * 0.14:
                continue
            for b in initial_blocks:
                b_r = b.bbox
                b_txt = b.text.strip().lower()
                if b_r.width >= self.width * 0.35 or len(b.lines) > 2:
                    continue
                if any(b_txt.startswith(pfx) for pfx in caption_skip_pfx):
                    continue
                if b_r.x0 < self.width * 0.15 and b_r.y0 < self.height * 0.08:
                    continue

                is_inside = c.expanded(2.0, 2.0).contains(b_r)
                is_above = (
                    (c.y0 - 20.0 <= b_r.y1 <= c.y1)
                    and (c.x0 - 15.0 <= b_r.x0)
                    and (b_r.x1 <= c.x1 + 15.0)
                )
                if is_inside or is_above:
                    clusters[ci] = clusters[ci].union(b_r)
                    absorbed_diagram_texts.append(b_r)
                    b.is_protected = False

        # =========================================================================
        # ADIM 8: DİYAGRAM KUTUSU SINIR KİLİDİ (CLAMPING) VE 288 DPI KIRPMA
        # =========================================================================
        for c_idx, c_rect in enumerate(clusters):
            clip = Rect(
                max(0.0, c_rect.x0 - 3.5),
                max(0.0, c_rect.y0 - 3.5),
                min(self.width, c_rect.x1 + 3.5),
                min(self.height, c_rect.y1 + 3.5),
            )

            # Sınır Kilidi (Clamping): Korumalı paragraflar ve Figure altyazıları
            for pb in protected_blocks:
                # Diyagram içine katılan grup başlıklarına karşı tavanı aşağı itme
                if any(ar.contains(pb.bbox) or pb.bbox.contains(ar) for ar in absorbed_diagram_texts):
                    continue
                h_overlap = max(0.0, min(clip.x1, pb.bbox.x1) - max(clip.x0, pb.bbox.x0))
                if h_overlap > 5.0:
                    # Paragraf veya başlık üstteyse, diyagramın tavanını aşağı it
                    if pb.bbox.y0 < clip.y0 and pb.bbox.y1 > clip.y0 - 8.0:
                        clip.y0 = max(clip.y0, pb.bbox.y1 + 1.5)
                    # Altyazı veya paragraf alttaysa, diyagramın tabanını yukarı çek
                    if pb.bbox.y1 > clip.y1 and pb.bbox.y0 < clip.y1 + 8.0:
                        clip.y1 = min(clip.y1, pb.bbox.y0 - 1.5)

            # Tablolarla çakışmayı önle
            for tb in valid_table_bboxes:
                h_overlap = max(0.0, min(clip.x1, tb.x1) - max(clip.x0, tb.x0))
                if h_overlap > 5.0:
                    if tb.y0 < clip.y0 and tb.y1 > clip.y0 - 8.0:
                        clip.y0 = max(clip.y0, tb.y1 + 1.5)
                    if tb.y1 > clip.y1 and tb.y0 < clip.y1 + 8.0:
                        clip.y1 = min(clip.y1, tb.y0 - 1.5)

            clip = clip.clamp_to_slide(self.width, self.height)

            if clip.width >= 18.0 and clip.height >= 18.0:
                final_diagram_bboxes.append(clip)
                # Register internal spans to consumed_span_ids (Kural 1)
                for s in raw_parsed_spans:
                    if clip.intersects(s.bbox) and s.bbox.area > 0:
                        if (clip.intersection_area(s.bbox) / s.bbox.area) >= 0.75:
                            is_body_para = any(
                                pb.bbox.contains(s.bbox) and len(pb.lines) >= 3 and pb.bbox.width >= self.width * 0.50
                                for pb in protected_blocks
                            )
                            if not is_body_para:
                                consumed_span_ids.add(s.span_id)

                try:
                    fitz_clip = self.to_pdf_clip(clip)
                    pix = self.page.get_pixmap(clip=fitz_clip, dpi=288, alpha=False)
                    page_ir.diagrams.append(
                        DiagramIR(
                            id=f"diagram_{c_idx}",
                            bbox=clip,
                            image_bytes=pix.tobytes("png"),
                            element_count=len(clusters),
                        )
                    )
                except Exception:
                    pass

        # Callouts ekle (Diyagram ile çakışmayanlar)
        for c in confirmed_callouts:
            if not any(c.bbox.intersects(db) and (c.bbox.intersection_area(db) >= 0.15 * c.bbox.area) for db in final_diagram_bboxes):
                page_ir.callouts.append(c)

        # Ayırıcı çizgiler (ShapeIR)
        divider_x: Optional[float] = None
        for d in stroke_drawings:
            try:
                for item in d[0].get("items", []):
                    cmd = item[0]
                    if cmd == "l" and len(item) >= 3:
                        p1, p2 = item[1], item[2]
                        if not hasattr(p1, "x") or not hasattr(p2, "x"):
                            continue
                        p1_x = (safe_float(p1.x) - self.origin_x) * self.scale
                        p1_y = (safe_float(p1.y) - self.origin_y) * self.scale
                        p2_x = (safe_float(p2.x) - self.origin_x) * self.scale
                        p2_y = (safe_float(p2.y) - self.origin_y) * self.scale
                        if abs(p1_x - p2_x) < 3.0 and abs(p2_y - p1_y) > 100.0:
                            divider_x = p1_x
                        line_r = Rect(min(p1_x, p2_x), min(p1_y, p2_y), max(p1_x, p2_x), max(p1_y, p2_y))
                        if not any(line_r.intersects(db) for db in final_diagram_bboxes) and not any(line_r.intersects(tb) for tb in valid_table_bboxes):
                            if ((p2_x - p1_x) ** 2 + (p2_y - p1_y) ** 2) ** 0.5 > 15:
                                color = d[0].get("color")
                                stroke_rgb = safe_color_rgb(color, (0, 0, 0))
                                raw_w = d[0].get("width")
                                w = max(0.5, safe_float(raw_w, 0.75) * self.scale)
                                page_ir.shapes.append(
                                    ShapeIR(
                                        id=f"line_{len(page_ir.shapes)}",
                                        shape_type="line",
                                        bbox=line_r,
                                        p1=(p1_x, p1_y),
                                        p2=(p2_x, p2_y),
                                        stroke_color=stroke_rgb,
                                        stroke_width=w,
                                    )
                                )
            except Exception:
                continue

        # =========================================================================
        # ADIM 9: METİN ÇIKARIMI VE TEKİL SPAN SAHİPLİĞİ (KURAL 1 & KURAL 5)
        # =========================================================================
        body_spans: List[TextSpanIR] = []
        for s in raw_parsed_spans:
            # 1. Tüketilmiş span'ler (tablolar, rozetler, diyagram etiketleri) ASLA body_spans'e giremez! (Kural 1)
            if s.span_id in consumed_span_ids:
                continue

            # 2. Absorbed grup başlıkları
            if any(ar.contains(Rect(s.bbox.center_x, s.bbox.center_y, s.bbox.center_x + 1, s.bbox.center_y + 1)) for ar in absorbed_diagram_texts):
                continue

            # 3. Beyaz zemin üstünde beyaz metin filtresi
            in_callout = any(c.bbox.contains(s.bbox) for c in page_ir.callouts)
            if s.font.color == (255, 255, 255) and not in_callout:
                continue

            parent_container = None
            for c in page_ir.callouts:
                if c.bbox.contains(s.bbox) or c.bbox.intersection_area(s.bbox) > 0.6 * s.bbox.area:
                    parent_container = c.bbox
                    break

            s.parent_container_rect = parent_container
            body_spans.append(s)

        # Düzen ve okuma sırasına göre bloklama
        if body_spans:
            try:
                sorted_blocks = partition_and_process_spans(
                    body_spans, self.width, self.height, divider_x=divider_x
                )
                for b in sorted_blocks:
                    b_txt = b.text.strip()
                    b_txt_lower = b_txt.lower()
                    b_w = b.bbox.width
                    if len(b.lines) >= 2 and b_w >= self.width * 0.40:
                        b.is_protected = True
                    elif b_w >= self.width * 0.44:
                        b.is_protected = True
                    elif any(b_txt_lower.startswith(pfx) for pfx in caption_prefixes) or bool(bullet_re.match(b_txt)):
                        b.is_protected = True
                    elif b.bbox.y1 <= self.height * 0.08 or b.bbox.y0 < self.height * 0.08 or b.bbox.y0 >= self.height * 0.94:
                        b.is_protected = True
                    elif b.lines and b.lines[0].spans and (b.lines[0].spans[0].font.size >= 10.0 or b.lines[0].spans[0].font.bold) and b.alignment == "LEFT":
                        b.is_protected = True

                filtered_blocks = []
                for b in sorted_blocks:
                    # Tablo ile çakışan blokları bastır (> %25 kesişim - Kural 1)
                    if any(b.bbox.intersects(tb) and (b.bbox.intersection_area(tb) / max(b.bbox.area, 1.0) > 0.25) for tb in valid_table_bboxes):
                        continue
                    is_real_body_para = (len(b.lines) >= 3 and b.bbox.width >= self.width * 0.50)
                    in_diag = False
                    for db in final_diagram_bboxes:
                        if db.intersects(b.bbox) and b.bbox.area > 0:
                            if (db.intersection_area(b.bbox) / b.bbox.area) >= 0.75 and not is_real_body_para:
                                in_diag = True
                                break
                    if in_diag:
                        continue
                    if any(ar.contains(Rect(b.bbox.center_x, b.bbox.center_y, b.bbox.center_x + 1, b.bbox.center_y + 1)) for ar in absorbed_diagram_texts):
                        continue
                    filtered_blocks.append(b)

                page_ir.text_blocks = filtered_blocks
            except Exception:
                lines = cluster_spans_into_lines(body_spans)
                raw_b = group_lines_into_blocks(lines)
                filtered_blocks = []
                for b in raw_b:
                    if any(b.bbox.intersects(tb) and (b.bbox.intersection_area(tb) / max(b.bbox.area, 1.0) > 0.25) for tb in valid_table_bboxes):
                        continue
                    is_real_body_para = (len(b.lines) >= 3 and b.bbox.width >= self.width * 0.50)
                    in_diag = False
                    for db in final_diagram_bboxes:
                        if db.intersects(b.bbox) and b.bbox.area > 0:
                            if (db.intersection_area(b.bbox) / b.bbox.area) >= 0.75 and not is_real_body_para:
                                in_diag = True
                                break
                    if in_diag:
                        continue
                    filtered_blocks.append(b)
                page_ir.text_blocks = filtered_blocks

        return page_ir

    def _build_reconstructed_table_ir(
        self,
        table_id: str,
        t_rect: Rect,
        fragments: list,
        all_spans: List[TextSpanIR],
        consumed_span_ids: set[int],
    ) -> Optional[TableIR]:
        try:
            fragments.sort(key=lambda t: t.bbox[1])
            # En uygun sütun sınırlarını bul (satır 0 hücreleri eksiksiz olan ilk fragman)
            best_frag = None
            for frag in fragments:
                if frag.rows and len(frag.rows[0].cells) >= 2:
                    if all(c is not None for c in frag.rows[0].cells):
                        best_frag = frag
                        break
            if not best_frag:
                best_frag = max(fragments, key=lambda f: len(f.rows[0].cells) if f.rows else 0)

            first_row_cells = best_frag.rows[0].cells if best_frag.rows else []
            col_bounds = []
            if first_row_cells:
                first_c = first_row_cells[0]
                if first_c and len(first_c) >= 4 and first_c[0] is not None:
                    col_bounds.append((safe_float(first_c[0]) - self.origin_x) * self.scale)
                for c in first_row_cells:
                    if c and len(c) >= 4 and c[2] is not None:
                        col_bounds.append((safe_float(c[2]) - self.origin_x) * self.scale)

            if len(col_bounds) < 2:
                col_bounds = [t_rect.x0, t_rect.x0 + t_rect.width * 0.25, t_rect.x0 + t_rect.width * 0.6, t_rect.x1]

            cols = len(col_bounds) - 1
            col_w = [col_bounds[i + 1] - col_bounds[i] for i in range(cols)]

            # Satır sınırlarını birleştir (fragman satırları + beyaz ara boşluk satırları)
            row_bounds = []
            for fi, frag in enumerate(fragments):
                if fi > 0:
                    prev_frag = fragments[fi - 1]
                    gap_y0 = (safe_float(prev_frag.bbox[3]) - self.origin_y) * self.scale
                    gap_y1 = (safe_float(frag.bbox[1]) - self.origin_y) * self.scale
                    if gap_y1 - gap_y0 >= 8.0:
                        row_bounds.append((gap_y0, gap_y1))
                for r in frag.rows:
                    ry0 = (safe_float(r.bbox[1]) - self.origin_y) * self.scale
                    ry1 = (safe_float(r.bbox[3]) - self.origin_y) * self.scale
                    if ry1 - ry0 >= 4.0:
                        row_bounds.append((ry0, ry1))

            rows = len(row_bounds)
            if rows == 0 or cols == 0:
                return None

            row_h = [rb[1] - rb[0] for rb in row_bounds]

            cells: List[List[TableCellIR]] = []
            PUNCT_CHARS = (",", ".", ";", ":", "!", "?", ")", "]", "”", "’", "*", "%")

            for ri, (ry0, ry1) in enumerate(row_bounds):
                row_cells: List[TableCellIR] = []
                for ci in range(cols):
                    cx0 = col_bounds[ci]
                    cx1 = col_bounds[ci + 1]
                    cell_rect = Rect(cx0, ry0, cx1, ry1)

                    matched_cell_spans: List[TextSpanIR] = []
                    for s in all_spans:
                        if s.span_id not in consumed_span_ids:
                            if cell_rect.x0 <= s.bbox.center_x < cell_rect.x1 and cell_rect.y0 <= s.bbox.center_y < cell_rect.y1:
                                matched_cell_spans.append(s)

                    cell_lines = cluster_spans_into_lines(matched_cell_spans)
                    matched_cell_spans = [s for l in cell_lines for s in l.spans]

                    for ms in matched_cell_spans:
                        consumed_span_ids.add(ms.span_id)

                    cell_txt_parts = []
                    for l_idx, line in enumerate(cell_lines):
                        if l_idx > 0 and cell_txt_parts:
                            cell_txt_parts.append("\n")
                        for s_idx, ms in enumerate(line.spans):
                            stxt = clean_xml_text(ms.text).strip()
                            if not stxt:
                                continue
                            if s_idx > 0 and not stxt.startswith(PUNCT_CHARS) and cell_txt_parts and not cell_txt_parts[-1].endswith(" ") and not cell_txt_parts[-1].endswith("\n"):
                                cell_txt_parts.append(" " + stxt)
                            else:
                                cell_txt_parts.append(stxt)
                    raw_str = "".join(cell_txt_parts).strip()

                    is_hdr = (ri == 0)
                    font_size = (9.0 if cols > 10 else 9.5) * self.scale
                    font = FontInfo(
                        size=font_size,
                        bold=is_hdr,
                        color=(255, 255, 255) if is_hdr else (35, 31, 32),
                    )
                    bg = (88, 89, 91) if is_hdr else (255, 255, 255)
                    row_cells.append(
                        TableCellIR(
                            text=raw_str,
                            font=font,
                            bg_color=bg,
                            is_header=is_hdr,
                            font_size=font_size,
                            spans=matched_cell_spans,
                        )
                    )
                cells.append(row_cells)

            return TableIR(
                id=table_id,
                bbox=t_rect,
                rows=rows,
                cols=cols,
                col_widths=col_w,
                row_heights=row_h,
                cells=cells,
                header_bg_rgb=(88, 89, 91),
            )
        except Exception:
            return None

    def _build_table_ir(
        self,
        table_id: str,
        raw_table,
        t_rect: Rect,
        data: List[List],
        all_spans: Optional[List[TextSpanIR]] = None,
    ) -> Optional[TableIR]:
        consumed = set()
        return self._build_reconstructed_table_ir(
            table_id=table_id,
            t_rect=t_rect,
            fragments=[raw_table],
            all_spans=all_spans or [],
            consumed_span_ids=consumed,
        )
