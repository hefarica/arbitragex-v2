#!/usr/bin/env python3
"""Bounded, read-only VPS snapshot. No shell commands, writes or business rows.

Run by `ssh ... python3 - < probe.py`; all output is an allowlisted JSON report.
A successful capture does NOT certify a deployment or authorize financial activity.
"""
from __future__ import annotations

import base64
import datetime as dt
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import subprocess

TARGET = "f0daeb650b67b3e40c74e02ca62380c906c2b2b1"
PROJECT = Path("/opt/arbitragex-v2")
CORE = ("postgres", "redis", "api-server", "edge", "frontend", "searcher-rs",
        "sim-ctl", "relays-client", "recon", "selector-api", "math-engine", "token-enricher")
PERSISTENCE = {"loading", "aof_enabled", "aof_rewrite_in_progress", "aof_last_write_status",
               "aof_last_bgrewrite_status", "aof_current_size", "aof_base_size",
               "rdb_last_bgsave_status", "rdb_last_save_time"}
CONFIG_NAMES = ("RPC_HTTP_1", "RPC_WS_1", "RPC_HTTP_11155111", "RPC_WS_11155111",
                "ARBX_ACCOUNTING_FEEDS_JSON", "ARBX_LIVE_EXEC_ENABLED", "ARBX_LIVE_EXEC_CHAINS",
                "ARBX_STAGE2_CALIBRATION_MODE", "ARBX_PRIORS_REFRESH_SECS")
CONTAINER_FORMAT = ('{"id":{{json .Id}},"service":{{json (index .Config.Labels "com.docker.compose.service")}},'
    '"state":{{json .State.Status}},"health":{{if .State.Health}}{{json .State.Health.Status}}{{else}}null{{end}},'
    '"restarts":{{.RestartCount}},"oom":{{.State.OOMKilled}},"image_id":{{json .Image}},'
    '"started_at":{{json .State.StartedAt}}}')
IMAGE_FORMAT = ('{"image_id":{{json .Id}},"revision":{{json (index .Config.Labels "org.opencontainers.image.revision")}},'
                '"created_at":{{json .Created}}}')
SQL = """SELECT json_build_object(
 'version', current_setting('server_version'),
 'transaction_read_only', current_setting('transaction_read_only'),
 'in_recovery', pg_is_in_recovery(),
 'database_bytes', pg_database_size(current_database()),
 'waiting_locks', (SELECT count(*) FROM pg_locks WHERE NOT granted),
 'transactions_over_5m', (SELECT count(*) FROM pg_stat_activity WHERE xact_start < now()-interval '5 minutes'),
 'replication_slots', (SELECT count(*) FROM pg_replication_slots),
 'archiver_failed_count', (SELECT failed_count FROM pg_stat_archiver),
 'archiver_last_failed_time', (SELECT last_failed_time FROM pg_stat_archiver));"""


def command(argv: list[str], timeout: int = 8) -> dict:
    """Do not forward subprocess stderr, which may contain operational secrets."""
    try:
        result = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True,
                                text=True, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {"ok": False, "error": type(exc).__name__}
    if result.returncode:
        return {"ok": False, "exit_code": result.returncode}
    if len(result.stdout) > 1_000_000:
        return {"ok": False, "error": "output_limit"}
    return {"ok": True, "text": result.stdout}


def parse_info(text: str) -> dict:
    return {key: value for line in text.splitlines() if ':' in line
            for key, value in [line.split(':', 1)] if key in PERSISTENCE}


