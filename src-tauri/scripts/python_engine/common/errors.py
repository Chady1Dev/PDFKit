"""
PDFKit Common Error Definitions and Handlers
"""

import json
import sys


class ConversionError(Exception):
    """Base exception for all conversion engine failures."""
    pass


class InputValidationError(ConversionError):
    """Raised when the input PDF cannot be accessed or is invalid."""
    pass


class OutputValidationError(ConversionError):
    """Raised when the output target directory/file is invalid or locked."""
    pass


class UnsupportedFormatError(ConversionError):
    """Raised when an unrecognized conversion format is requested."""
    pass


def emit_error(message: str) -> None:
    """Emits a single-line JSON error message to stdout and flushes."""
    payload = {
        "type": "error",
        "message": message,
    }
    print(json.dumps(payload), flush=True)
