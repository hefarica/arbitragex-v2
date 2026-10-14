#!/usr/bin/env python3
"""G-SIM-1 coverage-ceiling instrument (GSIM-COVERAGE-CEILING-01, 2026-10-08).

WHY THIS EXISTS
---------------
The variance benchmark can only label the topologies its own A.3.a encoder can
encode. Measured on production (`main` = 901eb947ff3bec359a3021a8db5b074e7080b79f,
window `2026-10-08 13:24:46Z`, 2 h rolling, chain 1): **13 of 962** distinct
topologies in the window are labelable. The other **949 are discarded by the
ENCODER, not by the market**, and every one of them EXISTS as a row in
`opportunities`.

Before this instrument the discard was an AGGREGATE:
  * `scripts/gsim1_variance_export.sql:62-64` drops them silently inside a SQL
    `WHERE` (`jsonb_array_length(...) = 2` + `<@ '["UniswapV2","SushiSwap"]'`);
  * the driver printed only the total gap
    (`scripts/gsim1_variance_benchmark.sh:181-185`);
  * the registry row's `coverage` field is `samples_labeled / population_size`
    where `population_size` is the ENCODER-FILTERED export — so `coverage = 1.0`
    means "we labeled everything the encoder allows". That reads as a result of
    the market while it is a statement about the encoder. Removing that
    misreading is the point of this instrument (R10: no field may present itself
    as computed when it is not processed in every layer).

WHAT IT EMITS
-------------
1. ONE NAMED REASON PER DISCARDED TOPOLOGY PLUS ITS COUNT, with the
   `strategy_kind` and the adapter labels VERBATIM as production spells them.
2. `COVERAGE_VERDICT` — **NO_COMPUTADO** whenever the discard is non-zero, with
   the reason spelled out. The degradation is STRUCTURAL (`discarded == 0`),
   never a tunable cut: `min_samples`, the gate threshold and the `skips`
   histogram are not touched by this change.
3. A partition that RECONCILES: the named buckets sum to `window_total`. An
   unnamed remainder is a test failure, not a footnote.

TWO ENCODER SCOPES, DELIBERATELY NOT MERGED
-------------------------------------------
The export filters with jsonb containment against two LITERAL labels
(`scripts/gsim1_variance_export.sql:64`), while the harness' `adapter_to_semantic()`
(`backend/sim-core/tests/variance_benchmark.rs:111`) accepts six literal
spellings — and the two producers spell their adapters differently (measured:
`dex_arb` rows carry `UniswapV3`/`PancakeSwap V3`, `triangular` rows carry
lowercase `uniswap-v3`). A topology the harness COULD encode but the export
drops is therefore reported under its own code
(`adapter_label_spelling:export_literal_mismatch`), never folded into
`unsupported_adapter`. Merging the two would misattribute an export-side
spelling gap to the encoder.

HONEST BOUNDARY (R8 — never read a label as another thing)
---------------------------------------------------------
`unsupported_adapter:<label>` names the ENCODER's limitation. It is NOT a claim
that the market held a profitable route there. The production state of those
same rows (`status`/`rejection_reason`) is a DIFFERENT axis, measured by the
workflow's per-sample attribution step. The two are emitted side by side and
never merged.

READ-ONLY. The only SQL this script can send is the single `SELECT` in
`build_window_sql()`. No INSERT/UPDATE/DELETE/DDL, no signing, no broadcast.

EXIT CODE IS ALWAYS 0. Every anomaly route prints a literal reason and a GitHub
`::warning::`. Reddening the benchmark gate for an observational instrument
would be a new defect; GSIM-PRED-REASON-01 §2 reached the same conclusion for
the pred_failed attribution step.
"""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys

# --------------------------------------------------------------------------
# The two encoder contracts this instrument mirrors. One source of truth each;
# a drift must be LOUD (the workflow cross-checks these literals against the
# files they mirror, and the unit tests re-derive them from the real sources).
# --------------------------------------------------------------------------
# adapter_to_semantic() — the HARNESS encoder scope (what the harness can label).
HARNESS_ACCEPTED_ADAPTERS: tuple[str, ...] = (
    "UniswapV2",
    "uniswap-v2",
    "uniswapv2",
    "SushiSwap",
    "sushi",
    "sushiswap",
)
HARNESS_ADAPTER_SOURCE = "backend/sim-core/tests/variance_benchmark.rs:111 adapter_to_semantic()"

