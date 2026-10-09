#!/usr/bin/env python3
"""Tests for the G-SIM-1 coverage-ceiling instrument (GSIM-COVERAGE-CEILING-01).

Hermetic: no database, no network, no Docker. The REAL production snapshot lives
in `scripts/ci/fixtures/gsim1_coverage_window.json` (captured read-only from
`arbitragex-v2-postgres-1`; provenance + sha256 of the raw capture are inside it),
so the assertions are bound to measured bytes rather than to a hand-written
example.

Run:  python3 -m unittest discover -s scripts/ci -p 'test_gsim1_coverage_ceiling.py' -v

Two of these tests are the contract this task exists to install:

  * `test_unsupported_adapter_discard_carries_its_named_reason` and
    `test_no_emitted_code_is_a_bare_category_name` FAIL if the discard regresses
    to an aggregate without a reason.
  * `test_verdict_degrades_to_no_computado_on_the_real_window` FAILS if the
    coverage verdict can be published as a result of the market when the encoder
    covers 1.3% of the window.
"""

from __future__ import annotations

import importlib.util
import io
import json
import re
import shlex
import unittest
import contextlib
from pathlib import Path

CI_DIR = Path(__file__).resolve().parent
REPO_ROOT = CI_DIR.parents[1]
MODULE_PATH = CI_DIR / "gsim1_coverage_ceiling.py"
FIXTURE_PATH = CI_DIR / "fixtures" / "gsim1_coverage_window.json"
DRIVER_PATH = REPO_ROOT / "scripts" / "gsim1_variance_benchmark.sh"
HARNESS_PATH = REPO_ROOT / "backend" / "sim-core" / "tests" / "variance_benchmark.rs"
EXPORT_SQL_PATH = REPO_ROOT / "scripts" / "gsim1_variance_export.sql"


def load_module():
    spec = importlib.util.spec_from_file_location("gsim1_coverage_ceiling", MODULE_PATH)
    assert spec and spec.loader, f"cannot load {MODULE_PATH}"
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


CC = load_module()


def run_main(argv: list[str]) -> tuple[int, str]:
    """Invoke main() capturing stdout; returns (exit_code, stdout)."""
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = CC.main(argv)
    return rc, buf.getvalue()


def real_rows() -> list[dict]:
    with open(FIXTURE_PATH, encoding="utf-8") as fh:
        return list(json.load(fh)["rows"])


