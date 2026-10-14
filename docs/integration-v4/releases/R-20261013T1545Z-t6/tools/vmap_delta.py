#!/usr/bin/env python3
# vmap_delta.py - V-MAP-Δ EJECUTABLE PERSISTIDO (t6, condición b del quórum §7).
# Re-ejecuta G2/G4/G6 + equivalente de releases (git state / compose ps) del VPS
# contra el mapa A0 congelado (t15, vps-map/VPS_MAP.json sha256 registrado en ℛ.tsv).
# Enmienda α11-D: 'G13' NO existe en el universo medido (G01-G11) — misma errata que
# cond7; el "equivalente de historial de releases" es git state + docker compose ls.
#
# Modos:
#   python vmap_delta.py --check-artifacts p1.txt p2.txt p3.txt   # valida pasadas grabadas (no toca el VPS)
#   python vmap_delta.py --live                                    # 1 pasada en vivo por ssh (SOLO LECTURA)
#        requiere: Git ssh.exe pineado + HOME=%USERPROFILE% (t15 R12: ssh.exe nativo falla exit 255)
#
# Salida: DRIFT <campo> <esperado> <obtenido> por cada deriva; DERIVA=0 ⇒ exit 0.
# Fail-honest: campo no medible en el artefacto ⇒ NOT_PARSED (no se rellena).
import re
import subprocess
import sys

# A0 congelado (t15 V-MAP 2026-10-13T08:2xZ; G2/G4/G6 + G6-git de VPS_MAP.md)
A0 = {
    "git_head": "5f8ea3d67c964d152ae8fda89883952de8282dfb",
    "opt_arbx_exists": False,
    "failed_units": 0,
    "pub_ports": {"22", "80", "8787"},
    "compose_project": "arbitragex-v2",
    "compose_state": "running(25)",
    "compose_file": "/opt/arbitragex-v2/docker/compose.prod.yml",
}

SSH_BIN = r"C:\Program Files\Git\usr\bin\ssh.exe"
LIVE_SCRIPT = r"""
echo "--- G2 ---"
systemctl list-units --state=failed --no-legend | wc -l
echo "--- G4 ---"
ss -lnt | awk 'NR>1 && ($4 ~ /0\.0\.0\.0:|\[::\]:/) {n=split($4,a,":"); print a[n]}' | sort -u
echo "--- G6 ---"
ls -d /opt/arbx 2>/dev/null || echo NO_OPT_ARBX
cd /opt/arbitragex-v2 && git rev-parse HEAD
echo "--- releases-equiv ---"
docker compose ls | tail -1
"""


def parse(text: str) -> dict:
    """Extrae los invariantes de UNA pasada (salida del script ssh)."""
    out = {}
    m = re.search(r"--- G2 ---\s*\n(\d+)", text)
    out["failed_units"] = int(m.group(1)) if m else None
    ports = set(re.findall(r"(?:0\.0\.0\.0|\[::\]):(\d+)", text))
    out["pub_ports"] = ports if ports else None
    out["opt_arbx_exists"] = "NO_OPT_ARBX" not in text and bool(re.search(r"/opt/arbx(?!\S)", text))
    # git_head: el único sha40 de la salida
    m = re.search(r"\b([0-9a-f]{40})\b", text)
    out["git_head"] = m.group(1) if m else None
    m = re.search(r"(arbitragex-\S+)\s+(running\(\d+\))\s+(\S*compose\.prod\.yml)", text)
    if m:
        out["compose_project"], out["compose_state"], out["compose_file"] = m.groups()
    return out


def diff_vs_a0(meas: dict) -> list:
    drifts = []
    for k, want in A0.items():
        got = meas.get(k, None)
        if got is None:
            drifts.append(f"NOT_PARSED {k} (campo no medible en el artefacto — no se rellena)")
        elif got != want:
            drifts.append(f"DRIFT {k}: A0={want!r} got={got!r}")
    return drifts


def main() -> int:
    args = sys.argv[1:]
    if args and args[0] == "--check-artifacts":
        files = args[1:]
        if not files:
            print("uso: --check-artifacts p1.txt p2.txt [p3.txt ...]")
            return 2
        total_drift = 0
        for f in files:
            text = open(f, encoding="utf-8", errors="replace").read()
            meas = parse(text)
            drifts = diff_vs_a0(meas)
            print(f"== {f} ==")
            print(f"  parsed: {meas}")
            if drifts:
                for d in drifts:
                    mark = "DRIFT" if d.startswith("DRIFT") else "WARN"
                    print(f"  {mark}: {d}")
                total_drift += sum(1 for d in drifts if d.startswith("DRIFT"))
            else:
                print("  DERIVA=0 (todos los invariantes == A0)")
        print(f"V-MAP-DELTA total_drift={total_drift}")
        return 1 if total_drift else 0
    if args and args[0] == "--live":
        r = subprocess.run([SSH_BIN, "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
                            "arbx", "bash -s"], input=LIVE_SCRIPT, capture_output=True, text=True)
        print(r.stdout)
        if r.returncode != 0:
            print(f"ssh_exit={r.returncode} stderr={r.stderr!r} — INSTRUMENTO NO DISPONIBLE")
            return 2
        meas = parse(r.stdout)
        drifts = diff_vs_a0(meas)
        for d in drifts:
            print(d)
        print(f"V-MAP-DELTA total_drift={sum(1 for d in drifts if d.startswith('DRIFT'))}")
        return 1 if any(d.startswith("DRIFT") for d in drifts) else 0
    print("uso: vmap_delta.py --check-artifacts <p1> <p2> <p3> | --live")
    return 2


if __name__ == "__main__":
    sys.exit(main())
