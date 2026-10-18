"""Generate the reintegration ledger + manifest.

Rules (iteration 2):
  * The ledger hashes every file under docs/reaintegration/** and scripts/gates/*
    EXCEPT the two self-referential artifacts (the ledger and the manifest
    themselves), which cannot hash themselves. The manifest publishes the
    ledger's sha256, so the whole chain is verifiable end to end.
  * Gate outcomes come from docs/reaintegration/gates/RESULTS.json — a real
    recorded run, never a value invented here.
"""
import hashlib, json, os, glob, subprocess, datetime

SELF_REFERENTIAL = {
    "docs/reaintegration/ledger/LEDGER_SHA256.json",
    "docs/reaintegration/MANIFEST_SHA256.json",
}


def sha(p):
    h = hashlib.sha256()
    with open(p, 'rb') as f:
        for b in iter(lambda: f.read(65536), b''):
            h.update(b)
    return h.hexdigest()


head = subprocess.run("git rev-parse HEAD", shell=True, capture_output=True, text=True).stdout.strip()
files = sorted(glob.glob("docs/reaintegration/**/*", recursive=True)) + \
        sorted(glob.glob("scripts/gates/*"))
files = [f for f in files if os.path.isfile(f) and f not in SELF_REFERENTIAL]
ledger = {
    "schema": "reaintegration/ledger/2",
    "generated_at": datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "base_sha": head,
    "algorithm": "sha256",
    "excluded_self_referential": sorted(SELF_REFERENTIAL),
    "entries": [{"path": f, "sha256": sha(f), "bytes": os.path.getsize(f)} for f in files],
}
os.makedirs("docs/reaintegration/ledger", exist_ok=True)
json.dump(ledger, open("docs/reaintegration/ledger/LEDGER_SHA256.json", "w"), indent=2)

results_path = "docs/reaintegration/gates/RESULTS.json"
gates_result = json.load(open(results_path))["results"] if os.path.exists(results_path) else {}

manifest = {
    "schema": "reaintegration/manifest/2",
    "generated_at": ledger["generated_at"],
    "base_sha": head,
    "artifacts": {
        "inventory": "docs/reaintegration/00-INVENTORY.json",
        "rea_fiches": len(glob.glob("docs/reaintegration/rea/*.md")),
        "adrs": sorted(glob.glob("docs/reaintegration/adr/*.md")),
        "gates": sorted(glob.glob("docs/reaintegration/gates/*.md")) + sorted(glob.glob("scripts/gates/*.sh")),
        "ledger": "docs/reaintegration/ledger/LEDGER_SHA256.json",
        "ledger_sha256": sha("docs/reaintegration/ledger/LEDGER_SHA256.json"),
    },
    "gates_result": gates_result,
    "files_hashed": len(ledger["entries"]),
}
json.dump(manifest, open("docs/reaintegration/MANIFEST_SHA256.json", "w"), indent=2)
print("ledger entries:", len(ledger["entries"]))
print("manifest ledger_sha256:", manifest["artifacts"]["ledger_sha256"])
print("rea fiches:", manifest["artifacts"]["rea_fiches"])
print("gates:", len(manifest["artifacts"]["gates"]))
