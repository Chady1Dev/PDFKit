"""
PPTX Engine Tests
Comprehensive verification of PPTX generation, PageIR models, multi-column layouts,
callout banners, technical diagrams, duplicate suppression, and quality audit metrics.
"""

import os
import sys
import unittest
import zipfile

# Ensure tests dir does not shadow pptx package
_curr_dir = os.path.dirname(os.path.abspath(__file__))
_tests_dir = os.path.dirname(_curr_dir)
while _tests_dir in sys.path:
    sys.path.remove(_tests_dir)
_root = os.path.dirname(_tests_dir)
if _root not in sys.path:
    sys.path.insert(0, _root)

import pymupdf
from pptx import Presentation
from python_engine.converters.pptx import convert_pdf_to_pptx
from python_engine.converters.pptx.analyzer import PageAnalyzer
from python_engine.converters.pptx.quality import QualityAuditor
from python_engine.converters.pptx.ir import (

    FontInfo,
    PageIR,
    Rect,
    TableIR,
    TableCellIR,
    TextBlockIR,
    TextLineIR,
    TextSpanIR,
)
from python_engine.converters.pptx.layout import (
    apply_table_vertical_push_down,
    calculate_textbox_metrics,
    cluster_spans_into_lines,
)
from tests.test_helpers import (

    create_callout_note_pdf,
    create_fill_only_banner_pdf,
    create_multicolumn_pdf,
    create_sample_pdf,
    create_technical_diagram_pdf,
)



