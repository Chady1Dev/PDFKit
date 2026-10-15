"""
PDFKit Common Engine Utilities
"""

from .errors import (
    ConversionError,
    InputValidationError,
    OutputValidationError,
    UnsupportedFormatError,
    emit_error,
)
from .input_validation import validate_input_pdf, validate_output_path
from .pdf_inspection import get_pdf_page_count, parse_page_indices
from .progress import clean_xml_text, emit_progress, emit_success

__all__ = [
    "clean_xml_text",
    "emit_progress",
    "emit_success",
    "emit_error",
    "ConversionError",
    "InputValidationError",
    "OutputValidationError",
    "UnsupportedFormatError",
    "validate_input_pdf",
    "validate_output_path",
    "get_pdf_page_count",
    "parse_page_indices",
]
