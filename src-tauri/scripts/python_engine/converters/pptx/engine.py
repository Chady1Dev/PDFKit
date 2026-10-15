"""
PPTX Converter Engine
Powered by PyMuPDF and python-pptx (100% Offline / Local-first).
High-fidelity general-purpose reconstruction via PageIR, geometric analysis,
active healing, and precise slide builder.
"""

from typing import List, Optional
from pptx import Presentation
from pptx.util import Pt
from python_engine.common.errors import emit_error
from python_engine.common.input_validation import validate_input_pdf, validate_output_path
from python_engine.common.progress import emit_progress, emit_success
from .analyzer import PageAnalyzer
from .ir import safe_float
from .pptx_builder import PptxSlideBuilder
from .quality import QualityAuditor


def convert_pdf_to_pptx(
    input_pdf: str,
    output_pptx: str,
    pages: Optional[List[int]] = None,
    **kwargs,
) -> int:
    """
    Converts PDF pages into 100% compliant Microsoft PowerPoint (.pptx) presentation
    with all images, native tables, diagrams, and exact positioned text boxes.
    Returns 0 on success, non-zero on error.
    """
    try:
        validate_input_pdf(input_pdf)
        validate_output_path(output_pptx)

        import pymupdf

        doc = pymupdf.open(input_pdf)

        # ADIM 1: In-memory deletion of StructTreeRoot/ActualText and StructParents to kill ghost alt-text
        try:
            cat_xref = doc.pdf_catalog()
            if cat_xref:
                doc.xref_del_key(cat_xref, "StructTreeRoot")
                doc.xref_del_key(cat_xref, "MarkInfo")
            for p in doc:
                doc.xref_del_key(p.xref, "StructParents")
        except Exception:
            pass

        target_indices = pages if pages is not None else list(range(len(doc)))
        total_pages = len(target_indices)
        emit_progress(0, total_pages, 0, "start")

        prs = Presentation()
        blank_layout = prs.slide_layouts[6]

        # Universal slide dimensions and scale calculation
        scale = 1.0
        if total_pages > 0 and 0 <= target_indices[0] < len(doc):
            first_p = doc[target_indices[0]]
            raw_w = max(1.0, float(first_p.rect.width))
            raw_h = max(1.0, float(first_p.rect.height))
            max_dim, min_dim = max(raw_w, raw_h), min(raw_w, raw_h)
            if max_dim > 3600.0:
                scale = 3600.0 / max_dim
            elif min_dim < 200.0:
                scale = 720.0 / max_dim
            else:
                scale = 1.0
            slide_w = raw_w * scale
            slide_h = raw_h * scale
            prs.slide_width = Pt(slide_w)
            prs.slide_height = Pt(slide_h)

        for idx, page_num in enumerate(target_indices):
            if not (0 <= page_num < len(doc)):
                continue

            page = doc[page_num]

            # In-memory stream cleaning: neutralize /MCID and /ActualText to prevent ghost alt-texts (Figure 3 'e Quiz...' fix)
            for c_xref in page.get_contents():
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

            # 1. Page Analysis and IR extraction (with universal scale & origin)
            analyzer = PageAnalyzer(page, page_num + 1, scale=scale)
            page_ir = analyzer.analyze()

            # 2. Quality Audit and Active Healing
            _ = QualityAuditor.audit_and_heal(page_ir)

            # 3. Add PowerPoint slide and render elements
            slide = prs.slides.add_slide(blank_layout)
            builder = PptxSlideBuilder(slide, page_ir)
            builder.build()

            pct = int(((idx + 1) / total_pages) * 100)
            emit_progress(idx + 1, total_pages, pct, "pptx_page")

        prs.save(output_pptx)
        doc.close()

        emit_progress(total_pages, total_pages, 100, "done")
        emit_success(output_pptx)
        return 0

    except Exception as exc:
        emit_error(str(exc))
        return 1