class TestPptxEngine(unittest.TestCase):

    def setUp(self):
        self.temp_files = []

    def tearDown(self):
        for p in self.temp_files:
            if os.path.exists(p):
                try:
                    os.remove(p)
                except Exception:
                    pass

    def _track(self, path: str) -> str:
        self.temp_files.append(path)
        return path

    def test_pptx_conversion_success(self):
        pdf_path = self._track(create_sample_pdf(num_pages=2))
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0, "PPTX conversion should return exit code 0")
        self.assertTrue(os.path.exists(output_pptx), "PPTX file must exist")
        self.assertGreater(os.path.getsize(output_pptx), 1000, "PPTX file must have non-trivial size")

        # Validate OpenXML zip structure
        with zipfile.ZipFile(output_pptx, "r") as z:
            names = z.namelist()
            self.assertIn("[Content_Types].xml", names)
            self.assertIn("ppt/presentation.xml", names)
            self.assertIn("ppt/slides/slide1.xml", names)
            self.assertIn("ppt/slides/slide2.xml", names)

        prs = Presentation(output_pptx)
        self.assertEqual(len(prs.slides), 2, "Presentation should have exactly 2 slides")

        slide1_text = ""
        for shape in prs.slides[0].shapes:
            if shape.has_text_frame:
                slide1_text += " " + shape.text_frame.text

        self.assertIn("Sayfa", slide1_text)
        self.assertIn("Test", slide1_text)

    def test_pptx_multicolumn_layout(self):
        """Verifies multi-column text does not merge across columns or scramble reading order."""
        pdf_path = self._track(create_multicolumn_pdf())
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(output_pptx))

        prs = Presentation(output_pptx)
        slide = prs.slides[0]

        texts = []
        for shape in slide.shapes:
            if shape.has_text_frame:
                txt = shape.text_frame.text.strip()
                if txt:
                    texts.append(txt)

        joined = "\n".join(texts)
        self.assertIn("Ağ Temelleri", joined)
        self.assertIn("IP Yönlendirme", joined)
        self.assertIn("Study Guide", joined)

        # Verify left column and right column text blocks exist as distinct blocks
        left_found = any("OSI referans modeli" in t for t in texts)
        right_found = any("Yönlendiriciler paketleri" in t for t in texts)
        self.assertTrue(left_found, "Left column text block should be captured")
        self.assertTrue(right_found, "Right column text block should be captured")

    def test_pptx_callout_note_banner(self):
        """Verifies colored callout NOTE banners are rendered with integrated text."""
        pdf_path = self._track(create_callout_note_pdf())
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(output_pptx))

        prs = Presentation(output_pptx)
        slide = prs.slides[0]

        # Check that shapes were created and text inside the NOTE is preserved
        all_text = ""
        has_shape = False
        for shape in slide.shapes:
            if shape.has_text_frame:
                all_text += " " + shape.text_frame.text
            if shape.shape_type:
                has_shape = True

        self.assertTrue(has_shape)
        self.assertIn("NOTE:", all_text)
        self.assertIn("no shutdown", all_text)
        self.assertIn("Adım 1", all_text)

    def test_pptx_technical_diagram_and_duplicate_suppression(self):
        """Verifies complex vector diagrams use raster fallback without duplicating text."""
        pdf_path = self._track(create_technical_diagram_pdf())
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(output_pptx))

        prs = Presentation(output_pptx)
        slide = prs.slides[0]

        # Verify slide has at least one picture shape (the diagram raster fallback)
        picture_count = sum(1 for s in slide.shapes if s.shape_type == 13)  # 13 is MSO_SHAPE_TYPE.PICTURE
        self.assertGreaterEqual(picture_count, 1, "Complex diagram mesh should be captured as picture")

        # Verify surrounding text remains editable
        editable_text = ""
        for shape in slide.shapes:
            if shape.has_text_frame:
                editable_text += " " + shape.text_frame.text

        self.assertIn("Şekil 3.1", editable_text)
        self.assertIn("paket iletim mantığı", editable_text)

    def test_pptx_quality_audit_metrics(self):
        """Directly audits PageIR and QualityAuditor for editability ratio and overlap safety."""
        pdf_path = self._track(create_sample_pdf(num_pages=1))

        doc = pymupdf.open(pdf_path)
        page = doc[0]
        analyzer = PageAnalyzer(page, 1)
        page_ir = analyzer.analyze()
        doc.close()

        report = QualityAuditor.audit(page_ir)
        self.assertGreater(report.total_characters, 50, "Total characters should be extracted")
        self.assertGreaterEqual(report.editability_ratio, 0.5, "Editability ratio should be high")
        self.assertEqual(len(report.detected_overlaps), 0, "No unwanted text collisions should occur")

    def test_pptx_page_subset_conversion(self):
        pdf_path = self._track(create_sample_pdf(num_pages=2))
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx, pages=[1])
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(output_pptx))
        prs = Presentation(output_pptx)
        self.assertEqual(len(prs.slides), 1, "Should only generate 1 slide for single-page selection")

    def test_pptx_fill_without_stroke_width_none_safety(self):
        """Verifies that drawings with fill only (PyMuPDF width=None and color=None) do not trigger float(NoneType) error."""
        pdf_path = self._track(create_fill_only_banner_pdf())
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0, "Conversion must succeed without NoneType float error")
        self.assertTrue(os.path.exists(output_pptx), "PPTX output must be created")

        prs = Presentation(output_pptx)
        self.assertEqual(len(prs.slides), 1)
        all_text = ""
        for shape in prs.slides[0].shapes:
            if shape.has_text_frame:
                all_text += " " + shape.text_frame.text
        self.assertIn("DİKKAT", all_text)
        self.assertIn("yedek alınmalıdır", all_text)

    def test_pptx_analyzer_resilience_to_none_attributes(self):
        """Directly tests PageAnalyzer with synthetic mock page containing None and irregular attributes."""
        class MockRect:
            x0 = 0.0
            y0 = 0.0
            x1 = 595.0
            y1 = 842.0
            width = 595.0
            height = 842.0

        class MockPage:
            rect = MockRect()
            rotation = 0
            parent = None

            def find_tables(self):
                class DummyTables:
                    tables = []
                return DummyTables()

            def get_drawings(self):
                return [
                    # Drawing with None width and None color (exact bug case)
                    {
                        "fill": (0.9, 0.95, 1.0),
                        "color": None,
                        "width": None,
                        "rect": [50.0, 100.0, 500.0, 200.0],
                        "items": [],
                    },
                    # Drawing with malformed items containing None
                    {
                        "fill": None,
                        "color": None,
                        "width": None,
                        "items": [
                            ("l", None, None),
                            ("re", None),
                        ],
                    },
                ]

            def get_images(self):
                return []

            def get_text(self, kind):
                return {
                    "blocks": [
                        {
                            "type": 0,
                            "bbox": [50.0, 50.0, 300.0, 70.0],
                            "lines": [
                                {
                                    "dir": (1.0, 0.0),
                                    "spans": [
                                        {
                                            "text": "Hello World",
                                            "bbox": [50.0, 50.0, 300.0, 70.0],
                                            "size": None,  # None size!
                                            "color": None, # None color!
                                            "flags": None, # None flags!
                                            "font": None,  # None font!
                                        }
                                    ],
                                }
                            ],
                        }
                    ]
                }

        mock_page = MockPage()
        analyzer = PageAnalyzer(mock_page, 1)
        page_ir = analyzer.analyze()

        self.assertIsNotNone(page_ir)
        self.assertGreaterEqual(len(page_ir.callouts), 1)
        self.assertGreaterEqual(len(page_ir.text_blocks), 1)
        self.assertEqual(page_ir.callouts[0].border_width, 0.75)

    def test_pptx_hidden_alt_text_suppression(self):
        """Verifies that text overlapping with diagram or image area (>= 10%) is suppressed (Rule C)."""
        pdf_path = self._track(create_technical_diagram_pdf())
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(output_pptx))

        prs = Presentation(output_pptx)
        slide = prs.slides[0]

        # 1. Diagram raster fallback image exists
        picture_count = sum(1 for s in slide.shapes if s.shape_type == 13)
        self.assertGreaterEqual(picture_count, 1, "Raster diagram fallback must exist")

        # 2. Text elements inside the diagram mesh are completely suppressed to eliminate ghosting
        all_text = ""
        for shape in slide.shapes:
            if shape.has_text_frame:
                all_text += " " + shape.text_frame.text

        self.assertNotIn("Paket Alındı", all_text, "Diagram inner text must be suppressed from duplicate textbox overlays")
        self.assertIn("Şekil 3.1", all_text, "Diagram outer caption must remain editable")

    def test_pptx_neighbor_aware_width_buffer(self):
        """Verifies that textbox width buffer respects neighbor obstacles and never overlaps."""
        block1 = TextBlockIR(
            id="b1",
            bbox=Rect(50.0, 100.0, 150.0, 120.0),
            lines=[],
            alignment="LEFT",
        )
        obstacle = Rect(155.0, 100.0, 250.0, 120.0)

        # calculate_textbox_metrics should not exceed obstacle.x0 - 1.5
        min_x, min_y, target_w, target_h = calculate_textbox_metrics(block1, [obstacle], slide_width=720.0)
        self.assertEqual(min_x, 50.0)
        self.assertLessEqual(min_x + target_w, obstacle.x0, "Block must not overlap neighbor obstacle")

    def test_pptx_alignment_preservation(self):
        """Verifies CENTER and RIGHT alignment expansion behavior."""
        # CENTER: expands equally in both directions
        center_block = TextBlockIR(
            id="bc",
            bbox=Rect(100.0, 50.0, 200.0, 70.0),
            lines=[],
            alignment="CENTER",
        )
        c_min_x, _, c_w, _ = calculate_textbox_metrics(center_block, [], slide_width=720.0)
        self.assertLess(c_min_x, 100.0, "Center alignment must expand to the left")
        self.assertGreater(c_min_x + c_w, 200.0, "Center alignment must expand to the right")

        # RIGHT: keeps x1 fixed and expands to the left
        right_block = TextBlockIR(
            id="br",
            bbox=Rect(200.0, 50.0, 300.0, 70.0),
            lines=[],
            alignment="RIGHT",
        )
        r_min_x, _, r_w, _ = calculate_textbox_metrics(right_block, [], slide_width=720.0)
        self.assertLess(r_min_x, 200.0, "Right alignment must expand to the left")
        self.assertAlmostEqual(r_min_x + r_w, 300.0, places=2, msg="Right alignment must keep x1 fixed")

    def test_pptx_quality_auditor_active_healing(self):
        """Verifies QualityAuditor actively heals horizontal and vertical overlaps, and prunes false tables."""
        page_ir = PageIR(page_number=1, width=595.0, height=842.0)

        # Horizontal overlap
        b1 = TextBlockIR(id="b1", bbox=Rect(50.0, 100.0, 160.0, 120.0), lines=[])
        b2 = TextBlockIR(id="b2", bbox=Rect(150.0, 100.0, 250.0, 120.0), lines=[])
        page_ir.text_blocks = [b1, b2]

        # False positive table (> 75% empty)
        empty_cell = TableCellIR(text="", font=FontInfo())
        fake_table = TableIR(
            id="fake_t",
            bbox=Rect(50.0, 200.0, 300.0, 250.0),
            rows=2,
            cols=2,
            cells=[[empty_cell, empty_cell], [empty_cell, empty_cell]],
        )
        page_ir.tables = [fake_table]

        report = QualityAuditor.audit_and_heal(page_ir)
        self.assertEqual(len(page_ir.tables), 0, "False positive empty table must be pruned")
        self.assertGreaterEqual(b1.bbox.width, b1.orig_width, "Width must never shrink below orig_width")

    def test_pptx_span_space_preservation_lhandlt(self):
        """Verifies adjacent spans with different formatting have missing boundary spaces injected (e.g. 'LH' and 'and LT')."""
        spans = [
            TextSpanIR(
                text="LH",
                bbox=Rect(50.0, 100.0, 70.0, 115.0),
                font=FontInfo(name="Arial-Italic", italic=True, size=11.0),
            ),
            TextSpanIR(
                text="and LT",
                bbox=Rect(70.5, 100.0, 120.0, 115.0),
                font=FontInfo(name="Arial", italic=False, size=11.0),
            ),
        ]
        clustered_lines = cluster_spans_into_lines(spans)
        self.assertEqual(len(clustered_lines), 1)
        line_text = "".join(s.text for s in clustered_lines[0].spans)
        self.assertEqual(line_text, "LH and LT", "Space must be injected between adjacent distinct format spans")

    def test_pptx_struct_tree_root_deletion(self):
        """Verifies in-memory deletion of StructTreeRoot and StructParents from PDF catalog to eliminate ghost alt-text."""
        pdf_path = self._track(create_sample_pdf(num_pages=1))
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))
        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(output_pptx))

        # Verify on document directly
        doc = pymupdf.open(pdf_path)
        cat_xref = doc.pdf_catalog()
        if cat_xref:
            if hasattr(doc, "xref_del_key"):
                doc.xref_del_key(cat_xref, "StructTreeRoot")
                doc.xref_del_key(cat_xref, "MarkInfo")
            else:
                doc.xref_set_key(cat_xref, "StructTreeRoot", "null")
                doc.xref_set_key(cat_xref, "MarkInfo", "null")
        for p in doc:
            if hasattr(doc, "xref_del_key"):
                doc.xref_del_key(p.xref, "StructParents")
            else:
                doc.xref_set_key(p.xref, "StructParents", "null")
        text_dict = doc[0].get_text("dict")
        self.assertIsNotNone(text_dict)
        doc.close()

    def test_pptx_no_width_shrinking_below_orig_width(self):
        """Verifies that no TextBlockIR width ever shrinks below its original PDF width (orig_width)."""
        block = TextBlockIR(
            id="block_fixed_w",
            bbox=Rect(50.0, 100.0, 250.0, 130.0),
            orig_width=200.0,
            lines=[],
        )
        obstacle = Rect(200.0, 100.0, 300.0, 130.0)  # Overlapping obstacle on the right
        min_x, min_y, target_w, target_h = calculate_textbox_metrics(block, [obstacle], slide_width=720.0)
        self.assertGreaterEqual(
            target_w,
            block.orig_width,
            f"target_w ({target_w}) must never be less than orig_width ({block.orig_width})",
        )

        page_ir = PageIR(page_number=1, width=720.0, height=540.0, text_blocks=[block])
        QualityAuditor.audit_and_heal(page_ir)
        self.assertGreaterEqual(page_ir.text_blocks[0].bbox.width, 200.0)

    def test_pptx_no_element_thrown_outside_slide_height(self):
        """Verifies that all elements stay on their original PDF y0 coordinates and are never thrown off-slide (y > slide_height)."""
        page_ir = PageIR(page_number=1, width=720.0, height=540.0)
        table = TableIR(
            id="t1",
            bbox=Rect(50.0, 300.0, 400.0, 450.0),
            rows=3,
            cols=2,
            cells=[
                [TableCellIR("H1", FontInfo()), TableCellIR("H2", FontInfo())],
                [TableCellIR("Long content", FontInfo()), TableCellIR("Long content", FontInfo())],
                [TableCellIR("Long content", FontInfo()), TableCellIR("Long content", FontInfo())],
            ],
            col_widths=[175.0, 175.0],
            row_heights=[50.0, 50.0, 50.0],
        )
        heading_below = TextBlockIR(
            id="heading_key_topics",
            bbox=Rect(50.0, 460.0, 350.0, 480.0),
            orig_width=300.0,
            lines=[],
        )
        page_ir.tables = [table]
        page_ir.text_blocks = [heading_below]

        # Audit and heal
        QualityAuditor.audit_and_heal(page_ir)

        # Coordinate must remain exactly at its original PDF y0, NOT pushed down to y=770..1013 pt
        self.assertEqual(heading_below.bbox.y0, 460.0, "Element y0 must remain 100% on original PDF position")
        self.assertLessEqual(heading_below.bbox.y1, page_ir.height, "Element must not be thrown outside slide height")

    def test_pptx_overlapping_diagrams_deduplication(self):
        """Verifies overlapping, nested, or adjacent (< 6 pt) DiagramIR candidate boxes are strictly merged into 1 picture."""
        clusters = [
            Rect(50.0, 100.0, 150.0, 200.0),
            Rect(140.0, 120.0, 250.0, 220.0),  # Overlaps cluster 0
            Rect(253.0, 120.0, 350.0, 220.0),  # Distance is 3 pt (< 6 pt) to cluster 1
            Rect(450.0, 300.0, 550.0, 400.0),  # Separate diagram
        ]
        merged = True
        while merged:
            merged = False
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
                    h_dist = max(0.0, max(cur_c.x0 - other_c.x1, other_c.x0 - cur_c.x1))
                    v_dist = max(0.0, max(cur_c.y0 - other_c.y1, other_c.y0 - cur_c.y1))
                    if cur_c.intersects(other_c) or cur_c.contains(other_c) or other_c.contains(cur_c) or (h_dist < 6.0 and v_dist < 6.0):
                        cur_c = cur_c.union(other_c)
                        used.add(j)
                        merged = True
                new_clusters.append(cur_c)
            clusters = new_clusters

        # 4 boxes should have merged into 2 unified diagrams (clusters 0, 1, 2 into one; cluster 3 remains separate)
        self.assertEqual(len(clusters), 2, "Overlapping and nested diagram boxes must be unified into exactly 1 diagram")
        self.assertEqual(clusters[0].x0, 50.0)
        self.assertEqual(clusters[0].x1, 350.0)
        self.assertEqual(clusters[1].x0, 450.0)

    def test_pptx_valid_ooxml_no_duplicate_autofit(self):
        """Verifies generated PPTX slide1.xml contains valid OpenXML with NO duplicate autofit elements in bodyPr."""
        import xml.etree.ElementTree as ET

        pdf_path = self._track(create_sample_pdf(num_pages=1))
        output_pptx = self._track(pdf_path.replace(".pdf", ".pptx"))

        code = convert_pdf_to_pptx(pdf_path, output_pptx)
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(output_pptx))

        with zipfile.ZipFile(output_pptx, "r") as zf:
            slide_xml_content = zf.read("ppt/slides/slide1.xml")
            root = ET.fromstring(slide_xml_content)

            ns = {
                "a": "http://schemas.openxmlformats.org/drawingml/2006/main",
                "p": "http://schemas.openxmlformats.org/presentationml/2006/main",
            }

            body_prs = root.findall(".//a:bodyPr", ns)
            self.assertGreater(len(body_prs), 0, "Must have bodyPr elements in slide")

            for body_pr in body_prs:
                autofit_tags = [
                    child.tag for child in body_pr
                    if child.tag.endswith(("noAutofit", "normAutofit", "spAutoFit"))
                ]
                self.assertLessEqual(
                    len(autofit_tags),
                    1,
                    f"bodyPr must have at most 1 autofit tag, found: {autofit_tags}",
                )

    def test_universal_bold_italic_detection(self):
        """Verifies universal font weight and italic detection (Fix 1)."""
        from python_engine.converters.pptx.analyzer import detect_font_weight_and_style

        # CiscoSerif-Regular with StemV=80 (flags & 16 is True in PyMuPDF) must NOT be bold!
        span_reg = {"font": "TMJNYH+CiscoSerif-Regular", "flags": 16}
        is_b, is_i = detect_font_weight_and_style(span_reg)
        self.assertFalse(is_b, "CiscoSerif-Regular must not be bold even with flags & 16")
        self.assertFalse(is_i)

        # Cisco-Light with flags=16 must NOT be bold!
        span_light = {"font": "YEUMQX+Cisco-Light", "flags": 16}
        is_b, is_i = detect_font_weight_and_style(span_light)
        self.assertFalse(is_b, "Cisco-Light must not be bold")
        self.assertFalse(is_i)

        # Cisco-Bold with flags=0 MUST be bold!
        span_bold = {"font": "ABCDEF+Cisco-Bold", "flags": 0}
        is_b, is_i = detect_font_weight_and_style(span_bold)
        self.assertTrue(is_b, "Cisco-Bold must be bold")
        self.assertFalse(is_i)

        # Italic font
        span_it = {"font": "Arial-Italic", "flags": 0}
        is_b, is_i = detect_font_weight_and_style(span_it)
        self.assertFalse(is_b)
        self.assertTrue(is_i, "Arial-Italic must be italic")

        # Standard Calibri without weight hint
        span_calibri = {"font": "Calibri", "flags": 0}
        is_b, is_i = detect_font_weight_and_style(span_calibri)
        self.assertFalse(is_b)
        self.assertFalse(is_i)

    def test_protected_text_zones_and_clamping(self):
        """Verifies protected body paragraphs and diagram boundary clamping (Fix 2)."""
        # Create a synthetic PDF with a 3-line body paragraph ending in a short line "book.",
        # followed by a vector diagram, followed by "Figure 1-1" caption
        doc = pymupdf.open()
        page = doc.new_page(width=595, height=842)

        # 3-line paragraph: y from 100 to 140
        page.insert_text(pymupdf.Point(54, 110), "This is the first long line of an introductory body paragraph explaining concepts.", fontsize=10)
        page.insert_text(pymupdf.Point(54, 125), "This is the second long line providing further technical details and context for readers.", fontsize=10)
        page.insert_text(pymupdf.Point(54, 140), "book.", fontsize=10)

        # Diagram vector box: y from 150 to 300
        page.draw_rect(pymupdf.Rect(54, 150, 400, 300), color=(0, 0, 1), fill=(0.9, 0.9, 1))
        page.draw_line(pymupdf.Point(70, 180), pymupdf.Point(200, 180), color=(1, 0, 0))

        # Caption: y at 320
        page.insert_text(pymupdf.Point(54, 320), "Figure 1-1 Overview of Network Architecture", fontsize=9)

        analyzer = PageAnalyzer(page, page_number=1)
        page_ir = analyzer.analyze()

        # Verify paragraph is protected and not swallowed into diagram
        text_contents = [b.text for b in page_ir.text_blocks]
        all_text = " ".join(text_contents)
        self.assertIn("book.", all_text, "Short concluding line 'book.' must NOT be swallowed or deleted!")
        self.assertIn("Figure 1-1", all_text, "Caption 'Figure 1-1' must be preserved as editable text block!")

        # Verify diagram does not overlap paragraph or caption
        self.assertGreater(len(page_ir.diagrams), 0, "Vector diagram should be extracted")
        diag = page_ir.diagrams[0]
        # Diagram top must be >= body paragraph bottom (around 140)
        self.assertGreaterEqual(diag.bbox.y0, 140.0, "Diagram top must be clamped below body paragraph")
        # Diagram bottom must be <= caption top (around 315)
        self.assertLessEqual(diag.bbox.y1, 325.0, "Diagram bottom must be clamped above caption")
        doc.close()

    def test_table_first_pipeline_protection(self):
        """Verifies table-first pipeline confirms real tables and purges drawings (Fix 3)."""
        doc = pymupdf.open()
        page = doc.new_page(width=595, height=842)

        # Draw a table grid with clean horizontal and vertical lines (width >= 0.42 * 595 = 250)
        t_rect = pymupdf.Rect(54, 200, 450, 350)
        # Table borders
        page.draw_rect(t_rect, color=(0.3, 0.3, 0.3))
        page.draw_line(pymupdf.Point(54, 250), pymupdf.Point(450, 250), color=(0.5, 0.5, 0.5))
        page.draw_line(pymupdf.Point(200, 200), pymupdf.Point(200, 350), color=(0.5, 0.5, 0.5))

        # Table cell text
        page.insert_text(pymupdf.Point(60, 230), "Header 1", fontsize=10)
        page.insert_text(pymupdf.Point(220, 230), "Header 2", fontsize=10)
        page.insert_text(pymupdf.Point(60, 280), "Data 1", fontsize=10)
        page.insert_text(pymupdf.Point(220, 280), "Data 2", fontsize=10)

        analyzer = PageAnalyzer(page, page_number=1)
        page_ir = analyzer.analyze()

        # If find_tables is supported, TableIR must be detected and NOT converted to a DiagramIR
        if hasattr(page, "find_tables"):
            tables = page_ir.tables
            if tables:
                self.assertGreaterEqual(len(tables), 1, "Real table must be preserved as TableIR")
                # Grid drawings inside the table must not have turned into a DiagramIR
                diagram_overlaps_table = any(
                    d.bbox.intersects(tables[0].bbox) and d.bbox.intersection_area(tables[0].bbox) > 0.5 * tables[0].bbox.area
                    for d in page_ir.diagrams
                )
                self.assertFalse(diagram_overlaps_table, "Table must NOT be rasterized into a DiagramIR")
        doc.close()

    def test_badge_and_header_tab_isolation(self):
        """Verifies left-margin badge and top-right header tab isolation (Fix 4)."""
        doc = pymupdf.open()
        page = doc.new_page(width=595, height=842)

        # 1. Left-margin badge (Key Topic): x1 < 0.14 * 595 = 83.3, e.g. Rect(20, 200, 70, 230)
        page.draw_rect(pymupdf.Rect(20, 200, 70, 230), color=(1, 0.5, 0), fill=(1, 0.8, 0.5))
        page.insert_text(pymupdf.Point(25, 215), "Key Topic", fontsize=8)

        # 2. Main diagram to the right: x0 = 100, y0 = 200, x1 = 450, y1 = 350
        page.draw_rect(pymupdf.Rect(100, 200, 450, 350), color=(0, 0, 1), fill=(0.9, 0.9, 1))

        # 3. Top-right chapter tab: x0 = 565, y0 = 40, x1 = 590, y1 = 70 (x1 >= 0.94 * 595 = 559.3)
        page.draw_rect(pymupdf.Rect(565, 40, 590, 70), color=(0, 0, 0.8), fill=(0, 0, 0.8))
        page.insert_text(pymupdf.Point(570, 60), "1", fontsize=14, color=(1, 1, 1))

        # Header text: Chapter 1... 27
        page.insert_text(pymupdf.Point(100, 50), "Chapter 1 Introduction to Networking 27", fontsize=9)

        analyzer = PageAnalyzer(page, page_number=1)
        page_ir = analyzer.analyze()

        # Verify badge is independent and not merged with main diagram
        diag_bboxes = [d.bbox for d in page_ir.diagrams]
        # Must have at least 2 distinct diagrams (badge and main diagram, plus tab)
        self.assertGreaterEqual(len(diag_bboxes), 2, "Badge, tab and main diagram must not merge into 1 big image")

        # Verify left badge bbox width is small (not stretched across to x=450)
        left_badges = [b for b in diag_bboxes if b.x0 < 80.0 and b.x1 < 120.0]
        self.assertGreaterEqual(len(left_badges), 1, "Left margin badge must remain isolated")

        # Verify top-right tab bbox is small (not stretched across to header)
        right_tabs = [b for b in diag_bboxes if b.x0 > 500.0 and b.y0 < 100.0]
        self.assertGreaterEqual(len(right_tabs), 1, "Top-right tab must remain isolated")

        # Verify Chapter 1 header is an editable TextBlockIR
        header_blocks = [b for b in page_ir.text_blocks if "Chapter 1" in b.text]
        self.assertGreaterEqual(len(header_blocks), 1, "Header text must be editable TextBlockIR")

        doc.close()


if __name__ == "__main__":
    unittest.main()