class NamedDiscardTests(unittest.TestCase):
    """R1 -- the discard must stop being an aggregate without a reason."""

    def test_unsupported_adapter_discard_carries_its_named_reason(self):
        """A sample with an unsupported adapter must emit its reason BY NAME.

        The input is exactly the shape production produces: two `UniswapV3`
        legs, and `PancakeSwap V3` spelled the way the scanner writes it. The
        label must survive verbatim -- normalising it would be the same defect
        wearing a different mask.
        """
        rows = [
            {"legs": 2, "kind": "dex_arb", "adapters": ["UniswapV3", "UniswapV3"],
             "amount_nonzero": True, "n": 46},
            {"legs": 2, "kind": "dex_arb", "adapters": ["PancakeSwap V3", "PancakeSwap V3"],
             "amount_nonzero": True, "n": 5},
            {"legs": 2, "kind": "dex_arb", "adapters": ["UniswapV2", "SushiSwap"],
             "amount_nonzero": True, "n": 7},
        ]
        lines = CC.render_lines(CC.compute_ceiling(rows), [])
        blob = "\n".join(lines)

        self.assertIn(
            'code=unsupported_adapter:UniswapV3+UniswapV3 kind=dex_arb '
            'adapters=["UniswapV3","UniswapV3"] count=46',
            blob,
            "the UniswapV3 discard must appear with its OWN reason and its OWN count",
        )
        self.assertIn(
            'code=unsupported_adapter:PancakeSwap V3+PancakeSwap V3',
            blob,
            "the production spelling `PancakeSwap V3` (with the space) must appear verbatim",
        )
        # The aggregate-without-a-reason form must NOT be what identifies a sample.
        for line in lines:
            if not line.startswith("coverage_discard "):
                continue
            self.assertNotRegex(
                line,
                r"code=unsupported_adapter kind=",
                f"a bare `unsupported_adapter` code is a category, not a reason: {line}",
            )

    def test_no_emitted_code_is_a_bare_category_name(self):
        """Every emitted code must carry a reason (`:`) or be a complete reason.

        `zero_amount_in` is a complete reason by itself; it is the ONLY code
        allowed to be a single token, and it is the export's own scope filter
        (not an encoder limit). Anything else without a `:` is the defect.
        """
        allowed_bare = {CC.CODE_ZERO_AMOUNT}
        lines = CC.render_lines(CC.compute_ceiling(real_rows()), [])
        codes = [ln.split("code=", 1)[1].split(" ", 1)[0] for ln in lines if ln.startswith("coverage_discard ")]
        self.assertGreater(len(codes), 0, "no coverage_discard line was emitted at all")
        for code in codes:
            if code in allowed_bare:
                continue
            self.assertIn(
                ":",
                code,
                f"discard code {code!r} names a category, not a reason",
            )
        # The single-line roll-up must be whitespace-safe: a code can hold a space
        # (`PancakeSwap V3` is used verbatim), so every term must be shlex-quoted
        # and must reconstruct the exact code -> count map.
        dist_line = next(ln for ln in lines if ln.startswith("COVERAGE_DISCARD_DISTRIBUTION "))
        terms = shlex.split(dist_line.split(" total=", 1)[1].split(" ", 1)[1])
        parsed = {}
        for term in terms:
            code, _, n = term.rpartition("=")
            parsed[code] = int(n)
        ceiling = CC.compute_ceiling(real_rows())
        self.assertEqual(
            parsed, ceiling["code_totals"],
            "the roll-up line does not reconstruct the code -> count map (a naive split "
            "would invent a phantom term from the space in `PancakeSwap V3`)",
        )
        self.assertIn(
            "PancakeSwap V3",
            " ".join(parsed),
            "the verbatim production label must survive the roll-up",
        )

        # The authoritative delimiter-free form: one line per code.
        totals = {}
        pat = re.compile(r"^COVERAGE_DISCARD_CODE_TOTAL code=(?P<code>.+) count=(?P<n>\d+)$")
        for ln in lines:
            if not ln.startswith("COVERAGE_DISCARD_CODE_TOTAL "):
                continue
            m = pat.match(ln)
            self.assertIsNotNone(m, f"unparseable CODE_TOTAL line: {ln!r}")
            totals[m.group("code")] = int(m.group("n"))
        self.assertEqual(totals, ceiling["code_totals"])
        self.assertEqual(sum(totals.values()), ceiling["discarded"])
        for code in totals:
            self.assertTrue(code in allowed_bare or ":" in code, f"unnamed code {code!r}")

    def test_bucket_never_merges_two_adapter_sets(self):
        """A count must never be printed next to an adapter set it does not belong to.

        `UniswapV2+UniswapV3`, `SushiSwap+UniswapV3` and `UniswapV3+UniswapV2`
        all collapse to the code prefix `unsupported_adapter:UniswapV3`. Keying
        buckets on that code would print the first set's adapters against the
        summed count of all three -- a named reason attached to the wrong
        samples. Every bucket line must be a distinct (code, kind, adapters).
        """
        ceiling = CC.compute_ceiling(real_rows())
        seen: set[tuple] = set()
        for b in ceiling["buckets"]:
            key = (b["code"], b["kind"], tuple(b["adapters"]))
            self.assertNotIn(key, seen, f"bucket {key} emitted twice")
            seen.add(key)
        # And the three same-code sets above must each have their own line.
        same_code = [b for b in ceiling["buckets"] if b["code"] == "unsupported_adapter:UniswapV3"]
        self.assertGreaterEqual(
            len(same_code), 2,
            "expected several distinct adapter sets sharing the UniswapV3 code in the real window",
        )
        self.assertEqual(
            len({tuple(b["adapters"]) for b in same_code}), len(same_code),
            "two different adapter sets share one bucket line",
        )

    def test_every_discard_is_named_and_the_buckets_reconcile(self):
        """No discard may escape into an unnamed remainder.

        The named buckets plus the labelable count must equal the window total.
        An unnamed remainder would make this sum short, which is the arithmetic
        form of "the discard is an aggregate again".
        """
        ceiling = CC.compute_ceiling(real_rows())
        self.assertEqual(
            ceiling["named_total"], ceiling["window_total"],
            "the named buckets do not reconcile to the window total",
        )
        self.assertEqual(
            ceiling["labelable"] + sum(b["count"] for b in ceiling["buckets"]),
            ceiling["window_total"],
        )
        self.assertEqual(
            ceiling["discarded"], sum(b["count"] for b in ceiling["buckets"])
        )
        self.assertEqual(
            sum(ceiling["code_totals"].values()), ceiling["discarded"],
            "the code roll-up does not sum to the discarded total",
        )

    def test_real_window_reports_the_measured_ceiling(self):
        """The fixture is the measured production snapshot; pin its headline numbers.

        Pinned so a silent change to the classification shows up as a red test
        with the old and new numbers side by side, instead of as a quietly
        different ceiling in a log nobody diffs.
        """
        with open(FIXTURE_PATH, encoding="utf-8") as fh:
            payload = json.load(fh)
        prov = payload["provenance"]
        ceiling = CC.compute_ceiling(payload["rows"])
        self.assertEqual(prov["base_commit"], "901eb947ff3bec359a3021a8db5b074e7080b79f")
        self.assertEqual(prov["chain_id"], 1)
        self.assertEqual(prov["window"], "2 hours rolling")
        self.assertEqual(ceiling["window_total"], 977)
        self.assertEqual(ceiling["labelable"], 13)
        self.assertEqual(ceiling["discarded"], 964)
        self.assertEqual(ceiling["shape_breakdown"], {"legs=3": 248, "legs=4": 352, "legs=5": 184, "legs=6": 9})
        # 13 labelable topologies out of 977 -- the ceiling is the encoder's.
        self.assertLess(ceiling["share_labelable"], 0.02)
        self.assertGreater(ceiling["share_discarded"], 0.98)


