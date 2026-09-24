"""Direct driver: python tools/forensic_integrity/run.py --help."""
from pathlib import Path
import sys
if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from forensic_integrity.__main__ import main
    main()
