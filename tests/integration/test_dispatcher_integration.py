"""
Integration Tests for PDFKit Modular Engine Dispatcher
Verifies end-to-end command line execution, format isolation, progress streaming, and backward compatibility.
"""

import json
import os
import subprocess
import sys
import unittest

# Ensure tests dir does not shadow packages
_curr_dir = os.path.dirname(os.path.abspath(__file__))
_tests_dir = os.path.dirname(_curr_dir)
while _tests_dir in sys.path:
    sys.path.remove(_tests_dir)
_root = os.path.dirname(_tests_dir)
if _root not in sys.path:
    sys.path.insert(0, _root)

from python_engine.dispatcher import dispatch_conversion
from tests.test_helpers import create_sample_pdf


class TestDispatcherIntegration(unittest.TestCase):

    def setUp(self):
        self.pdf_path = create_sample_pdf(num_pages=2)
        self.temp_outputs = []

    def tearDown(self):
        if os.path.exists(self.pdf_path):
            try:
                os.remove(self.pdf_path)
            except Exception:
                pass
        for out in self.temp_outputs:
            if os.path.exists(out):
                try:
                    os.remove(out)
                except Exception:
                    pass

    def _get_out(self, ext):
        p = self.pdf_path.replace(".pdf", f"_out.{ext}")
        self.temp_outputs.append(p)
        return p

    def test_direct_dispatch_all_formats(self):
        """Directly calls dispatch_conversion for docx, pptx, xlsx, txt."""
        for fmt in ("docx", "pptx", "xlsx", "txt"):
            out = self._get_out(fmt)
            code = dispatch_conversion(self.pdf_path, out, fmt)
            self.assertEqual(code, 0, f"Dispatch failed for format: {fmt}")
            self.assertTrue(os.path.exists(out), f"Output file missing for format: {fmt}")
            self.assertGreater(os.path.getsize(out), 0, f"Output file empty for format: {fmt}")

    def test_format_isolation(self):
        """
        Verifies that converting one format does not leak state,
        corrupt global settings, or prevent other formats from succeeding.
        """
        pptx_out = self._get_out("pptx")
        code1 = dispatch_conversion(self.pdf_path, pptx_out, "pptx")
        self.assertEqual(code1, 0)

        docx_out = self._get_out("docx")
        code2 = dispatch_conversion(self.pdf_path, docx_out, "docx")
        self.assertEqual(code2, 0)

        xlsx_out = self._get_out("xlsx")
        code3 = dispatch_conversion(self.pdf_path, xlsx_out, "xlsx")
        self.assertEqual(code3, 0)

        txt_out = self._get_out("txt")
        code4 = dispatch_conversion(self.pdf_path, txt_out, "txt")
        self.assertEqual(code4, 0)

        for out in (pptx_out, docx_out, xlsx_out, txt_out):
            self.assertTrue(os.path.exists(out))

    def test_cli_subprocess_progress_streaming(self):
        """
        Executes python_engine/dispatcher.py via subprocess and inspects
        stdout JSON stream messages to ensure Tauri frontend receives valid progress events.
        """
        out = self._get_out("docx")
        dispatcher_script = os.path.join(
            os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
            "python_engine",
            "dispatcher.py"
        )
        proc = subprocess.run(
            [sys.executable, "-u", dispatcher_script, "--input", self.pdf_path, "--output", out, "--format", "docx"],
            capture_output=True,
            text=True
        )
        self.assertEqual(proc.returncode, 0, f"CLI execution failed: {proc.stderr}")

        # Parse stdout JSON lines
        progress_events = []
        success_event = None
        for line in proc.stdout.splitlines():
            line = line.strip()
            if line.startswith("{") and line.endswith("}"):
                try:
                    payload = json.loads(line)
                    if payload.get("type") == "progress":
                        progress_events.append(payload)
                    elif payload.get("type") == "success":
                        success_event = payload
                except Exception:
                    pass

        self.assertGreater(len(progress_events), 0, "CLI must emit at least one JSON progress event")
        self.assertIsNotNone(success_event, "CLI must emit a JSON success event at completion")
        self.assertEqual(success_event.get("output"), out)

    def test_legacy_shim_backward_compatibility(self):
        """
        Verifies that src-tauri/scripts/pdf2docx_engine.py still functions
        as an entry point and successfully delegates to dispatcher.
        """
        out = self._get_out("pptx")
        shim_script = os.path.join(
            os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
            "src-tauri",
            "scripts",
            "pdf2docx_engine.py"
        )
        proc = subprocess.run(
            [sys.executable, "-u", shim_script, "--input", self.pdf_path, "--output", out, "--format", "pptx"],
            capture_output=True,
            text=True
        )
        self.assertEqual(proc.returncode, 0, f"Shim execution failed: {proc.stderr}")
        self.assertTrue(os.path.exists(out))


if __name__ == "__main__":
    unittest.main()
