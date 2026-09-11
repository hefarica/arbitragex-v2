from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import contextlib
import io
import json
import os
from pathlib import Path
import socket
import subprocess
import threading
import unittest
from unittest.mock import patch

from evidence_delivery import (ContainerTransport, DeliveryError, HttpTransport,
    NODE_REQUEST, configured_urls, deliver_routes, docker_routes, endpoint_url, ssh_delivery)


PAYLOAD = dict(gate_id="G-SIM-1", item_key="unit_tests", status="evidenced",
               evidence_ref="run-fixture", verified_by="test", detail={"source_sha": "a" * 40})


class RegistryHandler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, code, data):
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def do_GET(self):
        if self.path.startswith("/redirect/"):
            self.send_response(302)
            self.send_header("Location", self.server.other_url)
            self.end_headers()
            return
        if not self.path.startswith("/admin/readiness-evidence"):
            self.reply(404, {"error": "not_found"})
        elif self.headers.get("x-arbx-admin-token") != "fixture-token":
            self.reply(401, {"error": "unauthorized"})
        elif self.server.wrong_contract:
            self.reply(200, {"ok": True})
        else:
            self.reply(200, {"gate_id": "G-SIM-1", "items": self.server.rows})

    def do_POST(self):
        self.server.writes += 1
        payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        if self.server.store_writes:
            self.server.rows = [dict(payload, is_fresh=True)]
        if self.server.lose_response:
            self.connection.shutdown(socket.SHUT_RDWR)
            self.connection.close()
            return
        self.reply(201, {"ok": True, "gate_id": payload["gate_id"], "item_key": payload["item_key"]})


