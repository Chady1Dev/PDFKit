"""
DOCX Converter Engine
Powered by pdf2docx, python-docx, and PyMuPDF.
100% offline and isolated from other format engines.
"""

import logging
from typing import List, Optional
from python_engine.common.errors import ConversionError, emit_error
from python_engine.common.input_validation import validate_input_pdf, validate_output_path
from python_engine.common.progress import emit_progress, emit_success
from .patches import (
    StreamProgressHandler,
    patch_docx_table_performance,
    patch_pdf2docx_layout_protection,
    patch_pdf2docx_unicode,
)


def convert_pdf_to_docx(
    input_pdf: str,
    output_docx: str,
    pages: Optional[List[int]] = None,
    **kwargs,
) -> int:
    """
    Converts PDF into high-fidelity editable Word (.docx)
    preserving images, tables, Turkish Unicode word boundaries, and complex diagrams.
    Returns 0 on success, non-zero on failure.
    """
    try:
        validate_input_pdf(input_pdf)
        validate_output_path(output_docx)

        import pymupdf
        doc = pymupdf.open(input_pdf)
        total_doc_pages = len(doc)
        doc.close()

        total_pages = len(pages) if pages else total_doc_pages
        emit_progress(0, total_pages, 0, "start")

        # Apply DOCX-only optimizations
        patch_pdf2docx_unicode()
        patch_docx_table_performance()
        patch_pdf2docx_layout_protection()

        from pdf2docx import Converter

        cv = Converter(input_pdf)

        logger = logging.getLogger()
        logger.setLevel(logging.INFO)
        progress_handler = StreamProgressHandler(total_pages)
        logger.addHandler(progress_handler)

        try:
            if pages:
                cv.convert(output_docx, pages=pages)
            else:
                cv.convert(output_docx)
        finally:
            cv.close()
            logger.removeHandler(progress_handler)

        emit_progress(total_pages, total_pages, 100, "done")
        emit_success(output_docx)
        return 0

    except Exception as exc:
        emit_error(str(exc))
        return 1
