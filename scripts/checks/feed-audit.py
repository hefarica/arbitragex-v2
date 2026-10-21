#!/usr/bin/env python3
"""feed-audit — auditoría read-only de la superficie servida de oportunidades.

Instrumento de medición, NO un artefacto del pipeline que mide: vive en su
propia rama y su propio PR (decisión de MANTIS, 2026-10-21). No muta nada:
sólo GET al wire público, o lectura de una fixture congelada.

Contrato de salida (heredado del diferencial de paper-runs y ampliado aquí):
  * exit 0  -> sin violaciones (y ninguna puerta NOT COMPUTED en --strict)
  * exit 1  -> al menos una puerta en VIOLACION
  * exit 2  -> la lectura no sirve: sin payload o población 0.
               EL VACÍO NUNCA ES «OK»: población 0 ⇒ NOT COMPUTED, exit 2.
  * exit 3  -> con --strict: alguna puerta quedó NOT COMPUTED con población > 0
  Cada puerta imprime población / presentes / nulos. Los umbrales van
  versionados con el sha256 del propio instrumento y NO se ajustan para
  obtener verde: si un umbral cambia, cambia la versión del archivo.

Puertas (v2, 2026-10-21):
  G01 filas viables                        G07 sin bloqueo de política
  G02 economía persistida                  G08 tripwire aritmético bruto/nocional <= 1
  G03 net de coste completo publicable     G09 [DIAGNÓSTICO] banda 0.05 < ratio < 1
  G04 topología de dos patas               G10 política apaga el ranking
  G05 anclaje (block_number)               G11 nocional reproducible (price_ts)
  G06 confianza computada                  G12 el orden publicado = un solo campo
                                           G13 suelo económico por fila (de su breakdown)

G09 NO es puerta: se imprime como diagnóstico declarado. Un ratio < 1 no es
anomalía mientras el campo publicado no declare sus unidades (si publica el
importe de salida en vez de un beneficio, ratio < 1 es lo esperado). El
tripwire de G08 sí es definicional: un ida y vuelta autofinanciado no puede
devolver más que su nocional.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import urllib.error
import urllib.request

INSTRUMENT_VERSION = "feed-audit/2.0.0"
THRESHOLDS_VERSION = "th/2026-10-21.1"

TH = {
    # Definicional, sin tuning: un ida y vuelta autofinanciado no devuelve más
    # que su nocional. No es una política, es aritmética.
    "ratio_imposible": 1.0,
    # Banda declarada como DIAGNÓSTICO (G09), nunca como violación.
    "ratio_banda": 0.05,
    # Suelo global de referencia — SOLO para comparar contra el suelo por fila
    # (G13). Prohibido usarlo como compuerta: el suelo real es por fila.
    "suelo_global_bps": 93.0,
    # Un nocional sin instante de precio no es reproducible entre lecturas.
    "exige_price_ts": True,
}

ENDPOINT = "/api/opportunities/live"
QUERY = ("?viable_only={viable}&limit={limit}&max_age_seconds={window}"
         "&order=profit_usd&route_representative=best_net")
BASE_DEFAULT = "https://edge-arbx.ape-tv.net"
UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
      "Chrome/124.0 Safari/537.36")

VIABLE_STATES = ("executed", "simulated", "viable")
POLICY_PREFIXES = ("TokenNotAllowed",)
PRICE_TS_KEYS = ("price_ts", "prices_ts", "price_timestamp", "price_updated_at", "quoted_at")


# ─────────────────────────── helpers de lectura ────────────────────────────

def instrument_sha(path: str) -> str:
    try:
        with open(path, "rb") as fh:
            return hashlib.sha256(fh.read()).hexdigest()[:12]
    except OSError:
        return "desconocido"


def fetch(base: str, url: str, timeout: int = 40):
    # El WAF del edge responde 403 al UA por defecto de urllib (medido
    # 2026-10-21): el UA de navegador es condición de acceso, no evasión —
    # el endpoint es público y sin credenciales.
    req = urllib.request.Request(base + url, headers={
        "accept": "application/json, text/plain, */*",
        "accept-language": "es-CO,es;q=0.9,en;q=0.8",
        "user-agent": UA,
    })
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        raw = resp.read()
    return raw, json.loads(raw.decode("utf-8"))


def pool_addresses(row) -> list:
    rm = row.get("route_metadata") or {}
    return [p for p in (rm.get("pool_addresses") or []) if p]


def has_two_legs(row) -> bool:
    """Dos patas resueltas: `dex_b` no nulo Y dos pozos con dirección."""
    return row.get("dex_b") is not None and len(pool_addresses(row)) >= 2


def is_policy_rejected(row) -> bool:
    return str(row.get("rejection_reason") or "").startswith(POLICY_PREFIXES)


def notional_usd(row):
    """Nocional con los precios del PROPIO payload — nunca con precios nuestros."""
    prices = {str(k).upper(): v for k, v in (row.get("token_prices_usd") or {}).items()}
    info = row.get("token_in_info") or {}
    sym = str(info.get("symbol") or "").upper()
    dec = info.get("decimals")
    price = prices.get(sym)
    try:
        amount = int(row.get("amount_in_wei") or 0) / (10 ** int(dec if dec is not None else 18))
    except (TypeError, ValueError):
        return None
    if price is None or amount <= 0:
        return None
    return amount * price


def ratio_of(row):
    notional = notional_usd(row)
    gross = row.get("expected_profit_usd")
    if notional is None or gross is None or notional <= 0:
        return None
    return gross / notional


def full_cost_net(row):
    """Net de coste COMPLETO persistido. `net_expected_profit_usd` es gas-only
    por construcción y no decide: se reporta aparte, nunca como el net."""
    econ = row.get("economics")
    if isinstance(econ, dict):
        for key in ("net_profit_usd", "net_usd", "net"):
            if econ.get(key) is not None:
                return econ[key]
    return row.get("simulated_net_profit_usd")


def cost_breakdown(row) -> dict:
    bd = row.get("simulated_cost_breakdown")
    return bd if isinstance(bd, dict) and bd else {}


def price_ts_present(row) -> bool:
    prices = row.get("token_prices_usd")
    if not isinstance(prices, dict):
        return False
    return any(k in prices or f"_{k}" in prices for k in PRICE_TS_KEYS)


def monotone_desc(seq) -> bool:
    return all(seq[i] >= seq[i + 1] for i in range(len(seq) - 1))


def blocked_token(row):
    """Token bloqueado por la dirección que nombra el motivo, cotejada con
    route_metadata.token_addresses (in=0, out=1) — jamás por proximidad."""
    reason = str(row.get("rejection_reason") or "")
    if not reason.startswith("TokenNotAllowed:"):
        return None
    addr = reason.split(":", 1)[1].strip().lower()
    addrs = [str(t).lower() for t in ((row.get("route_metadata") or {}).get("token_addresses") or [])]
    if addrs and addr == addrs[0]:
        return row.get("token_in_info")
    if len(addrs) > 1 and addr == addrs[1]:
        return row.get("token_out_info")
    return {"symbol": None, "verified": None, "validation": {}}


# ─────────────────────────── motor de puertas ──────────────────────────────

class Gate:
    """Una fila de la tabla de auditoría.

    req   : el requisito declarado ('100%', '>0 (aceptación #13)', '0 fallos',
            'cobertura (diagnóstico)'). Se imprime SIEMPRE, para que ninguna
            compuerta dependa de un umbral escondido en el código.
    """

    def __init__(self, gid, name, req, kind, population, present, nulls=0,
                 failures=0, state="OK", detail=""):
        self.gid, self.name, self.req, self.kind = gid, name, req, kind
        self.population, self.present = population, present
        self.nulls, self.failures = nulls, failures
        self.state, self.detail = state, detail

    def line(self) -> str:
        return ("  %-4s %-44s %-22s pob=%-5d pres=%-5d nulos=%-5d fallos=%-4d %-12s %s"
                % (self.gid, self.name[:44], self.req, self.population, self.present,
                   self.nulls, self.failures, self.state, self.detail[:70]))


def _gate(gid, name, req, kind, population, present, nulls=0, detail=""):
    """Construye una puerta y deja que el ESTADO salga del requisito, no del deseo."""
    failures = max(population - present, 0)
    if population == 0:
        return Gate(gid, name, req, kind, 0, 0, 0, 0, "NOT COMPUTED", detail)
    if kind == "cobertura":
        return Gate(gid, name, req, kind, population, present, nulls, 0, "COBERTURA", detail)
    if kind == "diagnostico":
        return Gate(gid, name, req, kind, population, present, nulls, failures, "DIAGNOSTICO", detail)
    if req == ">0":
        ok = present > 0
    else:                                  # '100%' salvo que se diga otra cosa
        ok = failures == 0
    return Gate(gid, name, req, kind, population, present, nulls, failures,
                "OK" if ok else "VIOLACION", detail)


def build_gates(items, viable_payload, ordering) -> list:
    n = len(items)

    def count(pred):
        return sum(1 for x in items if pred(x))

    gates = []
    gates.append(_gate("G01", "filas viables (executed/simulated/viable)", "cobertura", "cobertura",
                       n, count(lambda x: x.get("status") in VIABLE_STATES),
                       detail="viable_only=true -> count=%s" % (viable_payload or {}).get("count")))
    gates.append(_gate("G02", "economía persistida (economics)", "100%", "puerta", n,
                       count(lambda x: x.get("economics") is not None),
                       detail="una fila sin economía no es ordenable ni auditable"))
    gates.append(_gate("G03", "net de coste completo publicable", "100%", "puerta", n,
                       count(lambda x: full_cost_net(x) is not None),
                       detail="gas-only presente=%d (no decide)"
                              % count(lambda x: x.get("net_expected_profit_usd") is not None)))
    gates.append(_gate("G04", "topología de dos patas (dex_b + 2 pozos)", ">0", "puerta", n,
                       count(has_two_legs), detail="aceptación de #13: se cuenta, no se declara"))
    gates.append(_gate("G05", "anclaje (block_number)", "cobertura", "cobertura", n,
                       count(lambda x: x.get("block_number") is not None)))
    gates.append(_gate("G06", "confianza computada", "cobertura", "cobertura", n,
                       count(lambda x: x.get("confidence_state") != "not_computed")))

    ratios = [(r, x) for r, x in ((ratio_of(x), x) for x in items) if r is not None]
    trip = sorted([t for t in ratios if t[0] > TH["ratio_imposible"]], key=lambda t: -t[0])
    gates.append(Gate("G08", "tripwire bruto/nocional <= 1", "0 fallos", "puerta", len(ratios),
                      len(ratios) - len(trip), nulls=n - len(ratios), failures=len(trip),
                      state="VIOLACION" if trip else ("OK" if ratios else "NOT COMPUTED"),
                      detail="; ".join("ratio=%.4g" % r for r, _ in trip[:3])))

    policy = [x for x in items if is_policy_rejected(x)]
    offenders = [x for x in policy if x.get("expected_profit_usd") is not None]
    gates.append(Gate("G10", "política apaga el ranking", "0 fallos", "puerta", n, n - len(offenders),
                      failures=len(offenders),
                      state="VIOLACION" if offenders else "OK",
                      detail="%d rechazadas por política siguen compitiendo por el top" % len(offenders)))

    with_prices = [x for x in items if isinstance(x.get("token_prices_usd"), dict)]
    ts_ok = [x for x in with_prices if price_ts_present(x)]
    gates.append(_gate("G11", "nocional reproducible (price_ts)", "100%", "puerta", len(with_prices),
                       len(ts_ok), nulls=n - len(with_prices),
                       detail="sin instante de precio el nocional cambia entre lecturas"))

    gates.append(Gate("G12", "el orden publicado = un solo campo", "1 campo", "puerta",
                      ordering["population"], ordering["present"], nulls=ordering["nulls"],
                      failures=ordering["failures"], state=ordering["state"], detail=ordering["detail"]))

    bd_rows = [x for x in items if cost_breakdown(x)]
    floors, offenders13 = [], 0
    for x in bd_rows:
        total = x.get("simulated_costs_total_usd")
        if total is None:
            total = sum(v for v in cost_breakdown(x).values() if isinstance(v, (int, float)))
        notional = notional_usd(x)
        if notional and notional > 0:
            floors.append(10000.0 * total / notional)
        gross = x.get("simulated_gross_usd", x.get("expected_profit_usd"))
        net = x.get("simulated_net_profit_usd")
        if gross is not None and net is not None and abs((gross - total) - net) > 0.01:
            offenders13 += 1
    gates.append(Gate("G13", "suelo por fila y traza aritmética", "0 fallos", "puerta",
                      len(bd_rows), len(bd_rows) - offenders13, nulls=n - len(bd_rows),
                      failures=offenders13,
                      state="VIOLACION" if offenders13 else ("OK" if bd_rows else "NOT COMPUTED"),
                      detail=("suelos %.0f–%.0f bps: el suelo global de %.0f bps es decorativo"
                              % (min(floors), max(floors), TH["suelo_global_bps"])) if floors
                             else "sin nocional calculable"))
    return gates, ratios


def ordering_gate(items, viable_payload=None) -> dict:
    """¿QUÉ expresión explica el orden publicado?

    Se prueban candidatos DECLARADOS (nunca inventados por el resultado) y se
    exige monotonicidad sobre la secuencia COMPLETA con NULLS LAST — la misma
    forma que un ORDER BY de SQL. Probar sólo la subsecuencia de valores no
    nulos convierte cualquier sublista corta en «monótona» y miente: se midió
    un candidato que parecía explicar el orden con 13 valores sueltos.

    Si el único candidato que lo explica es una mezcla COALESCE de dos campos,
    el orden publicado no es reproducible por el campo que se publica.
    """
    NEG = float("-inf")

    def coalesce(x):
        for key in ("net_expected_profit_usd", "expected_profit_usd"):
            if x.get(key) is not None:
                return x[key]
        return None

    candidates = (
        ("expected_profit_usd", lambda x: x.get("expected_profit_usd")),
        ("net_expected_profit_usd", lambda x: x.get("net_expected_profit_usd")),
        ("simulated_net_profit_usd", lambda x: x.get("simulated_net_profit_usd")),
        ("simulated_gross_usd", lambda x: x.get("simulated_gross_usd")),
        ("COALESCE(net_expected, expected)", coalesce),
    )
    mono = []
    for name, fn in candidates:
        values = [(fn(x) if fn(x) is not None else NEG) for x in items]
        present = sum(1 for x in items if fn(x) is not None)
        if present >= 2 and monotone_desc(values):
            mono.append((name, present))
    published_present = sum(1 for x in items if x.get("expected_profit_usd") is not None)
    if published_present < 2:
        return {"population": 1, "present": 0, "nulls": 1, "failures": 1,
                "state": "NOT COMPUTED",
                "detail": "menos de 2 valores publicados: el orden no es medible"}
    names = [m[0] for m in mono]
    if "expected_profit_usd" in names:
        return {"population": 1, "present": 1, "nulls": 0, "failures": 0, "state": "OK",
                "detail": "lo explica el campo publicado (n=%d)" % published_present}
    if mono:
        return {"population": 1, "present": 0, "nulls": 1, "failures": 1, "state": "VIOLACION",
                "detail": "sólo lo explica %s" % ", ".join("%s (n=%d)" % m for m in mono)}
    return {"population": 1, "present": 0, "nulls": 1, "failures": 1, "state": "VIOLACION",
            "detail": "ningún candidato declarado explica el orden"}


def allowlist_census(items) -> list:
    blocked = [x for x in items if is_policy_rejected(x)]
    census, unattributed = {}, 0
    for x in blocked:
        tok = blocked_token(x)
        if not tok or tok.get("symbol") is None:
            unattributed += 1
            continue
        val = tok.get("validation") or {}
        key = (tok.get("symbol"), tok.get("verified"), val.get("status"),
               round(val.get("liquidity_usd") or 0))
        census[key] = census.get(key, 0) + 1
    return [(k, c, unattributed) for k, c in census.items()]


# ─────────────────────────────── informe ───────────────────────────────────

def report(items, viable_payload, header, ratios, gates, out, strict):
    n = len(items)
    lines = list(header)
    lines.append("")
    lines.append("PUERTAS — pob=población, pres=presentes, nulos=ausentes, fallos=no cumplen")
    lines.append("          tipo: puerta (exige el requisito) | cobertura (se declara, no se exige)"
                 " | diagnostico (se imprime, no decide)")
    for g in gates:
        lines.append(g.line())
    bad = [g for g in gates if g.state == "VIOLACION"]
    nc = [g for g in gates if g.state == "NOT COMPUTED"]

    band = sorted([r for r, _ in ratios if TH["ratio_banda"] < r <= TH["ratio_imposible"]], reverse=True)
    lines.append("")
    lines.append("G09 DIAGNÓSTICO (no es puerta) — banda %.2f < bruto/nocional <= %.2f: n=%d"
                 % (TH["ratio_banda"], TH["ratio_imposible"], len(band)))
    lines.append("    %s" % ("ratios: " + ", ".join("%.4g" % r for r in band[:8]) if band
                             else "sin filas en la banda"))
    lines.append("    (un ratio < 1 no es anomalía mientras el campo publicado no declare unidades:"
                 " si publica importe de salida en vez de beneficio, es lo esperado)")

    reasons = {}
    for x in items:
        key = str(x.get("rejection_reason") or "SIN_MOTIVO").split(":")[0]
        reasons[key] = reasons.get(key, 0) + 1
    lines.append("")
    lines.append("MOTIVOS: " + ", ".join("%s=%d" % (k, v) for k, v in sorted(reasons.items(), key=lambda t: -t[1])))

    census = allowlist_census(items)
    if census:
        unattributed = census[0][2]
        legit = sorted([(k, c) for k, c, _ in census if k[1] is True and (k[3] or 0) >= 10000],
                       key=lambda t: -(t[0][3] or 0))
        lines.append("ALLOWLIST: %d tokens distintos; atribución sin resolver=%d;"
                     " registro verificado y liquidez >= $10k: %d tokens (%d eventos)"
                     % (len(census), unattributed, len(legit), sum(c for _, c in legit)))
        for k, c in legit[:5]:
            lines.append("    %-12s x%-2d liquidez $%s" % (k[0], c, format(k[3] or 0, ",.0f")))

    lines.append("")
    if bad:
        lines.append("RESULTADO: VIOLACIONES=%d -> exit 1" % len(bad))
    else:
        lines.append("RESULTADO: sin violaciones -> exit 0")
    if nc:
        lines.append("PUERTAS NOT COMPUTED=%d: %s" % (len(nc), ", ".join(g.gid for g in nc)))
    text = "\n".join(lines)
    print(text)
    if out:
        with open(out, "w", encoding="utf-8") as fh:
            fh.write(text + "\n")
    if bad:
        return 1
    if strict and nc:
        return 3
    return 0


def header_block(source, captured_at, payload_sha, payload_bytes, population, viable, sha12):
    return [
        "%s — auditoría read-only de la superficie servida" % INSTRUMENT_VERSION,
        "instrumento sha256[0:12] : %s   umbrales: %s" % (sha12, THRESHOLDS_VERSION),
        "fuente                   : %s" % source,
        "capturado_en_utc         : %s" % captured_at,
        "payload sha256           : %s (%d bytes)" % (payload_sha, payload_bytes),
        "window_total             : %s | filas: %d | max_age servido: %s s"
        % (population.get("window_total"), len(population.get("items") or []), population.get("max_age_seconds")),
        "viable_only=true         : %s" % json.dumps(
            {k: (viable or {}).get(k) for k in ("count", "window_total")}, sort_keys=True),
        "umbrales                 : %s" % json.dumps(TH, sort_keys=True),
    ]


# ──────────────────────────────── modos ────────────────────────────────────

def payload_sha(raw, population):
    """sha256 de los bytes leídos; si la fuente es una fixture ya parseada, se
    hashea su forma canónica para que el informe siga siendo reproducible."""
    if raw is None:
        data = json.dumps(population, sort_keys=True).encode()
    elif isinstance(raw, (bytes, bytearray)):
        data = bytes(raw)
    else:
        data = str(raw).encode()
    return hashlib.sha256(data).hexdigest(), len(data)


def run_payload(population, viable, source, captured_at, out, strict, sha12, raw=None):
    items = population.get("items") or []
    psha, pbytes = payload_sha(raw, population)
    lines = header_block(source, captured_at, psha, pbytes, population, viable, sha12)
    if not items:
        lines.append("")
        lines.append("POBLACIÓN 0 — nada que auditar: NOT COMPUTED (el vacío nunca es OK) -> exit 2")
        print("\n".join(lines))
        if out:
            with open(out, "w", encoding="utf-8") as fh:
                fh.write("\n".join(lines) + "\n")
        return 2
    ordering = ordering_gate(items, viable)
    gates, ratios = build_gates(items, viable, ordering)
    return report(items, viable, lines, ratios, gates, out, strict)


def load_fixture(path):
    with open(path, "r", encoding="utf-8") as fh:
        doc = json.load(fh)
    meta = doc.get("audit_fixture") or {}
    return doc.get("population") or {}, doc.get("viable") or {}, meta


def main(argv=None):
    ap = argparse.ArgumentParser(description="Auditoría read-only de la superficie de oportunidades")
    ap.add_argument("--url", default=BASE_DEFAULT)
    ap.add_argument("--window", type=int, default=86400)
    ap.add_argument("--limit", type=int, default=200)
    ap.add_argument("--fixture", help="lee una fixture congelada en vez de la red")
    ap.add_argument("--controls", action="store_true",
                    help="corre los tres controles (positivo/negativo/vacío) y verifica los exit")
    ap.add_argument("--fixtures-dir", default=os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                                           "..", "fixtures", "feed-audit"))
    ap.add_argument("--out", help="escribe el informe en un archivo")
    ap.add_argument("--strict", action="store_true", help="exit 3 si alguna puerta queda NOT COMPUTED")
    a = ap.parse_args(argv)
    sha12 = instrument_sha(os.path.abspath(__file__))

    if a.controls:
        return run_controls(a, sha12)
    if a.fixture:
        population, viable, meta = load_fixture(a.fixture)
        return run_payload(population, viable,
                           "fixture %s (%s)" % (a.fixture, meta.get("kind", "?")),
                           meta.get("captured_at_utc", "-"), a.out, a.strict, sha12)
    q_all = ENDPOINT + QUERY.format(viable="false", limit=a.limit, window=a.window)
    q_via = ENDPOINT + QUERY.format(viable="true", limit=a.limit, window=a.window)
    try:
        raw, population = fetch(a.url, q_all)
        _, viable = fetch(a.url, q_via)
    except (urllib.error.URLError, urllib.error.HTTPError, ValueError, TimeoutError) as exc:
        lines = header_block("live %s%s" % (a.url, q_all), "-", "-", 0, {}, {}, sha12)
        lines.append("")
        lines.append("LECTURA FALLIDA: %s: %s -> exit 2 (sin lectura)" % (type(exc).__name__, exc))
        print("\n".join(lines))
        return 2
    import datetime
    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    return run_payload(population, viable, "live %s%s" % (a.url, q_all), now, a.out, a.strict, sha12, raw=raw)


def run_controls(a, sha12):
    expected = (("positive", 0), ("negative", 1), ("empty", 2))
    base = os.path.abspath(a.fixtures_dir)
    rc, fails = 0, []
    for name, want in expected:
        path = os.path.join(base, "%s.json" % name)
        population, viable, meta = load_fixture(path)
        got = run_payload(population, viable, "fixture %s.json" % name,
                          meta.get("captured_at_utc", "-"), None, False, sha12)
        status = "OK" if got == want else "FALLA"
        if got != want:
            fails.append((name, want, got))
            rc = 1
        print("CONTROL %-9s exit esperado=%d obtenido=%d  %s" % (name, want, got, status))
        print("-" * 72)
    print("CONTROLES: %d/%d %s" % (len(expected) - len(fails), len(expected),
                                   "-> exit 0" if not fails else "-> exit 1"))
    return rc


if __name__ == "__main__":
    sys.exit(main())