class DeliveryTests(unittest.TestCase):
    def setUp(self):
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), RegistryHandler)
        self.server.rows, self.server.writes = [], 0
        self.server.wrong_contract = False
        self.server.store_writes, self.server.lose_response = True, False
        self.server.other_url = "http://127.0.0.1:1/admin/readiness-evidence"
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.origin = f"http://127.0.0.1:{self.server.server_port}"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def transport(self, path="/admin/readiness-evidence", token="fixture-token"):
        return HttpTransport("http-test", self.origin + path, token)

    def test_wrong_path_falls_back_to_active_path_and_verifies_readback(self):
        report = []
        self.assertTrue(deliver_routes([self.transport("/old/admin/readiness-evidence"), self.transport()], PAYLOAD, report))
        self.assertEqual(report[0]["error"], "http_404")
        self.assertEqual(report[-1]["result"], "recorded_and_verified")
        self.assertEqual(self.server.writes, 1)

    def test_auth_failure_can_use_another_transport_without_removing_auth(self):
        report = []
        self.assertTrue(deliver_routes([self.transport(token="old-token"), self.transport()], PAYLOAD, report))
        self.assertEqual(report[0]["error"], "http_401")
        self.assertNotIn("old-token", json.dumps(report))

    def test_lost_post_response_and_retry_do_not_duplicate_confirmed_write(self):
        self.server.lose_response = True
        report = []
        self.assertTrue(deliver_routes([self.transport()], PAYLOAD, report))
        self.assertEqual(report[-1]["result"], "confirmed_after_lost_response")
        self.assertTrue(deliver_routes([self.transport()], PAYLOAD, []))
        self.assertEqual(self.server.writes, 1)

    def test_success_acknowledgement_without_persistence_is_failure(self):
        self.server.store_writes = False
        report = []
        self.assertFalse(deliver_routes([self.transport()], PAYLOAD, report))
        self.assertEqual(report[-1]["error"], "write_not_confirmed_by_readback")

    def test_generic_health_json_is_not_a_registry(self):
        self.server.wrong_contract = True
        self.assertFalse(deliver_routes([self.transport()], PAYLOAD, []))
        self.assertEqual(self.server.writes, 0)

    def test_redirect_is_not_followed_with_admin_credentials(self):
        report = []
        self.assertFalse(deliver_routes([self.transport("/redirect/admin/readiness-evidence")], PAYLOAD, report))
        self.assertEqual(report[0]["error"], "http_302")
        self.assertEqual(self.server.writes, 0)

    def runtime(self):
        return {"Id": "1" * 64, "State": {"Running": True},
            "Config": {"Env": ["API_PORT=9123", "ARBX_ADMIN_TOKEN=fixture-token", "ARBX_DEPLOY_SHA=" + "a" * 40]},
            "NetworkSettings": {"Ports": {"9123/tcp": [{"HostIp": "0.0.0.0", "HostPort": "49155"}]},
                                "Networks": {"runtime": {"IPAddress": "172.20.0.3"}}}}

    def test_docker_topology_uses_current_ports_and_has_container_fallback(self):
        routes = docker_routes(self.runtime(), 0, "a" * 40)
        self.assertEqual(routes[0].url, "http://127.0.0.1:49155/admin/readiness-evidence")
        self.assertEqual(routes[1].url, "http://172.20.0.3:9123/admin/readiness-evidence")
        self.assertIsInstance(routes[2], ContainerTransport)
        self.assertEqual(routes[2].port, 9123)
        container = self.runtime()
        container["NetworkSettings"] = {"Ports": {}, "Networks": {}}
        self.assertIsInstance(docker_routes(container, 0, "a" * 40)[0], ContainerTransport)

    def test_wrong_deployment_stopped_container_or_missing_role_token_is_rejected(self):
        for change in ("sha", "token", "port", "stopped"):
            c = self.runtime()
            if change == "stopped":
                c["State"]["Running"] = False
            else:
                key = {"sha": "ARBX_DEPLOY_SHA", "token": "ARBX_ADMIN_TOKEN", "port": "API_PORT"}[change]
                c["Config"]["Env"] = [row for row in c["Config"]["Env"] if not row.startswith(key + "=")]
            with self.subTest(change=change), self.assertRaises(DeliveryError):
                docker_routes(c, 0, "a" * 40)

    def test_container_node_transport_against_real_http_socket(self):
        def run_node(args, data=None, timeout=15):
            result = subprocess.run(["node", "-e", NODE_REQUEST], input=data, text=True,
                capture_output=True, timeout=timeout, env={**os.environ, "ARBX_ADMIN_TOKEN": "fixture-token"})
            self.assertEqual(result.returncode, 0, result.stderr)
            return result.stdout
        route = ContainerTransport("container-test", "1" * 64, self.server.server_port)
        with patch("evidence_delivery.command", side_effect=run_node):
            self.assertTrue(deliver_routes([route], PAYLOAD, []))
        self.assertEqual(self.server.writes, 1)

    def test_endpoint_configuration_is_bounded_and_cannot_embed_credentials(self):
        self.assertEqual(configured_urls("https://a.example/admin/readiness-evidence", '[]'),
                         ["https://a.example/admin/readiness-evidence"])
        for value in ("https://token@a.example/admin/readiness-evidence", "http://a.example/admin/readiness-evidence",
                      "https://a.example/health", "https://a.example/admin/readiness-evidence?token=secret"):
            with self.subTest(value=value), self.assertRaises(DeliveryError):
                endpoint_url(value)
        with self.assertRaises(DeliveryError):
            configured_urls("", json.dumps(["x"] * 9))

    def test_ssh_bootstrap_discovers_runtime_and_cleans_key_without_exporting_token(self):
        key_paths = []

        def ssh_command(args, data=None, timeout=15):
            key = Path(args[args.index("-i") + 1])
            key_paths.append(key)
            self.assertEqual(key.read_text().strip(), "fixture-private-key")
            self.assertNotIn("fixture-token", data)
            packet = json.loads(data)
            runtime = self.runtime()
            runtime["NetworkSettings"]["Ports"]["9123/tcp"][0]["HostPort"] = str(self.server.server_port)
            scope = {"__name__": "evidence_remote"}
            exec(compile(packet["source"], "evidence_delivery.py", "exec"), scope)

            def docker_command(argv, data=None, timeout=15):
                if argv[1] == "compose":
                    return "1" * 64 + "\n"
                self.assertEqual(argv[1], "inspect")
                return json.dumps([runtime])

            scope["command"] = docker_command
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                scope["remote_main"](packet)
            return out.getvalue()

        report = []
        env = dict(SSH_KEY="fixture-private-key", SSH_HOST="configured.example", SSH_USER="operator",
                   DEPLOY_PATH=str(Path(__file__).resolve().parents[2]))
        with patch("evidence_delivery.command", side_effect=ssh_command):
            self.assertTrue(ssh_delivery(PAYLOAD, env, report))
        self.assertEqual(self.server.writes, 1)
        self.assertFalse(any(key.exists() for key in key_paths))
        self.assertNotIn("fixture-token", json.dumps(report))


if __name__ == "__main__":
    unittest.main()
