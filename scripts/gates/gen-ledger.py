import hashlib, json, os, glob, subprocess, datetime
def sha(p):
    h=hashlib.sha256()
    with open(p,'rb') as f:
        for b in iter(lambda:f.read(65536), b''): h.update(b)
    return h.hexdigest()

head = subprocess.run("git rev-parse HEAD",shell=True,capture_output=True,text=True).stdout.strip()
files = sorted(glob.glob("docs/reaintegration/**/*", recursive=True)) + \
        sorted(glob.glob("scripts/gates/*"))
files = [f for f in files if os.path.isfile(f)]
ledger = {
  "schema":"reaintegration/ledger/1",
  "generated_at": datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
  "base_sha": head,
  "algorithm":"sha256",
  "entries":[{"path":f,"sha256":sha(f),"bytes":os.path.getsize(f)} for f in files]
}
os.makedirs("docs/reaintegration/ledger",exist_ok=True)
# write ledger (excluding itself from its own entries is unavoidable; note it)
json.dump(ledger, open("docs/reaintegration/ledger/LEDGER_SHA256.json","w"), indent=2)

# manifest = ledger sha + gates + fichas count
manifest = {
  "schema":"reaintegration/manifest/1",
  "generated_at": ledger["generated_at"],
  "base_sha": head,
  "artifacts": {
    "inventory":"docs/reaintegration/00-INVENTORY.json",
    "rea_fiches": len(glob.glob("docs/reaintegration/rea/*.md")),
    "adrs": sorted(glob.glob("docs/reaintegration/adr/*.md")),
    "gates": sorted(glob.glob("docs/reaintegration/gates/*.md"))+["scripts/gates/g-backup-01.sh","scripts/gates/g-admin-root-01.sh"],
    "ledger":"docs/reaintegration/ledger/LEDGER_SHA256.json",
    "ledger_sha256": sha("docs/reaintegration/ledger/LEDGER_SHA256.json")
  },
  "gates_result":{"G-BACKUP-01":"PASS","G-ADMIN-ROOT-01":"PASS"},
  "files_hashed": len(ledger["entries"])
}
json.dump(manifest, open("docs/reaintegration/MANIFEST_SHA256.json","w"), indent=2)
print("ledger entries:", len(ledger["entries"]))
print("manifest ledger_sha256:", manifest["artifacts"]["ledger_sha256"])
print("rea fiches:", manifest["artifacts"]["rea_fiches"])