class DegradationTests(unittest.TestCase):
    """R10 -- the verdict must degrade where the coverage does not reach."""

    def test_verdict_degrades_to_no_computado_on_the_real_window(self):
        """On the measured window the verdict CANNOT be a result of the market."""
        ceiling = CC.compute_ceiling(real_rows())
        self.assertEqual(
            ceiling["verdict"], "NO_COMPUTADO",
            "13 of 977 labelable topologies must not produce a market verdict",
        )
        self.assertIn("encoder", ceiling["verdict_reason"])
        self.assertIn("964", ceiling["verdict_reason"])
        self.assertIn("977", ceiling["verdict_reason"])

        lines = CC.render_lines(ceiling, [])
        blob = "\n".join(lines)
        self.assertIn("COVERAGE_VERDICT=NO_COMPUTADO reason=", blob)
        # The field that reads as a market result must be named as degraded.
        degraded = next(ln for ln in lines if ln.startswith("COVERAGE_DEGRADED_FIELD "))
        self.assertIn("field=coverage", degraded)
        self.assertIn("reads_as=market_result", degraded)
        self.assertIn("measures=the_encoder_not_the_market", degraded)

    def test_degraded_field_is_the_one_the_driver_actually_emits(self):
        """The degraded field must BE the driver's `coverage`, with its real denominator.

        `scripts/gsim1_variance_benchmark.sh` computes
        `coverage = samples_labeled / population_size` where `population_size`
        is the ENCODER-FILTERED export. That is the field a reader takes for
        market coverage, so the instrument's degradation has to name it. If the
        driver's denominator is ever changed to a true window population, this
        test fails and forces the instrument to be re-reconciled instead of
        degrading a field that no longer exists in that form.
        """
        driver = DRIVER_PATH.read_text(encoding="utf-8")
        self.assertIn('detail["coverage"] = (', driver)
        self.assertRegex(
            driver,
            r'detail\["coverage"\]\s*=\s*\(\s*\n\s*round\(labeled / detail\["population_size"\], 4\)',
            "the driver's coverage denominator is no longer population_size",
        )
        self.assertIn('detail["population_size"] = int(population)', driver)
        # The instrument's provenance string must point at that same assignment.
        self.assertIn("gsim1_variance_benchmark.sh", CC.COVERAGE_FIELD_SOURCE)
        self.assertIn("samples_labeled / population_size", CC.COVERAGE_FIELD_SOURCE)

    def test_verdict_is_computado_when_the_encoder_covers_everything(self):
        """The degradation must be conditional, not hardcoded.

        A window that is entirely 2-leg and entirely inside the encoder's scope
        is the one case where a coverage verdict IS computable. If the verdict
        were hardcoded to NO_COMPUTADO this test would pass vacuously -- so it is
        paired with the real-window test above, which asserts the other branch.
        """
        rows = [
            {"legs": 2, "kind": "dex_arb", "adapters": ["UniswapV2", "SushiSwap"],
             "amount_nonzero": True, "n": 3},
            {"legs": 2, "kind": "dex_arb", "adapters": ["SushiSwap", "UniswapV2"],
             "amount_nonzero": True, "n": 2},
        ]
        ceiling = CC.compute_ceiling(rows)
        self.assertEqual(ceiling["discarded"], 0)
        self.assertEqual(ceiling["verdict"], "COMPUTADO")
        self.assertEqual(ceiling["labelable"], 5)

    def test_verdict_is_structural_not_a_threshold(self):
        """ONE discarded topology already degrades the verdict.

        A single `UniswapV3` sample among otherwise-encodable ones flips it. The
        criterion is `discarded == 0`, so no threshold was introduced and none of
        the existing ones (`min_samples`, drift) is touched by this instrument.
        """
        rows = [
            {"legs": 2, "kind": "dex_arb", "adapters": ["UniswapV2", "SushiSwap"],
             "amount_nonzero": True, "n": 1000},
            {"legs": 2, "kind": "dex_arb", "adapters": ["UniswapV3", "UniswapV3"],
             "amount_nonzero": True, "n": 1},
        ]
        ceiling = CC.compute_ceiling(rows)
        self.assertEqual(ceiling["discarded"], 1)
        self.assertEqual(ceiling["share_discarded"], round(1 / 1001, 6))
        self.assertEqual(
            ceiling["verdict"], "NO_COMPUTADO",
            "a 0.1% discard must still degrade: the criterion is structural",
        )

    def test_empty_window_is_no_computado_not_full_coverage(self):
        """0 topologies must never read as 'nothing was discarded'."""
        ceiling = CC.compute_ceiling([])
        self.assertEqual(ceiling["labelable"], 0)
        self.assertEqual(ceiling["discarded"], 0)
        self.assertIsNone(ceiling["share_labelable"])
        self.assertEqual(ceiling["verdict"], "NO_COMPUTADO")
        self.assertIn("empty_window", ceiling["verdict_reason"])

    def test_off_scope_route_shape_is_named_by_its_leg_count(self):
        """Multi-leg topology is named, and the leg count is in the code."""
        ceiling = CC.compute_ceiling(
            [{"legs": 4, "kind": "triangular",
              "adapters": ["uniswap-v3", "uniswap-v3", "uniswap-v3", "uniswap-v3"],
              "amount_nonzero": False, "n": 172}]
        )
        self.assertEqual(ceiling["buckets"][0]["code"], "unsupported_route_shape:legs=4")
        # The shape limit is thrown BEFORE the adapter/amount checks: this row is
        # discarded once, not twice, and the reason reported is the first one that
        # actually stops it.
        self.assertEqual(ceiling["discarded"], 172)

    def test_export_literal_mismatch_is_its_own_code(self):
        """A harness-encodable spelling the export drops must not be blamed on the encoder.

        `adapter_to_semantic()` accepts `uniswap-v2`, the export's jsonb
        containment accepts only `UniswapV2`. A 2-leg `uniswap-v2` topology is
        therefore labelable by the harness and dropped by the export -- an
        EXPORT-side gap. Folding it into `unsupported_adapter` would
        misattribute it to the encoder.
        """
        ceiling = CC.compute_ceiling(
            [{"legs": 2, "kind": "dex_arb", "adapters": ["uniswap-v2", "uniswap-v2"],
              "amount_nonzero": True, "n": 4}]
        )
        self.assertEqual(len(ceiling["buckets"]), 1)
        self.assertEqual(ceiling["buckets"][0]["code"], "adapter_label_spelling:export_literal_mismatch:uniswap-v2+uniswap-v2")
        self.assertNotIn("unsupported_adapter", ceiling["buckets"][0]["code"])


