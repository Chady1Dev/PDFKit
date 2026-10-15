"""
TXT Converter Engine
Extracts plain text in natural reading order with UTF-8 BOM encoding and page delineation.
100% Offline / Local-first, independent from DOCX/PPTX.
"""

from typing import List, Optional
from python_engine.common.errors import emit_error
from python_engine.common.input_validation import validate_input_pdf, validate_output_path
from python_engine.common.progress import emit_progress, emit_success


def convert_pdf_to_txt(
    input_pdf: str,
    output_txt: str,
    pages: Optional[List[int]] = None,
    **kwargs,
) -> int:
    """
    Extracts text from PDF into a clean, formatted UTF-8 text file with BOM.
    Preserves natural reading order, Unicode/Turkish characters, and page boundaries.
    Returns 0 on success, non-zero on error.
    """
    try:
        validate_input_pdf(input_pdf)
        validate_output_path(output_txt)

        import pymupdf

        doc = pymupdf.open(input_pdf)
        target_pages = [doc[i] for i in pages if 0 <= i < len(doc)] if pages else list(doc)
        total_pages = len(target_pages)
        emit_progress(0, total_pages, 0, "start")

        with open(output_txt, "wb") as f:
            # Write UTF-8 BOM (\xEF\xBB\xBF) so Windows Notepad and other editors recognize Turkish letters
            f.write(b"\xef\xbb\xbf")

            for idx, page in enumerate(target_pages):
                page_num = (pages[idx] + 1) if pages else (idx + 1)
                header = f"--- Sayfa {page_num} ---\r\n"
                f.write(header.encode("utf-8"))

                # Extract text using PyMuPDF sorted reading order
                raw_text = page.get_text("text", sort=True)
                lines = raw_text.splitlines()

                trimmed_lines = [l.strip() for l in lines]
                non_empty = [l for l in trimmed_lines if l]

                if not non_empty:
                    f.write("(Bu sayfa görsel içeriklidir veya metin katmanı içermemektedir)\r\n\r\n".encode("utf-8"))
                else:
                    for line in lines:
                        f.write(line.encode("utf-8"))
                        f.write(b"\r\n")
                    f.write(b"\r\n")

                if idx < total_pages - 1:
                    f.write(b"\r\n")

                pct = int(((idx + 1) / total_pages) * 100)
                emit_progress(idx + 1, total_pages, pct, "txt_page")

        doc.close()

        emit_progress(total_pages, total_pages, 100, "done")
        emit_success(output_txt)
        return 0

    except Exception as exc:
        emit_error(str(exc))
        return 1
