"""
TXT Engine Tests
Verifies plain text extraction, UTF-8 BOM encoding, Turkish character integrity, and page boundaries.
"""

import os
import unittest
from python_engine.converters.txt import convert_pdf_to_txt
from tests.test_helpers import create_sample_pdf


class TestTxtEngine(unittest.TestCase):

    def setUp(self):
        self.pdf_path = create_sample_pdf(num_pages=2)
        self.output_txt = self.pdf_path.replace(".pdf", ".txt")

    def tearDown(self):
        for p in (self.pdf_path, self.output_txt):
            if os.path.exists(p):
                try:
                    os.remove(p)
                except Exception:
                    pass

    def test_txt_conversion_success(self):
        code = convert_pdf_to_txt(self.pdf_path, self.output_txt)
        self.assertEqual(code, 0, "TXT conversion should return exit code 0")
        self.assertTrue(os.path.exists(self.output_txt), "TXT file must exist")

        with open(self.output_txt, "rb") as f:
            bytes_content = f.read()

        # Verify UTF-8 BOM header
        self.assertTrue(
            bytes_content.startswith(b"\xef\xbb\xbf"),
            "Output text must begin with UTF-8 BOM for Windows Notepad/Turkish compatibility"
        )

        text = bytes_content[3:].decode("utf-8")
        self.assertIn("--- Sayfa 1 ---", text)
        self.assertIn("--- Sayfa 2 ---", text)
        self.assertIn("Test Belgesi", text)
        self.assertIn("Kahve", text)

    def test_txt_page_subset_conversion(self):
        code = convert_pdf_to_txt(self.pdf_path, self.output_txt, pages=[0])
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(self.output_txt))
        with open(self.output_txt, "r", encoding="utf-8-sig") as f:
            text = f.read()
        self.assertIn("--- Sayfa 1 ---", text)
        self.assertNotIn("--- Sayfa 2 ---", text)

    def test_txt_invalid_input(self):
        code = convert_pdf_to_txt("does_not_exist_sample.pdf", self.output_txt)
        self.assertNotEqual(code, 0)
        self.assertFalse(os.path.exists(self.output_txt))


if __name__ == "__main__":
    unittest.main()