class HarnessSideNamingTests(unittest.TestCase):
    """The harness-side axis: per-sample names for the samples IT discards."""

    def test_harness_skip_lines_are_named_per_sample(self):
        log = (
            "running 1 test\n"
            'VARIANCE_BENCH_SKIP={"opportunity_id":"0249f64a-47de-4275-8eed-626f606cb82e",'
            '"code":"unsupported_adapter:UniswapV3"}\n'
            'VARIANCE_BENCH_SKIP={"opportunity_id":"082e835f-d80a-4b04-a35b-1b2ca52bb356",'
            '"code":"pred_failed:block=26145751"}\n'
            "VARIANCE_BENCH_JSON={}\n"
        )
        skips = CC.parse_harness_skips(log)
        self.assertEqual(len(skips), 2)
        lines = CC.render_lines(CC.compute_ceiling(real_rows()), skips)
        blob = "\n".join(lines)
        self.assertIn("HARNESS_SKIP_DISTRIBUTION total=2 ", blob)
        self.assertIn("unsupported_adapter:UniswapV3=1", blob)
        self.assertIn("pred_failed:block=26145751=1", blob)
        self.assertIn("harness_skip opportunity_id=0249f64a-47de-4275-8eed-626f606cb82e", blob)

    def test_absent_harness_skips_is_reported_as_not_observed(self):
        """Zero harness skips is NOT 'the harness discarded nothing' -- say which."""
        lines = CC.render_lines(CC.compute_ceiling(real_rows()), [])
        note = next(ln for ln in lines if ln.startswith("HARNESS_SKIP_DISTRIBUTION "))
        self.assertIn("total=0", note)
        self.assertIn("NOT a statement that nothing was discarded", note)

    def test_malformed_skip_line_is_named_not_dropped(self):
        skips = CC.parse_harness_skips('VARIANCE_BENCH_SKIP={"opportunity_id": broken\n')
        self.assertEqual(len(skips), 1)
        self.assertEqual(skips[0]["code"], "malformed_skip_line")


