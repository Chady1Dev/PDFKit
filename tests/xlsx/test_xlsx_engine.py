"""
XLSX Engine Tests
Verifies Excel spreadsheet generation, worksheets, table extraction, and Unicode text fidelity.
"""

import os
import unittest
import zipfile
from python_engine.converters.xlsx import convert_pdf_to_xlsx
from tests.test_helpers import create_sample_pdf


class TestXlsxEngine(unittest.TestCase):

    def setUp(self):
        self.pdf_path = create_sample_pdf(num_pages=2)
        self.output_xlsx = self.pdf_path.replace(".pdf", ".xlsx")

    def tearDown(self):
        for p in (self.pdf_path, self.output_xlsx):
            if os.path.exists(p):
                try:
                    os.remove(p)
                except Exception:
                    pass

    def test_xlsx_conversion_success(self):
        code = convert_pdf_to_xlsx(self.pdf_path, self.output_xlsx)
        self.assertEqual(code, 0, "XLSX conversion should return exit code 0")
        self.assertTrue(os.path.exists(self.output_xlsx), "XLSX file must exist")
        self.assertGreater(os.path.getsize(self.output_xlsx), 500, "XLSX file must have non-trivial size")

        # Validate OpenXML zip structure
        with zipfile.ZipFile(self.output_xlsx, "r") as z:
            names = z.namelist()
            self.assertIn("[Content_Types].xml", names)
            self.assertIn("xl/workbook.xml", names)
            self.assertIn("xl/worksheets/sheet1.xml", names)

            # Check sheet XML or sharedStrings for content
            content = ""
            for target in ("xl/worksheets/sheet1.xml", "xl/sharedStrings.xml"):
                if target in names:
                    content += z.read(target).decode("utf-8", errors="ignore")

            self.assertIn("Sayfa 1", content)
            self.assertIn("Kahve", content)

    def test_xlsx_page_subset_conversion(self):
        code = convert_pdf_to_xlsx(self.pdf_path, self.output_xlsx, pages=[0])
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(self.output_xlsx))

    def test_xlsx_invalid_input(self):
        code = convert_pdf_to_xlsx("missing_pdf_for_excel.pdf", self.output_xlsx)
        self.assertNotEqual(code, 0)
        self.assertFalse(os.path.exists(self.output_xlsx))


if __name__ == "__main__":
    unittest.main()
