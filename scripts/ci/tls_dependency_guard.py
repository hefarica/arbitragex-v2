"""Offline regression for RUSTSEC-2026-0285; complements, never replaces cargo audit.

Reference: https://rustsec.org/advisories/RUSTSEC-2026-0285.html
No network access, shell evaluation, dependency mutation, or advisory exemptions.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import tomllib

ADVISORY = "RUSTSEC-2026-0285"
VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$")


def findings(lock: dict) -> list[dict[str, str]]:
    packages = lock.get("package")
    if not isinstance(packages, list) or not packages:
        raise ValueError("lockfile_missing_packages")
    errors: list[dict[str, str]] = []
    for package in packages:
        if not isinstance(package, dict) or not isinstance(package.get("name"), str):
            raise ValueError("lockfile_invalid_package")
        if package["name"] != "rustls":
            continue
        version = package.get("version")
        match = VERSION.fullmatch(version) if isinstance(version, str) else None
        if match is None:
            raise ValueError("rustls_invalid_version")
        major, minor, patch = map(int, match.groups()[:3])
        # Do not treat a prerelease on the affected release line as a patched release.
        if (major, minor) == (0, 23) and (13 <= patch < 45 or match.group(4)):
            errors.append({"package": "rustls", "version": version, "advisory": ADVISORY})
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lockfile", type=Path, default=Path("backend/Cargo.lock"))
    args = parser.parse_args(argv)
    try:
        with args.lockfile.open("rb") as handle:
            errors = findings(tomllib.load(handle))
    except (OSError, ValueError):
        print(json.dumps({"guard": ADVISORY, "status": "failed", "reason": "unreadable_or_invalid_lockfile"}))
        return 1
    print(json.dumps({"guard": ADVISORY, "status": "failed" if errors else "passed", "findings": errors}))
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