def env_names(text: str) -> set[str]:
    return set(re.findall(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=", text, re.M))


def identities(text: str) -> list[str]:
    values = text.split()
    if len(values) > 100 or any(not re.fullmatch(r"[0-9a-f]{12,64}", value) for value in values):
        raise ValueError("invalid_container_inventory")
    return values


def parse_lines(result: dict, allowed: set[str]) -> list[dict] | dict:
    if not result["ok"]:
        return result
    try:
        values = [json.loads(line) for line in result["text"].splitlines() if line.strip()]
        if any(not isinstance(value, dict) for value in values):
            raise ValueError("expected_objects")
        return [{key: val for key, val in value.items() if key in allowed} for value in values]
    except (ValueError, TypeError):
        return {"ok": False, "error": "invalid_json"}


def http_probe(port: int, path: str, websocket: bool = False) -> dict:
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=4)
    headers = {"User-Agent": "arbx-readonly-release-probe/1"}
    key = base64.b64encode(os.urandom(16)).decode("ascii")
    if websocket:
        headers.update({"Connection": "Upgrade", "Upgrade": "websocket",
                        "Sec-WebSocket-Version": "13", "Sec-WebSocket-Key": key})
    try:
        conn.request("GET", path, headers=headers)
        response = conn.getresponse()
        output = {"port": port, "path": path, "status": response.status}
        if websocket:
            expected = base64.b64encode(hashlib.sha1(
                (key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode("ascii")).digest()).decode("ascii")
            output["upgrade_verified"] = (response.status == 101
                and response.getheader("Sec-WebSocket-Accept") == expected
                and response.getheader("Upgrade", "").lower() == "websocket")
            output["scope"] = "transport_handshake_only_not_application_e2e"
        return output
    except (OSError, http.client.HTTPException) as exc:
        return {"port": port, "path": path, "error": type(exc).__name__}
    finally:
        conn.close()


def main() -> None:
    report: dict = {"schema": 1, "captured_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
                   "target_main_sha": TARGET, "scope": "read_only_snapshot",
                   "deployment_certification": "not_certified_by_this_capture"}
    fs = os.statvfs("/")
    report["root_filesystem"] = {"total_bytes": fs.f_blocks * fs.f_frsize,
                                 "available_bytes": fs.f_bavail * fs.f_frsize,
                                 "inodes_total": fs.f_files, "inodes_free": fs.f_favail}
    git = command(["git", "--no-optional-locks", "-C", str(PROJECT), "rev-parse", "HEAD"])
    sha = git.get("text", "").strip()
    report["checkout"] = ({"sha": sha, "equals_target": sha == TARGET,
                           "scope": "checkout_only_not_running_image"}
                          if re.fullmatch(r"[0-9a-f]{40}", sha) else {"observed": False})
    names_file = PROJECT / ".env"
    try:
        if names_file.is_symlink() or names_file.stat().st_size > 1_000_000:
            raise ValueError("env_file_shape")
        names = env_names(names_file.read_text(encoding="utf-8"))
        report["configuration_names_only"] = {name: name in names for name in CONFIG_NAMES}
    except (OSError, ValueError):
        report["configuration_names_only"] = {"observed": False}
    inventory = command(["docker", "ps", "-a", "--filter",
                         "label=com.docker.compose.project=arbitragex-v2", "--format", "{{.ID}}"])
    containers: list[dict] | dict = {"observed": False}
    if inventory["ok"]:
        try:
            ids = identities(inventory["text"])
            if ids:
                containers = parse_lines(command(["docker", "inspect", "--format", CONTAINER_FORMAT, *ids]),
                    {"id", "service", "state", "health", "restarts", "oom", "image_id", "started_at"})
        except ValueError:
            containers = {"observed": False, "error": "invalid_inventory"}
    else:
        containers = inventory
    report["containers"] = containers
    if isinstance(containers, list):
        image_ids = sorted({row["image_id"] for row in containers
                            if re.fullmatch(r"sha256:[0-9a-f]{64}", row.get("image_id", ""))})
        report["images"] = (parse_lines(command(["docker", "image", "inspect", "--format", IMAGE_FORMAT, *image_ids]),
                            {"image_id", "revision", "created_at"}) if image_ids else [])
        report["core_observations"] = {service: [row for row in containers if row.get("service") == service]
                                        for service in CORE}
        def running_id(service: str) -> str | None:
            matches = [row for row in containers if row.get("service") == service and row.get("state") == "running"]
            return matches[0]["id"] if len(matches) == 1 and re.fullmatch(r"[0-9a-f]{64}", matches[0]["id"]) else None
        pg = running_id("postgres")
        if pg:
            result = command(["docker", "exec", "-e",
                "PGOPTIONS=-c default_transaction_read_only=on -c statement_timeout=3000 -c lock_timeout=1000",
                pg, "psql", "-U", "postgres", "-d", "arbitragex", "-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-c", SQL])
            report["postgresql"] = parse_lines(result, {"version", "transaction_read_only", "in_recovery",
                "database_bytes", "waiting_locks", "transactions_over_5m", "replication_slots",
                "archiver_failed_count", "archiver_last_failed_time"})
        else:
            report["postgresql"] = {"observed": False, "reason": "no_unique_running_container"}
        redis = running_id("redis")
        if redis:
            result = command(["docker", "exec", redis, "redis-cli", "INFO", "persistence"])
            values = parse_info(result.get("text", ""))
            report["redis_persistence"] = values if values else {"observed": False, "reason": "info_unavailable"}
        else:
            report["redis_persistence"] = {"observed": False, "reason": "no_unique_running_container"}
    report["loopback_http"] = [http_probe(port, path) for port, path in [
        (8080, "/api/health"), (8787, "/health"), (5173, "/opportunities/exchange"), (80, "/")]]
    report["loopback_websocket"] = http_probe(80, "/socket.io/?EIO=4&transport=websocket", True)
    report["completed_at_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    print(json.dumps(report, ensure_ascii=True, indent=2))


if __name__ == "__main__":
    main()
