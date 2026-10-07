"""
PDFKit High-Fidelity Office Reconstruction Engine
Powered by pdf2docx, PyMuPDF, and python-pptx (100% Offline / Local-first)

Features:
- Turkish & Unicode word boundary preservation (fixes merged/missing word spaces)
- Complex Graphic Region / Diagram protection (preserves multi-element diagrams, flowcharts, vector shapes)
- Full preservation of raster images and tables
- 100% compliant PowerPoint (.pptx) conversion with images, native tables, and exact text boxes
- Real-time JSON progress streaming on stdout
"""

import sys
import os
import io
import argparse
import json
import logging
import re
import warnings

# Suppress minor deprecation warnings so stdout/stderr remain clean
warnings.filterwarnings("ignore")
logging.basicConfig(level=logging.WARNING)


def emit_progress(current: int, total: int, percent: int, stage: str = "converting"):
    """Emits a single-line JSON progress message to stdout."""
    payload = {
        "type": "progress",
        "current": current,
        "total": total,
        "percent": max(0, min(100, percent)),
        "stage": stage,
    }
    print(json.dumps(payload), flush=True)


def patch_pdf2docx_unicode():
    """
    Patches pdf2docx internals to fix is_end_of_english_word bug that ignores
    non-ASCII/Turkish letters (c.encode().isalnum()).
    """
    try:
        import string
        from pdf2docx.text.Lines import Lines
        from pdf2docx.text.TextSpan import TextSpan

        punc_ex_hyphen = set(c for c in string.punctuation if c != "-")

        def is_word_char(c):
            if not c:
                return False
            # Python str.isalnum() returns True for all Unicode alphabetic chars (including ç, ğ, ı, ö, ş, ü)
            return c.isalnum() or (c in punc_ex_hyphen)

        def patched_adjust_last_word(self, delete_end_line_hyphen: bool):
            if not self._instances:
                return
            for i, line in enumerate(self._instances[:-1]):
                if not line.spans:
                    continue
                end_span = line.spans[-1]
                if not isinstance(end_span, TextSpan):
                    continue
                end_chars = end_span.chars
                if not end_chars:
                    continue
                end_char = end_chars[-1]

                if not self._instances[i + 1].spans:
                    continue
                start_span = self._instances[i + 1].spans[0]
                if not isinstance(start_span, TextSpan):
                    continue
                start_chars = start_span.chars
                if not start_chars:
                    continue
                next_start_char = start_chars[0]

                if delete_end_line_hyphen and end_char.c.endswith("-") and next_start_char.c.islower():
                    end_char.c = ""

                if is_word_char(end_char.c) and is_word_char(next_start_char.c):
                    end_char.c += " "

        Lines.adjust_last_word = patched_adjust_last_word
    except Exception:
        pass