# The EXPORT's literal jsonb containment scope (what actually reaches the harness).
EXPORT_LITERAL_ADAPTERS: tuple[str, ...] = ("UniswapV2", "SushiSwap")
EXPORT_ENCODER_FILTER_SOURCE = (
    "scripts/gsim1_variance_export.sql:64 ((dex_adapters) <@ '[\"UniswapV2\",\"SushiSwap\"]')"
)

SUPPORTED_ROUTE_SHAPE = "legs==2"
SUPPORTED_ROUTE_SHAPE_SOURCE = "scripts/gsim1_variance_export.sql:62 (jsonb_array_length(...) = 2)"
COVERAGE_FIELD_SOURCE = "scripts/gsim1_variance_benchmark.sh:269-274 (coverage = samples_labeled / population_size)"

# Named discard codes. Every code carries its own reason; a bare category name is
# never a code (asserted by the tests).
CODE_UNSUPPORTED_SHAPE = "unsupported_route_shape"
CODE_UNSUPPORTED_ADAPTER = "unsupported_adapter"
CODE_LABEL_SPELLING = "adapter_label_spelling:export_literal_mismatch"
# NOT an encoder limit: the export's own scope filter (`:65-66`), which the
# harness mirrors as `bad_shape` (`variance_benchmark.rs:422-428`). Named
# separately so nobody reads it as an encoder limitation.
CODE_ZERO_AMOUNT = "zero_amount_in"


def classify(legs: int | None, adapters: list[str], amount_nonzero: bool) -> str | None:
    """Named discard code for one topology bucket; ``None`` when it is labelable.

    Adapter labels are used VERBATIM. Normalising them here (lower-casing them,
    or stripping the space in ``PancakeSwap V3``) would hide exactly the mismatch
    these codes exist to expose: both encoder scopes match on the literal
    spelling, so a label they do not know is a label they cannot encode.
    """
    if legs is None:
        # `dex_adapters` present but not a JSON array — named, never dropped.
        return f"{CODE_UNSUPPORTED_SHAPE}:legs=not_an_array"
    if legs != 2:
        return f"{CODE_UNSUPPORTED_SHAPE}:legs={legs}"

    unknown = [a for a in adapters if a not in HARNESS_ACCEPTED_ADAPTERS]
    if unknown:
        return f"{CODE_UNSUPPORTED_ADAPTER}:" + "+".join(unknown)

    # The harness CAN encode this; only the export's literal comparison stops it.
    if any(a not in EXPORT_LITERAL_ADAPTERS for a in adapters):
        return CODE_LABEL_SPELLING + ":" + "+".join(adapters)

    if not amount_nonzero:
        return CODE_ZERO_AMOUNT
    return None


def build_window_sql(window: str = "2 hours") -> str:
    """The ONE statement this instrument sends. Read-only by construction.

    A single statement so every count shares one ``now()``: the window is a
    rolling 2 h slice of a live table, so separate queries drift apart within
    seconds (measured: 952 vs 962 distinct topologies four minutes apart). One
    snapshot, or the partition does not reconcile.
    """
    return (
        "WITH w AS (\n"
        "  SELECT DISTINCT ON (o.dex_a, o.token_in, o.token_out, o.route_metadata->'pool_addresses')\n"
        "         o.strategy_kind AS kind,\n"
        "         o.route_metadata->'dex_adapters' AS adapters,\n"
        "         (o.amount_in_wei IS NOT NULL AND o.amount_in_wei <> '0') AS amount_nonzero\n"
        "  FROM opportunities o\n"
        "  WHERE o.chain_id = 1\n"
        "    AND o.route_metadata IS NOT NULL\n"
        "    AND o.route_metadata ? 'dex_adapters'\n"
        f"    AND o.detected_at > now() - interval '{window}'\n"
        "  ORDER BY o.dex_a, o.token_in, o.token_out, o.route_metadata->'pool_addresses', o.detected_at DESC\n"
        ")\n"
        "SELECT jsonb_array_length(adapters) AS legs,\n"
        "       kind AS kind,\n"
        "       adapters::text AS adapters_json,\n"
        "       amount_nonzero AS amount_nonzero,\n"
        "       count(*) AS n\n"
        "FROM w\n"
        "GROUP BY 1, 2, 3, 4\n"
        "ORDER BY 5 DESC, 1;\n"
    )