class AnomalyRouteTests(unittest.TestCase):
    """Every anomaly route: exit 0 plus a ::warning::. Never a red gate."""

    def test_missing_ssh_credentials_exits_zero_with_warning(self):
        rc, out = run_main(["--vps-host", "", "--vps-user", ""])
        self.assertEqual(rc, 0)
        self.assertIn("::warning::", out)
        self.assertIn("COVERAGE_CEILING_UNAVAILABLE reason=ssh_credentials_absent", out)
        self.assertIn("COVERAGE_VERDICT=NO_COMPUTADO", out)

    def test_unreadable_fixture_exits_zero_with_warning(self):
        rc, out = run_main(["--from-fixture", str(CI_DIR / "does-not-exist.json")])
        self.assertEqual(rc, 0)
        self.assertIn("::warning::", out)
        self.assertIn("COVERAGE_CEILING_UNAVAILABLE reason=fixture_unreadable", out)
        self.assertIn("COVERAGE_VERDICT=NO_COMPUTADO", out)

    def test_not_measured_is_never_a_passing_coverage_verdict(self):
        """The unavailable path must forbid the reading 'no ceiling problem found'."""
        _rc, out = run_main(["--vps-host", "", "--vps-user", ""])
        self.assertIn("a missing measurement is never a passing coverage verdict", out)

    def test_unparseable_response_names_the_reason(self):
        with self.assertRaises(ValueError):
            CC.parse_rows("2|dex_arb|[\"UniswapV2\"]|t")  # 4 fields, not 5

    def test_successful_fixture_run_exits_zero_and_warns_on_degradation(self):
        rc, out = run_main(["--from-fixture", str(FIXTURE_PATH), "--job-summary", "/dev/null"])
        self.assertEqual(rc, 0)
        self.assertIn("COVERAGE_VERDICT=NO_COMPUTADO", out)
        self.assertIn("::warning::COVERAGE VERDICT DEGRADED to NO_COMPUTADO", out)
        self.assertIn("gate thresholds", out)


