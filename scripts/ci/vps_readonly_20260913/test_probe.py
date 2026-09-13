import importlib.util
from pathlib import Path
import sys
import unittest

spec = importlib.util.spec_from_file_location("probe", Path(__file__).with_name("probe.py"))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)

class ReadOnlyProbeTests(unittest.TestCase):
    def test_redis_only_persistence_allowlist(self):
        self.assertEqual(probe.parse_info("aof_last_write_status:ok\r\nprivate_key:SECRET\r\nrdb_last_bgsave_status:err"),
                         {"aof_last_write_status": "ok", "rdb_last_bgsave_status": "err"})
    def test_redis_noauth_not_green(self):
        self.assertEqual(probe.parse_info("NOAUTH Authentication required.\n"), {})
    def test_env_values_never_returned(self):
        self.assertEqual(probe.env_names("# BAD=secret\nRPC_HTTP_1=https://secret/\nexport GOOD=SECRET\n"),
                         {"RPC_HTTP_1", "GOOD"})
    def test_only_hex_ids(self):
        self.assertEqual(probe.identities("a"*12+"\n"+"b"*64), ["a"*12,"b"*64])
    def test_options_and_shell_rejected(self):
        for value in ["--all", "$(secret)", "a"*11, "a"*65, "not-a-container"]:
            with self.assertRaises(ValueError): probe.identities(value)
    def test_inventory_is_bounded(self):
        with self.assertRaises(ValueError): probe.identities(("a"*12+"\n")*101)
    def test_structured_output_drops_unknown_fields(self):
        result = probe.parse_lines({"ok":True, "text":'{"health":"healthy","secret":"HIDDEN"}'}, {"health"})
        self.assertEqual(result, [{"health":"healthy"}])
    def test_invalid_json_not_exposed(self):
        self.assertEqual(probe.parse_lines({"ok":True, "text":"SECRET-BAD-JSON"},{"state"}),
                         {"ok":False,"error":"invalid_json"})
    def test_arrays_not_accepted_as_inventory_objects(self):
        self.assertFalse(probe.parse_lines({"ok":True,"text":"[]"},{"state"})["ok"])
    def test_real_process_failure_hides_stdout_stderr(self):
        result = probe.command([sys.executable, "-c", "import sys; print('SECRET'); print('PRIVATE',file=sys.stderr); sys.exit(7)"])
        self.assertEqual(result, {"ok":False,"exit_code":7})
    def test_real_process_success(self):
        self.assertEqual(probe.command([sys.executable, "-c", "print('observed')"]), {"ok":True,"text":"observed\n"})
    def test_db_query_is_select_only(self):
        self.assertTrue(probe.SQL.lstrip().startswith("SELECT"))
        self.assertEqual(probe.SQL.count(';'),1)
        for token in ["TRUNCATE ","DELETE ","INSERT ","UPDATE ","VACUUM ","CHECKPOINT"]:
            self.assertNotIn(token,probe.SQL.upper())

if __name__ == '__main__': unittest.main(verbosity=2)
