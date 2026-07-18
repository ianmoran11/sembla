from __future__ import annotations

import sys
from pathlib import Path

NPE_ROOT = Path(__file__).resolve().parents[1]
if str(NPE_ROOT) not in sys.path:
    sys.path.insert(0, str(NPE_ROOT))
