#!/usr/bin/env python3
"""ArbitrageX v2 — config validator.

Validates configs/app.toml against configs/schemas/app.schema.json.
Non-zero exit on failure. Clear messages. Used by bootstrap.sh and CI.
"""
from __future__ import annotations
import json
import sys
from pathlib import Path


def load_toml(path: Path) -> dict:
    try:
        import tomllib  # py 3.11+
        with path.open("rb") as f:
            return tomllib.load(f)
    except ImportError:
        try:
            import tomli  # type: ignore
            with path.open("rb") as f:
                return tomli.load(f)
        except ImportError:
            print("FATAL: neither tomllib (py3.11+) nor tomli installed", file=sys.stderr)
            sys.exit(2)


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <config.toml> <schema.json>", file=sys.stderr)
        return 2
    cfg_path = Path(sys.argv[1])
    schema_path = Path(sys.argv[2])
    if not cfg_path.exists():
        print(f"FATAL: config not found: {cfg_path}", file=sys.stderr)
        return 2
    if not schema_path.exists():
        print(f"FATAL: schema not found: {schema_path}", file=sys.stderr)
        return 2

    cfg = load_toml(cfg_path)
    schema = json.loads(schema_path.read_text())

    try:
        import jsonschema
    except ImportError:
        print("FATAL: `pip install jsonschema` required", file=sys.stderr)
        return 2

    validator = jsonschema.Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(cfg), key=lambda e: list(e.absolute_path))
    if errors:
        for err in errors:
            loc = ".".join(map(str, err.absolute_path)) or "<root>"
            print(f"  INVALID at {loc}: {err.message}", file=sys.stderr)
        print(f"FATAL: config validation failed ({len(errors)} errors)", file=sys.stderr)
        return 1

    enabled = [c for c in cfg.get("chains", []) if c.get("enabled")]
    if not enabled:
        print("FATAL: no chains enabled in config", file=sys.stderr)
        return 1

    print(f"OK: {cfg_path} valid against {schema_path.name}  "
          f"(enabled_chains={[c['chain_id'] for c in enabled]})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
