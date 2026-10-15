"""
PDFKit Test Suite
"""

import os
import sys

# Remove 'tests' directory from sys.path if placed there by unittest discover,
# so that tests/docx and tests/pptx do not shadow site-packages docx/pptx.
_tests_dir = os.path.dirname(os.path.abspath(__file__))
while _tests_dir in sys.path:
    sys.path.remove(_tests_dir)

_project_root = os.path.dirname(_tests_dir)
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)