def patch_pdf2docx_layout_protection():
    """
    Adds Complex Graphic Region detection and protection layer to RawPageFitz.extract_raw_dict.
    Preserves intricate vector diagrams, flowcharts, and multi-element schemas (e.g. Figure 2, Figure 3)
    as high-resolution, un-scrambled protected regions while allowing standard paragraphs, headings,
    and tables to remain 100% editable Word content.
    """
    try:
        import pymupdf
        from pdf2docx.page.RawPageFitz import RawPageFitz

        orig_extract_raw_dict = RawPageFitz.extract_raw_dict

        def patched_extract_raw_dict(self, **settings):
            raw_dict = orig_extract_raw_dict(self, **settings)
            page = self.page_engine
            if not page:
                return raw_dict

            drawings = page.get_drawings()
            if len(drawings) < 5:
                return raw_dict

            # 1. Cluster drawing rectangles based on visual proximity
            clusters = []
            for d in drawings:
                r = pymupdf.Rect(d["rect"])
                if r.is_empty or r.width < 1 or r.height < 1:
                    continue
                clusters.append({"bbox": pymupdf.Rect(r), "count": 1})

            changed = True
            while changed:
                changed = False
                for i in range(len(clusters)):
                    for j in range(i + 1, len(clusters)):
                        r1 = clusters[i]["bbox"]
                        r2 = clusters[j]["bbox"]
                        dx = max(0, max(r1.x0, r2.x0) - min(r1.x1, r2.x1))
                        dy = max(0, max(r1.y0, r2.y0) - min(r1.y1, r2.y1))
                        # Connect adjacent diagram components (e.g., chapter boxes, side panels, arrows)
                        if dx <= 80 and dy <= 25:
                            r1.include_rect(r2)
                            clusters[i]["count"] += clusters[j]["count"]
                            clusters.pop(j)
                            changed = True
                            break
                    if changed:
                        break

            # 2. Select complex regions: >= 5 drawing elements, min dimensions, not page background
            page_area = page.rect.width * page.rect.height
            complex_regions = []
            for c in clusters:
                bbox = c["bbox"]
                area = bbox.width * bbox.height
                if c["count"] >= 5 and bbox.width > 50 and bbox.height > 25 and (area / page_area) < 0.85:
                    # Add 2pt padding for visual breathing room
                    padded = pymupdf.Rect(
                        max(0, bbox.x0 - 2),
                        max(0, bbox.y0 - 2),
                        min(page.rect.width, bbox.x1 + 2),
                        min(page.rect.height, bbox.y1 + 2),
                    )
                    complex_regions.append(padded)

            if not complex_regions:
                return raw_dict

            # 3. Render high-res PNG (288 DPI = 4x crisp) for each complex region
            new_blocks = []
            for cr in complex_regions:
                pix = page.get_pixmap(clip=cr, dpi=288)
                img_bytes = pix.tobytes("png")
                img_block = {
                    "type": 1,
                    "bbox": (cr.x0, cr.y0, cr.x1, cr.y1),
                    "width": pix.width,
                    "height": pix.height,
                    "image": img_bytes,
                }
                new_blocks.append(img_block)

            # 4. Filter text blocks that fall inside complex regions (suppress fragmented labels & alt-texts)
            filtered_blocks = []
            for b in raw_dict.get("blocks", []):
                b_rect = pymupdf.Rect(b.get("bbox", [0, 0, 0, 0]))
                inside = False
                for cr in complex_regions:
                    intersect = pymupdf.Rect(b_rect).intersect(cr)
                    if not intersect.is_empty:
                        b_area = b_rect.width * b_rect.height
                        if b_area > 0 and (intersect.width * intersect.height) / b_area > 0.4:
                            inside = True
                            break
                if not inside:
                    filtered_blocks.append(b)

            filtered_blocks.extend(new_blocks)
            raw_dict["blocks"] = filtered_blocks

            # 5. Filter shapes that fall inside complex regions
            filtered_shapes = []
            for s in raw_dict.get("shapes", []):
                s_rect = pymupdf.Rect(s.get("bbox", [0, 0, 0, 0]))
                inside = any(cr.contains(s_rect) or s_rect.intersects(cr) for cr in complex_regions)
                if not inside:
                    filtered_shapes.append(s)
            raw_dict["shapes"] = filtered_shapes

            return raw_dict

        RawPageFitz.extract_raw_dict = patched_extract_raw_dict
    except Exception:
        pass


class StreamProgressHandler(logging.Handler):
    """Intercepts pdf2docx internal logging and converts page reports to JSON on stdout."""

    def __init__(self, total_pages: int):
        super().__init__()
        self.total_pages = max(1, total_pages)
        self.last_percent = -1
        self.pattern = re.compile(r"\((\d+)/(\d+)\)\s+Page\s+(\d+)")

    def emit(self, record):
        try:
            msg = record.getMessage()
            match = self.pattern.search(msg)
            if match:
                curr = int(match.group(1))
                tot = int(match.group(2))
                stage = "parse"
                if "Creating pages" in msg or getattr(self, "in_make_stage", False):
                    stage = "write"
                    self.in_make_stage = True
                    pct = 70 + int((curr / tot) * 30)
                else:
                    pct = int((curr / tot) * 70)

                if pct != self.last_percent:
                    self.last_percent = pct
                    emit_progress(curr, tot, pct, stage)
            elif "Creating pages" in msg:
                self.in_make_stage = True
                emit_progress(0, self.total_pages, 70, "write")
        except Exception:
            pass


