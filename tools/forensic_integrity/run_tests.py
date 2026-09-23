"""Offline test entrypoint; never reads production credentials or starts a service."""
import sys
from pathlib import Path
import unittest
root=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(root))
suite=unittest.defaultTestLoader.discover(str(Path(__file__).with_name('tests')))
result=unittest.TextTestRunner(verbosity=2).run(suite)
raise SystemExit(not result.wasSuccessful())
