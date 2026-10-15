"""
PPTX Presentation Builder
Converts PageIR models into 100% compliant Microsoft PowerPoint (.pptx) slides.
Strictly implements:
- Valid OpenXML (strictly NO duplicate autofit tags to eliminate 'File Corrupted' errors)
- Multiple paragraphs within TextBlockIR (paras > 1) and line-end de-hyphenation
- Rich TableCellIR.spans preservation (italic/bold cell terms like *segment*, *packet*, *frame*)
- Strict slide boundary clamping on all objects (zero outward bleed)
- Beautiful professional table styling with dark gray headers, white rows, and subtle gray borders
- Strict Z-index layer order
- Clean zero-margin textboxes
"""

import io
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_CONNECTOR, MSO_SHAPE
from pptx.enum.text import MSO_ANCHOR, PP_ALIGN
from pptx.oxml.ns import qn
from pptx.util import Pt
from python_engine.common.progress import clean_xml_text
from .ir import CalloutIR, DiagramIR, ImageIR, PageIR, Rect, ShapeIR, TableIR, TextBlockIR, safe_float
from .layout import calculate_textbox_metrics, partition_block_into_paragraphs, should_dehyphenate


def _set_cell_border(cell, color_rgb=(140, 140, 140), width_pt=0.5):
    """Sets a thin border on all 4 sides of a table cell via OOXML."""
    try:
        tcPr = cell._tc.get_or_add_tcPr()
        width_emu = str(int(width_pt * 12700))
        hex_color = f"{color_rgb[0]:02X}{color_rgb[1]:02X}{color_rgb[2]:02X}"

        for border_name in ["lnL", "lnR", "lnT", "lnB"]:
            ln = tcPr.find(qn(f"a:{border_name}"))
            if ln is None:
                ln = tcPr.makeelement(qn(f"a:{border_name}"), {"w": width_emu, "cmpd": "s"})
                tcPr.append(ln)
            else:
                ln.set("w", width_emu)

            solidFill = ln.find(qn("a:solidFill"))
            if solidFill is None:
                solidFill = ln.makeelement(qn("a:solidFill"), {})
                ln.append(solidFill)

            srgbClr = solidFill.find(qn("a:srgbClr"))
            if srgbClr is None:
                srgbClr = solidFill.makeelement(qn("a:srgbClr"), {"val": hex_color})
                solidFill.append(srgbClr)
            else:
                srgbClr.set("val", hex_color)
    except Exception:
        pass


