#!/usr/bin/env python3
"""Tests V3 de los terminos ADMITIDOS t13 (adm/*). Ejecutar: python -m unittest discover -s adm -p 'test_*.py'"""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from econ_sizing import calibrate, kelly_cap, net_at, optimal_size, size_policy
from staleness_gate import apply as staleness_apply, gate as staleness_gate, staleness_threshold_ms
from nonatomic_gate import apply as nonatomic_apply, gate as nonatomic_gate


class TestADM1Sizing(unittest.TestCase):
    # calibracion desde el item medido 3 de la muestra (a0=2529.6636, net0=-45.9315,
    # slippage0=12.648318, lp0=7.5889908, costs=24.66828738) — x_ref/qa-baseline
    def test_calibrate_and_optimal_non_positive(self):
        cal = calibrate(2529.6636, -45.93151143, 12.648318, 7.5889907999999995, 24.66828738)
        self.assertIsNotNone(cal)
        # verificacion de calibracion: net(a0) == net0
        self.assertAlmostEqual(net_at(2529.6636, cal), -45.93151143, places=6)
        a_star, net_star = optimal_size(cal)
        # edge marginal negativo en la ventana => politica no opera
        pol = size_policy(cal)
        self.assertFalse(pol["trade"])
        self.assertEqual(pol["amount_usd"], 0.0)

    def test_synthetic_positive_edge(self):
        # net(a) = a*0.10 - a^2*0.001 - 1.0 => a*=50, net*=1.5
        cal = {"g": 0.10, "s_c": 0.001, "fixed": 1.0}
        a_star, net_star = optimal_size(cal)
        self.assertAlmostEqual(a_star, 50.0, places=9)
        self.assertAlmostEqual(net_star, 1.5, places=9)
        pol = size_policy(cal)
        self.assertTrue(pol["trade"])
        self.assertAlmostEqual(pol["amount_usd"], 50.0, places=9)

    def test_kelly_cap(self):
        self.assertAlmostEqual(kelly_cap(0.6, 1.0, 1000.0), 200.0, places=9)   # f*=0.2
        self.assertEqual(kelly_cap(0.4, 1.0, 1000.0), 0.0)                     # edge<=0
        self.assertIsNone(kelly_cap(0.6, 0.0, 1000.0))                         # dato invalido -> None (R2)
        # cota kelly recorta el optimo
        pol = size_policy({"g": 0.10, "s_c": 0.001, "fixed": 1.0}, kelly=30.0)
        self.assertTrue(pol["trade"])
        self.assertAlmostEqual(pol["amount_usd"], 30.0, places=9)

    def test_fail_honest(self):
        self.assertIsNone(calibrate(0.0, -1.0, 0.0, 0.0, 1.0))  # a0 invalido -> None


class TestADM3Staleness(unittest.TestCase):
    def test_gate(self):
        g = staleness_gate({"pipeline_latency_ms": 3000}, 2405.0)
        self.assertTrue(g["quarantine"])
        g = staleness_gate({"pipeline_latency_ms": 100}, 2405.0)
        self.assertFalse(g["quarantine"])

    def test_fail_honest_no_dato(self):
        g = staleness_gate({}, 2405.0)
        self.assertIsNone(g["quarantine"])
        self.assertIn("NO COMPUTADO", g["reason"])
        g = staleness_gate({"pipeline_latency_ms": 100}, None)
        self.assertIsNone(g["quarantine"])  # umbral env ausente => no computado

    def test_threshold_from_env(self):
        self.assertEqual(staleness_threshold_ms({"ARBX_STALENESS_MS": "2405"}), 2405.0)
        self.assertIsNone(staleness_threshold_ms({}))
        self.assertIsNone(staleness_threshold_ms({"ARBX_STALENESS_MS": "abc"}))

    def test_apply_muestra_congelada(self):
        # replay honesto del sustrato congelado (V4 shadow usa lo mismo)
        items = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "..",
                                            "qa-baseline", "edge_opps_live.json"), encoding="utf-8"))
        if isinstance(items, dict):
            items = items.get("items") or items.get("data") or []
        r = staleness_apply(items, 2405.0)
        self.assertEqual(r["n"], 26)
        self.assertEqual(r["quarantined"], 2)   # medido: 2/26 = 7.69% >= delta 2%
        self.assertEqual(r["not_computed"], 0)


class TestADM4NonAtomic(unittest.TestCase):
    def test_gate(self):
        self.assertTrue(nonatomic_gate({"exec_class": "NONATOMIC_BRIDGE_REQUIRED"})["quarantine"])
        self.assertTrue(nonatomic_gate({"exec_class": "DETERMINISTIC_EXECUTABLE", "bridge": "across"})["quarantine"])
        self.assertFalse(nonatomic_gate({"exec_class": "DETERMINISTIC_EXECUTABLE"})["quarantine"])
        g = nonatomic_gate({})
        self.assertIsNone(g["quarantine"])  # exec_class ausente => no computado (R2)

    def test_apply_taxonomy_a8(self):
        # fraccion medida en TESTS_ATOMICOS: 30000/264000
        items = [{"exec_class": c} for c in ["NONATOMIC_BRIDGE_REQUIRED"] * 30000 + ["SIM_REQUIRED"] * 34000]
        r = nonatomic_apply(items)
        self.assertEqual(r["quarantined"], 30000)
        self.assertEqual(r["n"], 64000)


if __name__ == "__main__":
    unittest.main(verbosity=2)
