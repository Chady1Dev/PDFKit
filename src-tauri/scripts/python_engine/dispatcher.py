"""
PDFKit Central Conversion Dispatcher
Routes conversion requests to isolated, format-specific engines with lazy module loading.
"""

import argparse
import os
import sys
import warnings

# Suppress minor deprecation warnings so stdout/stderr remain clean
warnings.filterwarnings("ignore")

# Ensure python_engine package can be resolved
parent_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if parent_dir not in sys.path:
    sys.path.insert(0, parent_dir)

from python_engine.common.errors import UnsupportedFormatError, emit_error
from python_engine.common.input_validation import validate_input_pdf, validate_output_path
from python_engine.common.pdf_inspection import get_pdf_page_count, parse_page_indices


def determine_format(format_arg: str, output_path: str) -> str:
    """Infers target format from explicit argument or output extension."""
    if format_arg:
        fmt = format_arg.lower().strip()
        if fmt in ("docx", "pptx", "xlsx", "txt"):
            return fmt

    ext = os.path.splitext(output_path)[1].lower().lstrip(".")
    if ext in ("docx", "pptx", "xlsx", "txt"):
        return ext

    return "docx"


def dispatch_conversion(input_pdf: str, output_path: str, target_format: str, pages_arg: str = None) -> int:
    """
    Validates input/output and delegates execution to the corresponding isolated engine.
    Lazy-loads only the requested engine to avoid unnecessary dependencies and cross-module side effects.
    """
    try:
        validate_input_pdf(input_pdf)
        validate_output_path(output_path)
    except Exception as e:
        emit_error(str(e))
        return 1

    total_pages = 0
    try:
        total_pages = get_pdf_page_count(input_pdf)
    except Exception as e:
        emit_error(f"PDF sayfası okunamadı: {str(e)}")
        return 1

    page_indices = parse_page_indices(pages_arg, total_pages)
    fmt = target_format.lower()

    if fmt == "docx":
        from python_engine.converters.docx import convert_pdf_to_docx
        return convert_pdf_to_docx(input_pdf, output_path, pages=page_indices)
    elif fmt == "pptx":
        from python_engine.converters.pptx import convert_pdf_to_pptx
        return convert_pdf_to_pptx(input_pdf, output_path, pages=page_indices)
    elif fmt == "xlsx":
        from python_engine.converters.xlsx import convert_pdf_to_xlsx
        return convert_pdf_to_xlsx(input_pdf, output_path, pages=page_indices)
    elif fmt == "txt":
        from python_engine.converters.txt import convert_pdf_to_txt
        return convert_pdf_to_txt(input_pdf, output_path, pages=page_indices)
    else:
        emit_error(f"Desteklenmeyen dosya formatı: {fmt}")
        return 1


def main() -> int:
    parser = argparse.ArgumentParser(description="PDFKit Advanced Modular Office Document Reconstructor")
    parser.add_argument("--input", "-i", required=True, help="Input PDF file path")
    parser.add_argument("--output", "-o", required=True, help="Output file path")
    parser.add_argument(
        "--format", "-f",
        default="",
        choices=["", "docx", "pptx", "xlsx", "txt"],
        help="Target format (docx, pptx, xlsx, txt)"
    )
    parser.add_argument("--pages", "-p", required=False, help="Comma-separated 1-based page numbers")

    args = parser.parse_args()
    target_fmt = determine_format(args.format, args.output)

    return dispatch_conversion(args.input, args.output, target_fmt, args.pages)


if __name__ == "__main__":
    sys.exit(main())
