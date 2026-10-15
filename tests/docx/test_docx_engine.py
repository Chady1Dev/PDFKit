"""
DOCX Engine Tests
Verifies DOCX conversion fidelity, Unicode preservation, and error handling.
"""

import os
import sys
import unittest
import zipfile

# Ensure tests dir does not shadow docx package
_curr_dir = os.path.dirname(os.path.abspath(__file__))
_tests_dir = os.path.dirname(_curr_dir)
while _tests_dir in sys.path:
    sys.path.remove(_tests_dir)
_root = os.path.dirname(_tests_dir)
if _root not in sys.path:
    sys.path.insert(0, _root)

from docx import Document
from python_engine.converters.docx import convert_pdf_to_docx
from tests.test_helpers import create_sample_pdf


class TestDocxEngine(unittest.TestCase):

    def setUp(self):
        self.pdf_path = create_sample_pdf(num_pages=2)
        self.output_docx = self.pdf_path.replace(".pdf", ".docx")

    def tearDown(self):
        for p in (self.pdf_path, self.output_docx):
            if os.path.exists(p):
                try:
                    os.remove(p)
                except Exception:
                    pass

    def test_docx_conversion_success(self):
        code = convert_pdf_to_docx(self.pdf_path, self.output_docx)
        self.assertEqual(code, 0, "DOCX conversion should return exit code 0")
        self.assertTrue(os.path.exists(self.output_docx), "DOCX file must exist")
        self.assertGreater(os.path.getsize(self.output_docx), 1000, "DOCX file must have non-trivial size")

        # Validate OpenXML zip structure
        with zipfile.ZipFile(self.output_docx, "r") as z:
            names = z.namelist()
            self.assertIn("[Content_Types].xml", names)
            self.assertIn("word/document.xml", names)

        # Validate content using python-docx
        doc = Document(self.output_docx)
        full_text = "\n".join(p.text for p in doc.paragraphs)
        self.assertIn("Test Belgesi", full_text)
        self.assertIn("Kahve", full_text)

    def test_docx_page_subset_conversion(self):
        code = convert_pdf_to_docx(self.pdf_path, self.output_docx, pages=[0])
        self.assertEqual(code, 0)
        self.assertTrue(os.path.exists(self.output_docx))

    def test_docx_invalid_input(self):
        code = convert_pdf_to_docx("non_existent_file.pdf", self.output_docx)
        self.assertNotEqual(code, 0, "Should return error code for missing file")
        self.assertFalse(os.path.exists(self.output_docx))


if __name__ == "__main__":
    unittest.main()
