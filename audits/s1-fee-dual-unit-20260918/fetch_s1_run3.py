import json, os, urllib.request, winreg

key = winreg.QueryValueEx(winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Environment"), "HERMES_API_SERVER_KEY")[0]
req = urllib.request.Request(
    "http://127.0.0.1:8642/v1/runs/run_fc3ccdc9d5804bc5b5f4eb1125fb147d",
    headers={"Authorization": "Bearer " + key},
)
d = json.load(urllib.request.urlopen(req, timeout=15))
print("status:", d.get("status"), "| completed:", d.get("completed"), "| interrupted:", d.get("interrupted"), "| last_event:", d.get("last_event"))
out = d.get("output") or ""
print("output_len:", len(out))
dest = os.path.join(os.path.dirname(os.path.abspath(__file__)), "hermes-verdict-run3")
open(dest + ".md", "w", encoding="utf-8").write(out)
json.dump(d, open(dest + ".json", "w", encoding="utf-8"), indent=1)
print("--- output head ---")
print(out[:3500])
