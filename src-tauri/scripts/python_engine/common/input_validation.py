"""
PDFKit Common Input and Output Path Validation
"""

import os
from .errors import InputValidationError, OutputValidationError


def validate_input_pdf(input_pdf: str) -> None:
    """
    Validates that the input PDF exists, is a file, and can be read.
    Raises InputValidationError on failure.
    """
    if not input_pdf:
        raise InputValidationError("Girdi PDF yolu belirtilmedi.")
    if not os.path.exists(input_pdf):
        raise InputValidationError(f"Girdi PDF bulunamadı: {input_pdf}")
    if not os.path.isfile(input_pdf):
        raise InputValidationError(f"Girdi yolu bir dosya değil: {input_pdf}")
    try:
        with open(input_pdf, "rb") as f:
            header = f.read(5)
            if header != b"%PDF-":
                # Check within first 1024 bytes for valid PDF header (some files have comments)
                f.seek(0)
                lead = f.read(1024)
                if b"%PDF-" not in lead:
                    raise InputValidationError(f"Geçersiz PDF dosyası başlığı: {input_pdf}")
    except PermissionError:
        raise InputValidationError(f"Dosya başka bir programda açık veya erişim engellendi: {input_pdf}")
    except Exception as e:
        if isinstance(e, InputValidationError):
            raise
        raise InputValidationError(f"Girdi dosyası okunamıyor: {str(e)}")


def validate_output_path(output_path: str) -> None:
    """
    Validates that the output file path is valid and its target directory exists or can be created.
    If the output file already exists, ensures it can be opened for writing (not locked by Office).
    """
    if not output_path:
        raise OutputValidationError("Çıktı dosya yolu belirtilmedi.")

    out_dir = os.path.dirname(output_path)
    if out_dir and not os.path.exists(out_dir):
        try:
            os.makedirs(out_dir, exist_ok=True)
        except Exception as e:
            raise OutputValidationError(f"Çıktı klasörü oluşturulamadı: {str(e)}")

    if os.path.exists(output_path):
        try:
            with open(output_path, "a+b") as f:
                pass
        except PermissionError:
            raise OutputValidationError("Hedef dosya başka bir programda açık (!)")
        except Exception as e:
            raise OutputValidationError(f"Hedef dosyaya yazılamıyor: {str(e)}")