class EncoderMirrorTests(unittest.TestCase):
    """The mirrored encoder contract must be checked against the real sources.

    A duplicate contract without a check is how a gate quietly starts enforcing
    something the code no longer means. Same pattern as
    `assert_mirror()` in `scripts/ci/gsim1_evidence_state.py`.
    """

    def test_harness_adapter_literals_match_adapter_to_semantic(self):
        src = HARNESS_PATH.read_text(encoding="utf-8")
        match = re.search(r"fn adapter_to_semantic.*?\n}\n", src, re.S)
        self.assertIsNotNone(match, "adapter_to_semantic() moved or was renamed")
        literals = set(re.findall(r'"([^"]+)"', match.group(0)))
        self.assertEqual(
            literals, set(CC.HARNESS_ACCEPTED_ADAPTERS),
            "the encoder's accepted adapter spellings changed -- re-derive "
            "HARNESS_ACCEPTED_ADAPTERS (a new V3/PancakeSwap arm means the ceiling moves: "
            "re-measure the coverage instead of silently widening this mirror)",
        )
        # The scopes must NOT be silently equal: V3 and PancakeSwap are absent from both.
        self.assertNotIn("UniswapV3", literals)
        self.assertNotIn("PancakeSwap V3", literals)

    def test_export_filter_and_shape_literals_match_the_sql(self):
        sql = EXPORT_SQL_PATH.read_text(encoding="utf-8")
        self.assertIn(
            "jsonb_array_length(o.route_metadata->'dex_adapters') = 2",
            sql,
            "the export's route-shape filter moved; SUPPORTED_ROUTE_SHAPE is now a stale mirror",
        )
        # The literal containment set must be exactly the two export labels.
        contained = re.search(r"<@\s+'(\[[^']*\])'::jsonb", sql)
        self.assertIsNotNone(contained, "the export's adapter containment filter moved")
        self.assertEqual(
            json.loads(contained.group(1)),
            list(CC.EXPORT_LITERAL_ADAPTERS),
            "the export's literal adapter set changed -- re-derive EXPORT_LITERAL_ADAPTERS",
        )

    def test_v3_and_pancake_are_still_unsupported_after_this_change(self):
        """Declared expectation: this change does NOT widen the encoder."""
        for unsupported in ("UniswapV3", "PancakeSwap V3"):
            self.assertIsNotNone(
                CC.classify(2, [unsupported, unsupported], True),
                f"{unsupported} must still be classified as discarded",
            )
        self.assertIsNone(CC.classify(2, ["UniswapV2", "SushiSwap"], True))


class OnlySelectIsEmitted(unittest.TestCase):
    """READ-ONLY BY CONSTRUCTION: the instrument can only send one SELECT.

    The workflow runs this against the production database, so the guarantee is
    asserted over the instrument's own source rather than promised in a comment.
    """

    def test_the_only_statement_is_a_single_read_only_select(self):
        sql = CC.build_window_sql()
        stripped = sql.strip()
        self.assertTrue(
            stripped.upper().startswith("WITH") or stripped.upper().startswith("SELECT"),
            "the instrument's SQL is not a SELECT/WITH statement",
        )
        self.assertEqual(stripped.count(";"), 1, "more than one statement is reachable")
        self.assertTrue(stripped.endswith(";"))
        for verb in ("INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "TRUNCATE", "GRANT", "COPY"):
            self.assertNotRegex(
                stripped.upper(),
                rf"\b{verb}\b",
                f"the instrument's SQL contains the write/DDL verb {verb}",
            )
        # And it must be the ONLY SQL in the module's live path.
        source = MODULE_PATH.read_text(encoding="utf-8")
        self.assertEqual(
            source.count("psql -U postgres"),
            1,
            "a second psql invocation appeared; re-verify that it is also SELECT-only",
        )
        self.assertIn("def build_window_sql(", source)

    def test_window_is_declared_and_matches_the_export(self):
        self.assertIn("interval '2 hours'", CC.build_window_sql())
        export = EXPORT_SQL_PATH.read_text(encoding="utf-8")
        self.assertIn("interval '2 hours'", export, "the export's window moved; keep the two in step")


if __name__ == "__main__":
    unittest.main(verbosity=2)