class PptxSlideBuilder:
    """Renders a single PageIR model onto a python-pptx Slide with precise geometry."""

    def __init__(self, slide, page_ir: PageIR):
        self.slide = slide
        self.page = page_ir

    def build(self) -> None:
        # 1. Render Callout / Alert background shapes (clean fill and stroke, no text inside shape)
        for callout in self.page.callouts:
            self._render_callout(callout)

        # 2. Render complex text-free diagram raster fallbacks (288 DPI non-destructive PNG)
        for diagram in self.page.diagrams:
            self._render_diagram(diagram)

        # 3. Render standalone raster images
        for img in self.page.images:
            self._render_image(img)

        # 4. Render clean vector divider lines and connectors
        for shape in self.page.shapes:
            self._render_shape(shape)

        # 5. Render native tables (with professional Cisco/Office styling and rich spans)
        for table in self.page.tables:
            self._render_table(table)

        # Collect obstacle bboxes for neighbor-aware width calculation
        obstacles: list[Rect] = [t.bbox for t in self.page.tables] + [b.bbox for b in self.page.text_blocks]

        # 6. Render editable text blocks in structured reading order on the top layer
        for block in self.page.text_blocks:
            self._render_text_block(block, obstacles)

    def _render_callout(self, callout: CalloutIR) -> None:
        try:
            # Skip empty or white-on-white footer boxes
            if not callout.bg_color or (callout.bg_color == (255, 255, 255) and not callout.border_color):
                return
            clamped = callout.bbox.clamp_to_slide(self.page.width, self.page.height)
            shape_type = MSO_SHAPE.ROUNDED_RECTANGLE if callout.is_rounded else MSO_SHAPE.RECTANGLE
            shape = self.slide.shapes.add_shape(
                shape_type,
                Pt(max(0.0, safe_float(clamped.x0))),
                Pt(max(0.0, safe_float(clamped.y0))),
                Pt(max(4.0, safe_float(clamped.width))),
                Pt(max(4.0, safe_float(clamped.height))),
            )
            shape.fill.solid()
            shape.fill.fore_color.rgb = RGBColor(*callout.bg_color)
            if callout.border_color:
                shape.line.color.rgb = RGBColor(*callout.border_color)
                shape.line.width = Pt(max(0.25, safe_float(callout.border_width, 0.75)))
            else:
                shape.line.fill.background()

            # Render left accent bar if present (Kural 2)
            if callout.accent_bar_rect:
                acc_clamped = callout.accent_bar_rect.clamp_to_slide(self.page.width, self.page.height)
                acc_shape = self.slide.shapes.add_shape(
                    MSO_SHAPE.RECTANGLE,
                    Pt(max(0.0, safe_float(acc_clamped.x0))),
                    Pt(max(0.0, safe_float(acc_clamped.y0))),
                    Pt(max(1.5, safe_float(acc_clamped.width))),
                    Pt(max(4.0, safe_float(acc_clamped.height))),
                )
                acc_shape.fill.solid()
                acc_color = callout.accent_bar_color or (90, 105, 120)
                acc_shape.fill.fore_color.rgb = RGBColor(*acc_color)
                acc_shape.line.fill.background()
        except Exception:
            pass

    def _render_diagram(self, diagram: DiagramIR) -> None:
        try:
            clamped = diagram.bbox.clamp_to_slide(self.page.width, self.page.height)
            self.slide.shapes.add_picture(
                io.BytesIO(diagram.image_bytes),
                Pt(max(0.0, safe_float(clamped.x0))),
                Pt(max(0.0, safe_float(clamped.y0))),
                Pt(max(4.0, safe_float(clamped.width))),
                Pt(max(4.0, safe_float(clamped.height))),
            )
        except Exception:
            pass

    def _render_image(self, img: ImageIR) -> None:
        try:
            clamped = img.bbox.clamp_to_slide(self.page.width, self.page.height)
            self.slide.shapes.add_picture(
                io.BytesIO(img.image_bytes),
                Pt(max(0.0, safe_float(clamped.x0))),
                Pt(max(0.0, safe_float(clamped.y0))),
                Pt(max(4.0, safe_float(clamped.width))),
                Pt(max(4.0, safe_float(clamped.height))),
            )
        except Exception:
            pass

    def _render_shape(self, shape: ShapeIR) -> None:
        try:
            if shape.shape_type == "line" and shape.p1 and shape.p2:
                p1_x = max(0.0, min(self.page.width, safe_float(shape.p1[0])))
                p1_y = max(0.0, min(self.page.height, safe_float(shape.p1[1])))
                p2_x = max(0.0, min(self.page.width, safe_float(shape.p2[0])))
                p2_y = max(0.0, min(self.page.height, safe_float(shape.p2[1])))
                conn = self.slide.shapes.add_connector(
                    MSO_CONNECTOR.STRAIGHT,
                    Pt(p1_x),
                    Pt(p1_y),
                    Pt(p2_x),
                    Pt(p2_y),
                )
                conn.line.width = Pt(max(0.25, safe_float(shape.stroke_width, 0.75)))
                if shape.stroke_color:
                    conn.line.color.rgb = RGBColor(*shape.stroke_color)
        except Exception:
            pass

    def _render_table(self, table_ir: TableIR) -> None:
        try:
            t_clamped = table_ir.bbox.clamp_to_slide(self.page.width, self.page.height)
            shape = self.slide.shapes.add_table(
                table_ir.rows,
                table_ir.cols,
                Pt(max(0.0, safe_float(t_clamped.x0))),
                Pt(max(0.0, safe_float(t_clamped.y0))),
                Pt(max(4.0, safe_float(t_clamped.width))),
                Pt(max(4.0, safe_float(t_clamped.height))),
            )
            table = shape.table

            for ci in range(min(table_ir.cols, len(table.columns))):
                pdf_col_w = table_ir.col_widths[ci] if ci < len(table_ir.col_widths) else (table_ir.bbox.width / table_ir.cols)
                table.columns[ci].width = Pt(max(10.0, safe_float(pdf_col_w, 50.0)))

            for ri in range(min(table_ir.rows, len(table.rows))):
                pdf_row_h = table_ir.row_heights[ri] if ri < len(table_ir.row_heights) else (table_ir.bbox.height / table_ir.rows)
                table.rows[ri].height = Pt(max(8.0, safe_float(pdf_row_h, 18.0)))

            header_bg = table_ir.header_bg_rgb or (88, 89, 91)

            for ri, row in enumerate(table_ir.cells):
                is_hdr = (ri == 0)
                for ci, cell_ir in enumerate(row):
                    if ri < len(table.rows) and ci < len(table.columns):
                        cell = table.cell(ri, ci)
                        cell.margin_left = Pt(2.0)
                        cell.margin_right = Pt(2.0)
                        cell.margin_top = Pt(0.5)
                        cell.margin_bottom = Pt(0.5)
                        cell.vertical_anchor = MSO_ANCHOR.MIDDLE

                        # Professional styling: Header is dark gray with white text, data rows are white
                        cell.fill.solid()
                        if is_hdr:
                            cell.fill.fore_color.rgb = RGBColor(*header_bg)
                        else:
                            cell.fill.fore_color.rgb = RGBColor(255, 255, 255)

                        # Cell border formatting
                        _set_cell_border(cell, color_rgb=(140, 140, 140), width_pt=0.5)

                        f_sz = min(8.0, max(6.5, safe_float(cell_ir.font_size, 9.5) - 0.8))

                        # Clean cell bodyPr
                        try:
                            bodyPr = cell.text_frame._txBody.find(qn("a:bodyPr"))
                            if bodyPr is not None:
                                bodyPr.set("spcFirstLastPara", "0")
                        except Exception:
                            pass

                        # Render rich cell content preserving italic/bold styles (e.g. *segment*, *packet*, *frame*)
                        if cell_ir.spans:
                            cell.text = ""
                            p_elem = cell.text_frame.paragraphs[0]
                            p_elem.space_before = Pt(0.0)
                            p_elem.space_after = Pt(0.0)
                            p_elem.line_spacing = 0.95
                            PUNCT_CHARS = (",", ".", ";", ":", "!", "?", ")", "]", "”", "’", "*", "%")
                            for s_idx, span in enumerate(cell_ir.spans):
                                txt = clean_xml_text(span.text)
                                if not txt:
                                    continue
                                if s_idx > 0 and not txt.startswith(" ") and not cell_ir.spans[s_idx - 1].text.endswith(" "):
                                    # DÜZELTME 3: Noktalama işareti öncesi boşluk yasağı ("segment" + ", " -> "segment, ")
                                    if not txt.lstrip().startswith(PUNCT_CHARS):
                                        txt = " " + txt
                                run = p_elem.add_run()
                                run.text = txt
                                run.font.size = Pt(f_sz)
                                run.font.name = span.clean_font_name or "Calibri"
                                run.font.bold = is_hdr or bool(span.font.bold)
                                run.font.italic = bool(span.font.italic)
                                run.font.color.rgb = RGBColor(255, 255, 255) if is_hdr else RGBColor(35, 31, 32)
                        else:
                            cell.text = cell_ir.text
                            p_elem = cell.text_frame.paragraphs[0]
                            p_elem.space_before = Pt(0.0)
                            p_elem.space_after = Pt(0.0)
                            p_elem.line_spacing = 0.95
                            p_elem.font.size = Pt(f_sz)
                            p_elem.font.name = cell_ir.font.name or "Calibri"
                            p_elem.font.bold = is_hdr
                            p_elem.font.italic = bool(cell_ir.font.italic)
                            p_elem.font.color.rgb = RGBColor(255, 255, 255) if is_hdr else RGBColor(35, 31, 32)
        except Exception:
            pass

    def _render_text_block(self, block: TextBlockIR, obstacles: list[Rect]) -> None:
        try:
            min_x, min_y, w, h = calculate_textbox_metrics(block, obstacles, self.page.width)
            box_rect = Rect(min_x, min_y, min_x + w, min_y + h).clamp_to_slide(self.page.width, self.page.height)
            tb = self.slide.shapes.add_textbox(
                Pt(max(0.0, safe_float(box_rect.x0))),
                Pt(max(0.0, safe_float(box_rect.y0))),
                Pt(max(4.0, safe_float(box_rect.width))),
                Pt(max(4.0, safe_float(box_rect.height))),
            )
            tf = tb.text_frame

            # KURAL: Genişliği page_w * 0.25'ten büyük olan veya karakter sayısı > 25 olan kutularda word_wrap = True
            # Monospace kod bloklarında veya kapsayıcı içi metinlerde word_wrap = False (girinti ve komut satırı koruması)
            text_content = block.text.strip()
            is_code_block = (block.parent_container_rect is not None)
            if not is_code_block:
                for l in block.lines:
                    for s in l.spans:
                        fn = (s.clean_font_name or "").lower()
                        if "courier" in fn or "mono" in fn or "consolas" in fn:
                            is_code_block = True
                            break
                    if is_code_block:
                        break
            if is_code_block:
                tf.word_wrap = False
            elif box_rect.width > self.page.width * 0.25 or len(text_content) > 25:
                tf.word_wrap = True
            else:
                tf.word_wrap = False

            # Standard python-pptx API margin reset
            tf.margin_left = Pt(0.0)
            tf.margin_right = Pt(0.0)
            tf.margin_top = Pt(0.0)
            tf.margin_bottom = Pt(0.0)

            # KESİN YASAK: Asla <a:normAutofit> veya <a:spAutoFit> append etme!
            try:
                bodyPr = tf._txBody.find(qn("a:bodyPr"))
                if bodyPr is not None:
                    bodyPr.set("spcFirstLastPara", "0")
                    bodyPr.set("lIns", "0")
                    bodyPr.set("tIns", "0")
                    bodyPr.set("rIns", "0")
                    bodyPr.set("bIns", "0")
            except Exception:
                pass

            # Partition lines into logical paragraphs (with de-hyphenation and spacing)
            paragraphs = partition_block_into_paragraphs(block)

            for pi, (para_lines, space_before) in enumerate(paragraphs):
                p_elem = tf.paragraphs[0] if pi == 0 else tf.add_paragraph()
                p_elem.space_before = Pt(space_before)
                p_elem.space_after = Pt(0.0)

                if block.alignment == "CENTER":
                    p_elem.alignment = PP_ALIGN.CENTER
                elif block.alignment == "RIGHT":
                    p_elem.alignment = PP_ALIGN.RIGHT
                else:
                    p_elem.alignment = PP_ALIGN.LEFT

                if block.exact_line_spacing_pt:
                    p_elem.line_spacing = Pt(safe_float(block.exact_line_spacing_pt))

                # Flow lines within this paragraph with de-hyphenation
                for li, line in enumerate(para_lines):
                    for si, s in enumerate(line.spans):
                        text = clean_xml_text(s.text)
                        if not text:
                            continue

                        # Check de-hyphenation if this is the last span of current line and there's a next line in this paragraph
                        if si == len(line.spans) - 1 and li < len(para_lines) - 1:
                            next_line = para_lines[li + 1]
                            if next_line.spans:
                                next_text = clean_xml_text(next_line.spans[0].text)
                                is_dehyphen, clean_txt = should_dehyphenate(text, next_text)
                                if is_dehyphen:
                                    text = clean_txt

                        # If starting a new line in a flowing paragraph:
                        if li > 0 and si == 0:
                            prev_was_dehyphen = False
                            prev_line = para_lines[li - 1]
                            if prev_line.spans:
                                prev_s_text = clean_xml_text(prev_line.spans[-1].text)
                                is_dh, _ = should_dehyphenate(prev_s_text, text)
                                prev_was_dehyphen = is_dh

                            if not prev_was_dehyphen:
                                prev_txt = prev_line.text.rstrip()
                                if not prev_txt.endswith(("-", "—", "–", "/")) and not text.lstrip().startswith(
                                    (" ", ",", ".", ";", ":", "!", "?", ")", "]", "”", "’", '"', "*", "%", "-", "—", "–")
                                ):
                                    text = " " + text

                        run = p_elem.add_run()
                        run.text = text
                        # Paragraf içi font boyutu tutarlılığı (dominant_font_size koruması)
                        if abs(s.visual_font_size - block.dominant_font_size) <= 1.0:
                            f_size = block.dominant_font_size
                        else:
                            f_size = s.visual_font_size
                        run.font.size = Pt(safe_float(f_size, 10.0))
                        run.font.name = s.clean_font_name or "Calibri"
                        run.font.bold = bool(s.font.bold)
                        run.font.italic = bool(s.font.italic)
                        if s.font.color:
                            run.font.color.rgb = RGBColor(*s.font.color)

                        if s.char_spacing_spc != 0:
                            try:
                                rPr = run._r.get_or_add_rPr()
                                rPr.set("spc", str(int(s.char_spacing_spc)))
                            except Exception:
                                pass
        except Exception:
            pass
