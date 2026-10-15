"""
XLSX Converter Engine
Extracts tables and structured text from PDF and builds formatted Microsoft Excel (.xlsx) spreadsheets.
100% Offline / Local-first, independent from DOCX/PPTX.
"""

from typing import List, Optional
from python_engine.common.errors import emit_error
from python_engine.common.input_validation import validate_input_pdf, validate_output_path
from python_engine.common.progress import clean_xml_text, emit_progress, emit_success


def convert_pdf_to_xlsx(
    input_pdf: str,
    output_xlsx: str,
    pages: Optional[List[int]] = None,
    **kwargs,
) -> int:
    """
    Converts PDF tables and structured text into a Microsoft Excel (.xlsx) spreadsheet.
    Returns 0 on success, non-zero on error.
    """
    try:
        validate_input_pdf(input_pdf)
        validate_output_path(output_xlsx)

        import pymupdf
        import xlsxwriter

        doc = pymupdf.open(input_pdf)
        target_pages = [doc[i] for i in pages if 0 <= i < len(doc)] if pages else list(doc)
        total_pages = len(target_pages)
        emit_progress(0, total_pages, 0, "start")

        workbook = xlsxwriter.Workbook(output_xlsx)
        worksheet = workbook.add_worksheet("PDF Verileri")

        # Styles
        header_format = workbook.add_format({
            "bold": True,
            "font_color": "#FFFFFF",
            "bg_color": "#2563EB",
            "border": 1,
            "border_color": "#CBD5E1",
            "align": "center",
            "valign": "vcenter",
        })
        page_title_format = workbook.add_format({
            "bold": True,
            "font_color": "#1E40AF",
            "bg_color": "#F1F5F9",
            "border": 1,
            "border_color": "#E2E8F0",
            "valign": "vcenter",
        })
        data_format = workbook.add_format({
            "border": 1,
            "border_color": "#E2E8F0",
            "valign": "vcenter",
        })

        current_row = 0
        col_max_widths = {}

        def update_col_width(c_idx, text):
            l = len(str(text or ""))
            col_max_widths[c_idx] = max(col_max_widths.get(c_idx, 10), min(50, l + 3))

        for idx, page in enumerate(target_pages):
            page_num = (pages[idx] + 1) if pages else (idx + 1)

            # Insert Page Banner if document has multiple pages
            if total_pages > 1:
                worksheet.write(current_row, 0, f"--- Sayfa {page_num} ---", page_title_format)
                update_col_width(0, f"--- Sayfa {page_num} ---")
                current_row += 1

            # 1. Try finding native tables using PyMuPDF table finder
            tables_found = []
            try:
                detected_tables = page.find_tables().tables
                for t in detected_tables:
                    extracted = t.extract()
                    if extracted and len(extracted) >= 1 and max(len(r) for r in extracted) >= 1:
                        tables_found.append(extracted)
            except Exception:
                tables_found = []

            if tables_found:
                for tbl_data in tables_found:
                    for r_idx, row in enumerate(tbl_data):
                        is_hdr = (r_idx == 0 and len(tbl_data) > 1)
                        fmt = header_format if is_hdr else data_format
                        for c_idx, cell_value in enumerate(row):
                            val_str = clean_xml_text(str(cell_value or "")).strip()
                            worksheet.write(current_row, c_idx, val_str, fmt)
                            update_col_width(c_idx, val_str)
                        current_row += 1
                    current_row += 1  # blank line between tables
            else:
                # 2. Text layout fallback: examine tab-separated or line-based text
                text_content = page.get_text("text")
                lines = [l.strip() for l in text_content.splitlines() if l.strip()]
                if not lines:
                    worksheet.write(
                        current_row, 0,
                        "(Bu sayfa görsel içeriklidir veya metin katmanı içermemektedir)",
                        data_format
                    )
                    update_col_width(0, "Bu sayfa görsel içeriklidir")
                    current_row += 1
                else:
                    for line in lines:
                        cols = line.split("\t") if "\t" in line else [line]
                        for c_idx, col_val in enumerate(cols):
                            val_str = clean_xml_text(col_val.strip())
                            worksheet.write(current_row, c_idx, val_str, data_format)
                            update_col_width(c_idx, val_str)
                        current_row += 1

            if idx < total_pages - 1:
                current_row += 1

            pct = int(((idx + 1) / total_pages) * 100)
            emit_progress(idx + 1, total_pages, pct, "xlsx_page")

        # Apply computed column widths
        for col_idx, width in col_max_widths.items():
            worksheet.set_column(col_idx, col_idx, width)

        workbook.close()
        doc.close()

        emit_progress(total_pages, total_pages, 100, "done")
        emit_success(output_xlsx)
        return 0

    except Exception as exc:
        emit_error(str(exc))
        return 1