def parse_rows(psql_stdout: str) -> list[dict]:
    """``-t -A -F'|'`` rows -> buckets. A malformed line is an error, not a guess."""
    rows: list[dict] = []
    for lineno, line in enumerate(psql_stdout.splitlines(), 1):
        line = line.strip()
        if not line:
            continue
        parts = line.split("|")
        if len(parts) != 5:
            raise ValueError(f"line {lineno}: expected 5 |-separated fields, got {len(parts)}: {line!r}")
        legs_raw, kind, adapters_json, amount_raw, n_raw = parts
        legs: int | None = None if legs_raw.strip() in ("", "NULL") else int(legs_raw)
        try:
            adapters = json.loads(adapters_json)
        except json.JSONDecodeError as exc:
            raise ValueError(f"line {lineno}: dex_adapters is not JSON: {exc}: {adapters_json!r}") from exc
        if not isinstance(adapters, list):
            raise ValueError(f"line {lineno}: dex_adapters is not a JSON array: {adapters_json!r}")
        rows.append(
            {
                "legs": legs,
                "kind": kind,
                # psql renders a boolean as `t`/`f` with -A.
                "adapters": [str(a) for a in adapters],
                "amount_nonzero": amount_raw.strip() in ("t", "true"),
                "n": int(n_raw),
            }
        )
    return rows


def compute_ceiling(rows: list[dict]) -> dict:
    """Partition the window into named buckets and decide the coverage verdict.

    INVARIANT (asserted by the tests, not merely intended): the sum of every
    named bucket equals ``window_total``. A discard that escaped into an unnamed
    remainder has no representation in this structure.
    """
    buckets: dict[tuple, dict] = {}
    window_total = 0
    labelable = 0
    for r in rows:
        n = int(r["n"])
        window_total += n
        code = classify(r.get("legs"), list(r.get("adapters") or []), bool(r.get("amount_nonzero")))
        if code is None:
            labelable += n
            continue
        adapters = list(r.get("adapters") or [])
        kind = r.get("kind")
        # The bucket key is the FULL (code, kind, adapters) tuple, never the code
        # alone. Keying on the code would merge `["UniswapV2","UniswapV3"]` with
        # `["SushiSwap","UniswapV3"]` under one `unsupported_adapter:UniswapV3`
        # line and print the FIRST set's adapters against their SUMMED count — a
        # line that names a reason the count does not belong to. That is the very
        # aggregate-without-a-reason defect this instrument removes, so it is
        # structurally impossible here (and `test_bucket_never_merges_two_adapter_sets`
        # fails if a future edit reintroduces it).
        key = (code, kind, json.dumps(adapters, ensure_ascii=False, separators=(",", ":")))
        slot = buckets.setdefault(
            key, {"code": code, "kind": kind, "adapters": adapters, "count": 0}
        )
        slot["count"] += n

    discarded = window_total - labelable
    ordered = sorted(buckets.values(), key=lambda b: (-b["count"], b["code"], json.dumps(b["adapters"])))
    # Code-level roll-up: every term is itself a named code (never a generic residue).
    code_totals: dict[str, int] = {}
    for b in ordered:
        code_totals[b["code"]] = code_totals.get(b["code"], 0) + b["count"]

    # Structural degradation — NOT a threshold. One discarded topology is already
    # enough that the readings cannot stand as a verdict about the window.
    if window_total == 0:
        verdict = "NO_COMPUTADO"
        reason = "empty_window: 0 distinct topologies measured — there is nothing for a verdict to be about"
    elif discarded == 0:
        verdict = "COMPUTADO"
        reason = "encoder scope covers every topology of the window"
    else:
        verdict = "NO_COMPUTADO"
        reason = (
            f"encoder_scope: supported_adapters={'/'.join(EXPORT_LITERAL_ADAPTERS)} "
            f"route_shape={SUPPORTED_ROUTE_SHAPE} excludes {discarded} of {window_total} "
            f"window topologies ({100.0 * discarded / window_total:.1f}%); "
            "the readings describe the encoder, not the window"
        )

    shapes: dict[str, int] = {}
    for b in ordered:
        if b["code"].startswith(CODE_UNSUPPORTED_SHAPE + ":"):
            key = b["code"].split(":", 1)[1]
            shapes[key] = shapes.get(key, 0) + b["count"]

    return {
        "window_total": window_total,
        "labelable": labelable,
        "discarded": discarded,
        "share_labelable": (round(labelable / window_total, 6) if window_total else None),
        "share_discarded": (round(discarded / window_total, 6) if window_total else None),
        "buckets": ordered,
        "code_totals": dict(sorted(code_totals.items(), key=lambda kv: (-kv[1], kv[0]))),
        "shape_breakdown": dict(sorted(shapes.items())),
        "verdict": verdict,
        "verdict_reason": reason,
        # Reconciled by construction; carried in the output so a reader can check
        # the sum without re-deriving it.
        "named_total": labelable + sum(b["count"] for b in ordered),
    }


