#!/usr/bin/env python3
"""Materialize Docker runtime secret files from .env without printing values."""
from pathlib import Path
import re, sys, os, subprocess

app = Path("/opt/arbitragex-v2")
envfile = app / ".env"
compose = app / "docker/compose.prod.yml"
secretdir = Path("/run/secrets/arbx")

os.makedirs(str(secretdir), exist_ok=True)
try:
    os.chmod("/run/secrets", 0o750)
except Exception:
    pass
os.chmod(str(secretdir), 0o750)

# Parse .env safely — never print values
env = {}
for raw in envfile.read_text(errors="replace").splitlines():
    line = raw.strip()
    if not line or line.startswith("#") or "=" not in raw:
        continue
    key, val = raw.split("=", 1)
    key = key.strip()
    val = val.strip()
    if len(val) >= 2 and val[0] == val[-1] and val[0] in ('"', "'"):
        val = val[1:-1]
    env[key] = val

# Find all /run/secrets/arbx/<name> references in compose
text = compose.read_text(errors="replace")
pat = r"/run/secrets/arbx/([A-Za-z0-9_.\-]+)"
secret_names = sorted(set(re.findall(pat, text)))

# Mapping from secret file name to .env key
mapping = {"grafana_admin_password": "GRAFANA_ADMIN_PASSWORD"}

missing = []
created = []

for name in secret_names:
    key = mapping.get(name, name.upper().replace("-", "_").replace(".", "_"))
    val = env.get(key, "")
    if not val or val.strip() == "" or "CHANGE_ME" in val or "<" in val:
        missing.append(f"{name}->{key}")
        continue
    target = secretdir / name
    target.write_text(val + "\n")
    created.append(name)

print(f"RUNTIME_SECRET_FILES_CREATED={len(created)}")
print(f"RUNTIME_SECRET_FILES_NAMES={','.join(created)}")
if missing:
    print(f"RUNTIME_SECRET_FILES_MISSING={','.join(missing)}")

# Fix ownership and permissions
subprocess.run(["chown", "-R", "deploy:deploy", str(secretdir)], check=True)
for f in secretdir.iterdir():
    if f.is_file():
        if f.name == "grafana_admin_password":
            subprocess.run(["chown", "472:0", str(f)], check=True)
            os.chmod(str(f), 0o400)
        else:
            os.chmod(str(f), 0o600)

print("RUNTIME_SECRETS_READY=true")
