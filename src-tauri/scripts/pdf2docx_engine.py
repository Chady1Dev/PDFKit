"""
PDFKit Office Document Reconstructor Shim
Backwards-compatible entrypoint that delegates to the modular python_engine architecture.
"""

import os
import sys

# Add potential python_engine search paths
script_dir = os.path.dirname(os.path.abspath(__file__))
candidates = [
    script_dir,
    os.path.abspath(os.path.join(script_dir, "python_engine")),
    os.path.abspath(os.path.join(script_dir, "..", "..")),
    os.path.abspath(os.path.join(script_dir, "..")),
]

for c in candidates:
    if c not in sys.path:
        sys.path.insert(0, c)

try:
    from python_engine.dispatcher import main
except ImportError:
    # If python_engine is installed directly as a subfolder of scripts
    try:
        from dispatcher import main
    except ImportError as e:
        import json
        print(json.dumps({"type": "error", "message": f"Dönüşüm modülü yüklenemedi: {e}"}), flush=True)
        sys.exit(1)

if __name__ == "__main__":
    sys.exit(main())