def convert_pdf_to_docx(input_pdf: str, output_docx: str, pages=None) -> int:
    """Converts PDF into high-fidelity editable Word (.docx) preserving images, tables, and complex diagrams."""
    if not os.path.exists(input_pdf):
        print(json.dumps({"type": "error", "message": f"Girdi PDF bulunamadı: {input_pdf}"}), file=sys.stderr)
        return 1

    try:
        import pymupdf

        doc = pymupdf.open(input_pdf)
        total_doc_pages = len(doc)
        doc.close()

        total_pages = len(pages) if pages else total_doc_pages
        emit_progress(0, total_pages, 0, "start")

        # Apply Unicode Turkish boundary fix
        patch_pdf2docx_unicode()

        # Apply Complex Graphic Region Layout Protection
        patch_pdf2docx_layout_protection()

        from pdf2docx import Converter

        cv = Converter(input_pdf)

        logger = logging.getLogger()
        progress_handler = StreamProgressHandler(total_pages)
        logger.addHandler(progress_handler)

        if pages:
            cv.convert(output_docx, pages=pages)
        else:
            cv.convert(output_docx)

        cv.close()
        logger.removeHandler(progress_handler)

        emit_progress(total_pages, total_pages, 100, "done")
        print(json.dumps({"type": "success", "output": output_docx}))
        return 0

    except Exception as exc:
        print(json.dumps({"type": "error", "message": str(exc)}), flush=True)
        return 1