def parse_harness_skips(harness_log: str) -> list[dict]:
    """``VARIANCE_BENCH_SKIP={...}`` lines -> the harness' own per-sample names.

    The harness-side axis: why the harness discarded a sample it DID receive.
    An empty list is a real state — the export pre-filters on the same encoder
    scope, so the harness normally receives none of the discarded rows — and is
    reported as "no harness-internal discard", never as "zero failures".
    """
    out: list[dict] = []
    marker = "VARIANCE_BENCH_SKIP="
    for line in harness_log.splitlines():
        idx = line.find(marker)
        if idx < 0:
            continue
        payload = line[idx + len(marker):].strip()
        try:
            obj = json.loads(payload)
        except json.JSONDecodeError:
            out.append({"opportunity_id": None, "code": "malformed_skip_line", "raw": payload[:200]})
            continue
        if isinstance(obj, dict):
            out.append(obj)
    return out


def distribution(counts: dict[str, int]) -> str:
    """Whitespace-safe one-line roll-up.

    A discard code is not whitespace-free: the production adapter label
    `PancakeSwap V3` contains a space, and it is used VERBATIM on purpose. A
    naive `awk`-style split of the roll-up would therefore invent a phantom
    `V3` term. Every term is `shlex.quote`d so the line round-trips through
    `shlex.split`; the authoritative, delimiter-free form is the per-code
    `..._CODE_TOTAL code=<code> count=<n>` line emitted alongside it.
    """
    return " ".join(
        f"{shlex.quote(k)}={v}" for k, v in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))
    )


