"""
PDFKit Common Read-only PDF Inspection and Argument Parsing
"""

from typing import List, Optional, Tuple


def get_pdf_page_count(input_pdf: str) -> int:
    """Returns the total number of pages in the PDF using PyMuPDF."""
    import pymupdf

    doc = pymupdf.open(input_pdf)
    total = len(doc)
    doc.close()
    return total


def parse_page_indices(pages_arg: Optional[str], total_pages: int) -> Optional[List[int]]:
    """
    Parses a comma-separated 1-based page numbers string (e.g. '1,2,5')
    into a sorted list of 0-based page indices valid for the PDF.
    """
    if not pages_arg:
        return None

    indices = []
    for p in pages_arg.split(","):
        p_str = p.strip()
        if p_str.isdigit():
            val = int(p_str)
            if 1 <= val <= total_pages:
                idx = val - 1
                if idx not in indices:
                    indices.append(idx)
    return indices if indices else None