def convert_pdf_to_pptx(input_pdf: str, output_pptx: str, pages: list = None) -> int:
    """
    Converts PDF pages into 100% compliant Microsoft PowerPoint (.pptx) presentation
    with all images, native tables, diagrams, and exact positioned text boxes.
    """
    if not os.path.exists(input_pdf):
        print(json.dumps({"type": "error", "message": f"Girdi PDF bulunamadı: {input_pdf}"}), file=sys.stderr)
        return 1

    try:
        import pymupdf
        from pptx import Presentation
        from pptx.util import Pt
        from pptx.dml.color import RGBColor

        doc = pymupdf.open(input_pdf)
        target_pages = [doc[i] for i in pages if 0 <= i < len(doc)] if pages else list(doc)
        total_pages = len(target_pages)
        emit_progress(0, total_pages, 0, "start")

        prs = Presentation()
        # Set slide dimensions matching PDF page
        first_page = target_pages[0] if target_pages else doc[0]
        prs.slide_width = Pt(first_page.rect.width)
        prs.slide_height = Pt(first_page.rect.height)
        blank_layout = prs.slide_layouts[6]

        for idx, page in enumerate(target_pages):
            slide = prs.slides.add_slide(blank_layout)

            def clean_xml_text(s: str) -> str:
                if not s:
                    return ""
                # Strip illegal XML 1.0 control characters (only \t, \n, \r and >= 0x20 allowed)
                return "".join(c for c in s if c in ("\t", "\n", "\r") or ord(c) >= 32)

            # 1. Detect valid native tables FIRST so their background fills and borders are never mistaken for diagrams
            valid_tables = []
            valid_table_bboxes = []
            try:
                page_tables = page.find_tables().tables
                for t in page_tables:
                    data = t.extract()
                    if not data or len(data) < 2 or max(len(row) for row in data) < 2:
                        continue
                    valid_tables.append(t)
                    valid_table_bboxes.append(pymupdf.Rect(t.bbox))
            except Exception:
                pass

            # 2. Detect and protect complex vector drawing regions (flowcharts, diagrams, schemas, figures, badges)
            # Skip any drawing paths that belong to valid tables
            drawings = page.get_drawings()
            clusters = []
            for path in drawings:
                pr = path.get("rect")
                if not pr:
                    continue
                r = pymupdf.Rect(pr)
                if any(tb.contains(r) or (r.intersects(tb) and r.intersect(tb).get_area() > 0.4 * r.get_area()) for tb in valid_table_bboxes):
                    continue

                matched = []
                for c in clusters:
                    expanded = pymupdf.Rect(c["rect"].x0 - 15, c["rect"].y0 - 10, c["rect"].x1 + 15, c["rect"].y1 + 10)
                    if expanded.intersects(r):
                        matched.append(c)
                if not matched:
                    clusters.append({"rect": r, "count": 1})
                else:
                    first = matched[0]
                    first["rect"] = first["rect"].include_rect(r)
                    first["count"] += 1
                    for other in matched[1:]:
                        first["rect"] = first["rect"].include_rect(other["rect"])
                        first["count"] += other["count"]
                        clusters.remove(other)

            diagram_rects = []
            for c in clusters:
                r = c["rect"]
                cnt = c["count"]
                if cnt >= 5 and r.width > 20 and r.height > 18 and (r.width * r.height) < (page.rect.width * page.rect.height * 0.85):
                    clip = pymupdf.Rect(
                        max(0, r.x0 - 2),
                        max(0, r.y0 - 2),
                        min(page.rect.width, r.x1 + 2),
                        min(page.rect.height, r.y1 + 2)
                    )
                    diagram_rects.append(clip)
                    try:
                        pix = page.get_pixmap(clip=clip, dpi=288)
                        img_bytes = pix.tobytes("png")
                        slide.shapes.add_picture(
                            io.BytesIO(img_bytes),
                            Pt(clip.x0),
                            Pt(clip.y0),
                            Pt(clip.width),
                            Pt(clip.height),
                        )
                    except Exception:
                        pass

            # 3. Extract and place raster images at their exact positions (skip if inside diagram or table)
            for img_info in page.get_images():
                xref = img_info[0]
                rects = page.get_image_rects(xref)
                if not rects:
                    continue
                extracted = doc.extract_image(xref)
                if not extracted or "image" not in extracted:
                    continue
                img_bytes = extracted["image"]
                for r in rects:
                    if any(r.intersects(d) for d in diagram_rects) or any(r.intersects(tb) for tb in valid_table_bboxes):
                        continue
                    try:
                        slide.shapes.add_picture(
                            io.BytesIO(img_bytes),
                            Pt(r.x0),
                            Pt(r.y0),
                            Pt(r.width),
                            Pt(r.height),
                        )
                    except Exception:
                        pass

            # 4. Create native tables with exact column widths, row heights, and compact padding
            for t in valid_tables:
                try:
                    data = t.extract()
                    if not data:
                        continue
                    rows = len(data)
                    cols = max(len(row) for row in data)
                    if rows == 0 or cols == 0:
                        continue
                    x0, y0, x1, y1 = t.bbox
                    table_shape = slide.shapes.add_table(
                        rows, cols, Pt(x0), Pt(y0), Pt(max(10, x1 - x0)), Pt(max(10, y1 - y0))
                    )
                    table = table_shape.table

                    # Compute accurate column widths from cell geometry
                    if len(t.rows) > 0 and len(t.rows[0].cells) > 0:
                        col_w = []
                        for ci in range(cols):
                            widths = [
                                (r.cells[ci][2] - r.cells[ci][0])
                                for r in t.rows
                                if ci < len(r.cells) and r.cells[ci]
                            ]
                            col_w.append(max(widths) if widths else (x1 - x0) / cols)
                        for ci in range(min(cols, len(table.columns))):
                            table.columns[ci].width = Pt(col_w[ci])

                    # Set accurate row heights
                    for ri in range(min(rows, len(table.rows))):
                        if ri < len(t.rows) and t.rows[ri].bbox:
                            row_h = t.rows[ri].bbox[3] - t.rows[ri].bbox[1]
                            if row_h > 5:
                                table.rows[ri].height = Pt(row_h)

                    for ri, row in enumerate(data):
                        for ci, cell_text in enumerate(row):
                            if ci < len(table.columns) and ri < len(table.rows):
                                cell = table.cell(ri, ci)
                                cell.margin_left = Pt(3)
                                cell.margin_right = Pt(3)
                                cell.margin_top = Pt(2)
                                cell.margin_bottom = Pt(2)
                                cell.text = clean_xml_text(str(cell_text or ""))
                                for p_elem in cell.text_frame.paragraphs:
                                    p_elem.font.size = Pt(8.5 if rows > 6 else 9)
                                    if ri == 0:
                                        p_elem.font.bold = True
                                        p_elem.font.color.rgb = RGBColor(0xFF, 0xFF, 0xFF)
                                    else:
                                        p_elem.font.color.rgb = RGBColor(0x1E, 0x29, 0x3B)
                except Exception:
                    pass

            # 5. Extract text blocks and position text boxes precisely (suppress text inside diagrams and alt-text slivers)
            page_dict = page.get_text("dict")
            for b in page_dict.get("blocks", []):
                if b.get("type") == 0:  # text block
                    x0, y0, x1, y1 = b["bbox"]
                    b_rect = pymupdf.Rect(x0, y0, x1, y1)

                    # Skip if block is already inside an extracted table to avoid duplicate text
                    if any(tb.contains(b_rect) or (b_rect.intersects(tb) and (b_rect.intersect(tb).get_area() > 0.3 * b_rect.get_area())) for tb in valid_table_bboxes):
                        continue

                    # Skip if block is inside a protected diagram (rendered at 288 DPI)
                    if any(d.contains(b_rect) or (b_rect.intersects(d) and (b_rect.intersect(d).get_area() > 0.3 * b_rect.get_area())) for d in diagram_rects):
                        continue

                    lines = ["".join(s.get("text", "") for s in l.get("spans", [])) for l in b.get("lines", [])]
                    full_text = clean_xml_text(" ".join(lines)).strip()
                    if not full_text:
                        continue

                    w = x1 - x0
                    char_count = len(full_text)

                    # Skip diagram-attached text (alt-text or fragments within diagram vertical span)
                    is_attached = False
                    for d in diagram_rects:
                        if (d.y0 - 2 <= y0 and y1 <= d.y1 + 2) and (d.x0 - 20 <= x0 and x1 <= d.x1 + 80) and w < 80:
                            is_attached = True
                            break
                    if is_attached:
                        continue

                    # Skip pathological narrow slivers / alt-text artifacts that cause vertical cascading
                    if (w < 45 and char_count > 10) or (w < 65 and char_count > 25) or (w < 120 and char_count > 60) or (char_count / max(1, w) > 0.35 and w < 100):
                        continue
                    if w < 10:
                        continue

                    avail_w = max(10, page.rect.width - x0 - 5)
                    box_w = min(max(50, x1 - x0), avail_w)
                    box_h = max(10, y1 - y0)
                    tb = slide.shapes.add_textbox(Pt(x0), Pt(y0), Pt(box_w), Pt(box_h))
                    tf = tb.text_frame
                    tf.word_wrap = True
                    tf.margin_left = Pt(0)
                    tf.margin_top = Pt(0)
                    tf.margin_right = Pt(0)
                    tf.margin_bottom = Pt(0)

                    for li, line in enumerate(b.get("lines", [])):
                        p_elem = tf.paragraphs[0] if li == 0 else tf.add_paragraph()
                        for s in line.get("spans", []):
                            text = clean_xml_text(s.get("text", ""))
                            if not text:
                                continue
                            run = p_elem.add_run()
                            run.text = text
                            run.font.size = Pt(max(6, s.get("size", 11)))
                            c = s.get("color", 0)
                            if isinstance(c, int):
                                run.font.color.rgb = RGBColor((c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF)
                            f_name = s.get("font", "").lower()
                            flags = s.get("flags", 0)
                            run.font.bold = bool(flags & 4 or "bold" in f_name)
                            run.font.italic = bool(flags & 2 or "italic" in f_name)

            pct = int(((idx + 1) / total_pages) * 100)
            emit_progress(idx + 1, total_pages, pct, "pptx_page")

        prs.save(output_pptx)
        doc.close()

        emit_progress(total_pages, total_pages, 100, "done")
        print(json.dumps({"type": "success", "output": output_pptx}))
        return 0

    except Exception as exc:
        print(json.dumps({"type": "error", "message": str(exc)}), flush=True)
        return 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="PDFKit Advanced Office Document Reconstructor")
    parser.add_argument("--input", "-i", required=True, help="Input PDF file path")
    parser.add_argument("--output", "-o", required=True, help="Output file path (.docx or .pptx)")
    parser.add_argument("--format", "-f", default="docx", choices=["docx", "pptx"], help="Target format")
    parser.add_argument("--pages", "-p", required=False, help="Comma-separated 1-based page numbers")

    args = parser.parse_args()

    page_indices = None
    if args.pages:
        page_indices = [
            int(p.strip()) - 1
            for p in args.pages.split(",")
            if p.strip().isdigit() and int(p.strip()) > 0
        ]

    fmt = args.format.lower()
    if fmt == "pptx" or args.output.lower().endswith(".pptx"):
        sys.exit(convert_pdf_to_pptx(args.input, args.output, pages=page_indices))
    else:
        sys.exit(convert_pdf_to_docx(args.input, args.output, pages=page_indices))