def count_codes(skips: list[dict]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for s in skips:
        code = str(s.get("code", "unnamed"))
        counts[code] = counts.get(code, 0) + 1
    return counts


def render_lines(ceiling: dict, harness_skips: list[dict]) -> list[str]:
    lines: list[str] = [
        "COVERAGE_CEILING_ENCODER "
        f"harness_accepted_adapters={','.join(HARNESS_ACCEPTED_ADAPTERS)} "
        f"export_literal_adapters={','.join(EXPORT_LITERAL_ADAPTERS)} "
        f"supported_route_shape={SUPPORTED_ROUTE_SHAPE} "
        f"harness_source={HARNESS_ADAPTER_SOURCE} "
        f"export_filter_source={EXPORT_ENCODER_FILTER_SOURCE}"
    ]
    # ONE LINE PER (reason, kind, adapter set) BUCKET -- never merged. The count on
    # a line always belongs to the adapter set printed on that same line.
    for b in ceiling["buckets"]:
        lines.append(
            "coverage_discard code={code} kind={kind} adapters={adapters} count={count}".format(
                code=b["code"],
                kind=b["kind"],
                adapters=json.dumps(b["adapters"], ensure_ascii=False, separators=(",", ":")),
                count=b["count"],
            )
        )
    # Per-code totals: one line per code, the authoritative machine-readable form
    # (a code may contain a space -- `PancakeSwap V3` -- so the single-line
    # roll-up below is shlex-quoted and this is the delimiter-free version).
    for code, n in ceiling["code_totals"].items():
        lines.append(f"COVERAGE_DISCARD_CODE_TOTAL code={code} count={n}")
    lines.append(
        "COVERAGE_DISCARD_DISTRIBUTION total={d} {dist}".format(
            d=ceiling["discarded"],
            dist=distribution(ceiling["code_totals"]) or "(none)",
        )
    )
    lines.append(
        "COVERAGE_CEILING window_total={t} labelable={l} discarded={d} named_total={nt} "
        "share_labelable={sl} share_discarded={sd} shape_breakdown={sb}".format(
            t=ceiling["window_total"],
            l=ceiling["labelable"],
            d=ceiling["discarded"],
            nt=ceiling["named_total"],
            sl=ceiling["share_labelable"],
            sd=ceiling["share_discarded"],
            sb=json.dumps(ceiling["shape_breakdown"], separators=(",", ":")),
        )
    )
    lines.append(f"COVERAGE_VERDICT={ceiling['verdict']} reason={ceiling['verdict_reason']}")
    lines.append(
        "COVERAGE_DEGRADED_FIELD field=coverage reads_as=market_result "
        f"measures=the_encoder_not_the_market verdict={ceiling['verdict']} source={COVERAGE_FIELD_SOURCE}"
    )
    # Harness-side axis, kept separate from the encoder axis on purpose.
    if harness_skips:
        skip_codes = [str(s.get("code", "unnamed")) for s in harness_skips]
        skip_counts: dict[str, int] = {}
        for c in skip_codes:
            skip_counts[c] = skip_counts.get(c, 0) + 1
        for code, n in sorted(skip_counts.items(), key=lambda kv: (-kv[1], kv[0])):
            lines.append(f"HARNESS_SKIP_CODE_TOTAL code={code} count={n}")
        lines.append(
            f"HARNESS_SKIP_DISTRIBUTION total={len(harness_skips)} {distribution(skip_counts)}"
        )
        for s in harness_skips:
            lines.append(f"harness_skip opportunity_id={s.get('opportunity_id')} code={s.get('code')}")
    else:
        lines.append(
            "HARNESS_SKIP_DISTRIBUTION total=0 (no harness-internal discard observed in this run - "
            "the export pre-filters on the same encoder scope, so the harness normally receives none "
            "of the discarded rows; this is NOT a statement that nothing was discarded)"
        )
    # CI logs are consumed by bash pipelines and greps; keep every emitted line
    # plain ASCII so an encoding round-trip can never mangle a code a test greps
    # for. (Declared because it is a real constraint, not cosmetics.)
    return [ln.encode("ascii", "replace").decode("ascii") for ln in lines]


def render_markdown(ceiling: dict, harness_skips: list[dict], window: str, source: str) -> str:
    md: list[str] = [
        "## G-SIM-1 coverage ceiling — counted and named (read-only)",
        "",
        f"- window: `{window}` rolling, chain 1, one snapshot shared by every count",
        f"- measured from: `{source}`",
        f"- encoder scope: `{'/'.join(EXPORT_LITERAL_ADAPTERS)}`, route shape `{SUPPORTED_ROUTE_SHAPE}`",
        f"- window topologies: **{ceiling['window_total']}**",
        f"- labelable by the benchmark encoder: **{ceiling['labelable']}**",
        f"- discarded by the encoder: **{ceiling['discarded']}**",
        "",
        f"**COVERAGE_VERDICT = `{ceiling['verdict']}`** — {ceiling['verdict_reason']}",
        "",
        "| named discard reason | kind | adapters | count |",
        "|---|---|---|---|",
    ]
    for b in ceiling["buckets"]:
        md.append(
            f"| `{b['code']}` | `{b['kind']}` | `{', '.join(b['adapters'])}` | {b['count']} |"
        )
    md += [
        "",
        f"Every named bucket sums to `window_total={ceiling['window_total']}` "
        f"(checked sum `named_total={ceiling['named_total']}`) — a discard that escaped into an "
        "unnamed remainder would break that sum, and the unit test fails on it.",
        "",
        "### What this does NOT claim",
        "",
        "`unsupported_adapter:<label>` is the **encoder's** limitation: it is not evidence that the "
        "market held a profitable route there. The production state of those same rows "
        "(`status`/`rejection_reason`) is a different axis, measured by the per-sample attribution step.",
        "",
        "This instrument makes the ceiling VISIBLE; it does not widen it. The labelable population, "
        "`min_samples` and the drift threshold are untouched.",
        "",
    ]
    if harness_skips:
        md.append(f"### Harness-internal discard (`VARIANCE_BENCH_SKIP`): {len(harness_skips)} sample(s)")
    else:
        md.append("### Harness-internal discard (`VARIANCE_BENCH_SKIP`): none observed in this run")
    md.append("")
    return "\n".join(md) + "\n"


def run_psql(sql: str, ssh_prefix: list[str], pg_container: str) -> tuple[int, str, str]:
    """Send the single SELECT through the documented channel.

    Both the transport rc and stderr are returned so a transport failure can never
    be read as an empty result (an absent datum is not a zero — R8/R9).
    """
    remote = (
        f"docker exec -e PGTZ=UTC {shlex.quote(pg_container)} "
        f"psql -U postgres -d arbitragex -t -A -F'|' -c {shlex.quote(sql)}"
    )
    proc = subprocess.run(ssh_prefix + [remote], capture_output=True, text=True, check=False)
    return proc.returncode, proc.stdout, proc.stderr


def ssh_prefix_for(args: argparse.Namespace) -> list[str]:
    """Build the ssh argv.

    Two forms, one path: the workflow supplies host/user/key from secrets, while
    `--ssh-target <alias>` uses an existing `~/.ssh/config` alias so the live path
    can be exercised (and verified) without the CI key.
    """
    if args.ssh_target:
        return [
            "ssh",
            "-p",
            str(args.vps_port),
            "-o",
            "BatchMode=yes",
            args.ssh_target,
        ]
    return [
        "ssh",
        "-i",
        os.path.expanduser(args.ssh_key),
        "-p",
        str(args.vps_port),
        "-o",
        "BatchMode=yes",
        "-o",
        "IdentitiesOnly=yes",
        f"{args.vps_user}@{args.vps_host}",
    ]


def _unavailable(reason: str, harness_skips: list[dict], job_summary: str) -> int:
    """Every unmeasured path ends here: literal reason + ::warning:: + exit 0."""
    print(f"::warning::coverage ceiling NOT MEASURED - {reason}")
    print(f"COVERAGE_CEILING_UNAVAILABLE reason={reason}")
    print(
        "COVERAGE_VERDICT=NO_COMPUTADO reason="
        f"not_measured:{reason} - a missing measurement is never a passing coverage verdict"
    )
    if harness_skips:
        skips_counts = count_codes(harness_skips)
        for code, n in sorted(skips_counts.items(), key=lambda kv: (-kv[1], kv[0])):
            print(f"HARNESS_SKIP_CODE_TOTAL code={code} count={n}")
        print(f"HARNESS_SKIP_DISTRIBUTION total={len(harness_skips)} {distribution(skips_counts)}")
    if job_summary and job_summary != "/dev/null":
        try:
            with open(job_summary, "a", encoding="utf-8") as fh:
                fh.write(
                    "## G-SIM-1 coverage ceiling - NOT MEASURED\n\n"
                    f"`COVERAGE_VERDICT = NO_COMPUTADO` - {reason}\n\n"
                    "A missing measurement is not a passing coverage verdict.\n"
                )
        except OSError as exc:
            print(f"::warning::job summary not written ({exc})")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--from-fixture", default="", help="JSON {rows:[...]} instead of a live query (tests/offline)")
    ap.add_argument("--harness-log", default="", help="driver log to mine for VARIANCE_BENCH_SKIP lines")
    ap.add_argument("--window", default="2 hours", help="window interval (must match the export's)")
    ap.add_argument("--vps-host", default=os.environ.get("VPS_HOST", ""))
    ap.add_argument("--vps-user", default=os.environ.get("VPS_USER", ""))
    ap.add_argument("--vps-port", default=os.environ.get("VPS_PORT", "22"))
    ap.add_argument("--ssh-key", default=os.environ.get("SSH_KEY", "~/.ssh/deploy_key"))
    ap.add_argument(
        "--ssh-target",
        default=os.environ.get("SSH_TARGET", ""),
        help="an ~/.ssh/config alias; overrides --vps-host/--vps-user/--ssh-key (local verification)",
    )
    ap.add_argument("--pg-container", default=os.environ.get("PG_CONTAINER", "arbitragex-v2-postgres-1"))
    ap.add_argument("--job-summary", default="", help="append the markdown block here (GITHUB_STEP_SUMMARY)")
    ap.add_argument("--out-json", default="", help="write the machine-readable ceiling + verdict here")
    args = ap.parse_args(argv)

    harness_skips: list[dict] = []
    if args.harness_log:
        try:
            with open(args.harness_log, encoding="utf-8", errors="replace") as fh:
                harness_skips = parse_harness_skips(fh.read())
        except OSError as exc:
            print(f"::warning::harness log unreadable ({exc}) — harness-side discard NOT measured this run")

    if args.from_fixture:
        try:
            with open(args.from_fixture, encoding="utf-8") as fh:
                payload = json.load(fh)
        except (OSError, json.JSONDecodeError) as exc:
            return _unavailable(f"fixture_unreadable:{exc}", harness_skips, args.job_summary)
        rows = list(payload.get("rows") or [])
        source = f"fixture:{os.path.basename(args.from_fixture)}"
    else:
        if not args.ssh_target and (not args.vps_host or not args.vps_user):
            return _unavailable(
                "ssh_credentials_absent(VPS_HOST/VPS_USER unset and no --ssh-target)",
                harness_skips,
                args.job_summary,
            )
        ssh_prefix = ssh_prefix_for(args)
        rc, out, err = run_psql(build_window_sql(args.window), ssh_prefix, args.pg_container)
        print(f"coverage_ceiling_transport ssh_and_psql_rc={rc} stdout_lines={len(out.splitlines())}")
        if err.strip():
            print(f"coverage_ceiling_transport_stderr {err.strip()[:400]}")
        if rc != 0:
            return _unavailable(f"query_failed_rc_{rc}", harness_skips, args.job_summary)
        try:
            rows = parse_rows(out)
        except ValueError as exc:
            return _unavailable(f"response_unparseable:{exc}", harness_skips, args.job_summary)
        source = f"live:chain1:window={args.window}"

    ceiling = compute_ceiling(rows)
    for line in render_lines(ceiling, harness_skips):
        print(line)
    print(f"coverage_ceiling_source {source} buckets={len(rows)}")

    if ceiling["verdict"] != "COMPUTADO":
        print(
            "::warning::COVERAGE VERDICT DEGRADED to NO_COMPUTADO - "
            f"{ceiling['verdict_reason']}. The `coverage` field of the registry row is a statement "
            "about the benchmark's encoder, NOT a result of the market. The gate thresholds "
            "(min_samples, drift) are NOT moved by this warning."
        )

    if args.out_json:
        try:
            with open(args.out_json, "w", encoding="utf-8") as fh:
                json.dump(
                    {
                        "source": source,
                        "window": args.window,
                        "coverage_ceiling": ceiling,
                        "harness_skip_distribution": (
                            count_codes(harness_skips) if harness_skips else None
                        ),
                        "harness_skip_total": len(harness_skips),
                    },
                    fh,
                    indent=2,
                )
                fh.write("\n")
        except OSError as exc:
            print(f"::warning::json artifact not written ({exc})")

    if args.job_summary and args.job_summary != "/dev/null":
        try:
            with open(args.job_summary, "a", encoding="utf-8") as fh:
                fh.write(render_markdown(ceiling, harness_skips, args.window, source))
        except OSError as exc:
            print(f"::warning::job summary not written ({exc})")

    # Exit 0 on EVERY path by contract — see the module docstring.
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
