#!/usr/bin/env python3
"""Deliver evidence through configured HTTP or discovered VPS runtime transports.

No port scanning, credential logging, redirects or fabricated success. Each route
must implement the registry's authenticated GET/POST contract and read back the
stored payload. A retry reads first, including after a lost POST response.
"""
import http.client
import ipaddress
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

MAX_BODY = 131072
HTTP_TIMEOUT = 8
REGISTRY_PATH = "/admin/readiness-evidence"


class DeliveryError(Exception):
    pass


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def endpoint_url(value, runtime=False):
    """Only explicit URLs or addresses from the selected Docker service are used."""
    p = urllib.parse.urlsplit(value)
    if p.scheme not in ("http", "https") or not p.hostname or p.username or p.password or p.fragment:
        raise DeliveryError("invalid_endpoint")
    if p.scheme == "http" and not runtime:
        try:
            loopback = ipaddress.ip_address(p.hostname).is_loopback
        except ValueError:
            loopback = p.hostname == "localhost"
        if not loopback:
            raise DeliveryError("https_required_for_configured_endpoint")
    if p.query or not p.path.endswith(REGISTRY_PATH):
        raise DeliveryError("endpoint_must_name_readiness_registry")
    return value


class HttpTransport:
    def __init__(self, label, url, token, runtime=False):
        self.label = label
        self.url = endpoint_url(url, runtime)
        self.token = token
        self.opener = urllib.request.build_opener(NoRedirect)

    def request(self, method, gate, payload=None):
        if not self.token:
            raise DeliveryError("missing_admin_token")
        url = self.url
        if method == "GET":
            url += "?" + urllib.parse.urlencode({"gate_id": gate})
        request = urllib.request.Request(url, method=method,
            data=json.dumps(payload).encode() if payload is not None else None,
            headers={"Content-Type": "application/json", "x-arbx-admin-token": self.token})
        try:
            with self.opener.open(request, timeout=HTTP_TIMEOUT) as response:
                body = response.read(MAX_BODY + 1)
                if len(body) > MAX_BODY:
                    raise DeliveryError("response_too_large")
                return response.status, json.loads(body)
        except urllib.error.HTTPError as error:
            raise DeliveryError(f"http_{error.code}") from None
        except (urllib.error.URLError, TimeoutError, OSError, http.client.HTTPException):
            raise DeliveryError("connection_or_timeout") from None
        except (ValueError, UnicodeError):
            raise DeliveryError("invalid_json_response") from None


# The API image already includes Node. No image pull, installation or host port
# is required for this transport. Its admin token stays inside that container.
NODE_REQUEST = r'''
const http = require('http');
let input = '';
process.stdin.on('data', b => { input += b; if (input.length > 262144) process.exit(2); });
process.stdin.on('end', () => {
  const p = JSON.parse(input);
  const token = process.env.ARBX_ADMIN_TOKEN;
  if (!token) { process.stdout.write(JSON.stringify({error:'missing_admin_token'})); return; }
  const path = '/admin/readiness-evidence' + (p.method === 'GET' ? '?gate_id=' + encodeURIComponent(p.gate) : '');
  const body = p.payload === null ? null : JSON.stringify(p.payload);
  const headers = {'content-type':'application/json','x-arbx-admin-token':token};
  if (body !== null) headers['content-length'] = Buffer.byteLength(body);
  const req = http.request({hostname:'127.0.0.1', port:p.port, path, method:p.method,
    headers}, res => {
    let data = ''; let oversized = false;
    res.on('data', b => { data += b; if (data.length > 131072) { oversized = true; res.destroy(); } });
    res.on('end', () => {
      if (oversized) return;
      try { process.stdout.write(JSON.stringify({status:res.statusCode, body:JSON.parse(data)})); }
      catch { process.stdout.write(JSON.stringify({error:'invalid_json_response'})); }
    });
    res.on('error', () => process.stdout.write(JSON.stringify({error:'response_interrupted'})));
  });
  req.setTimeout(8000, () => req.destroy());
  req.on('error', () => process.stdout.write(JSON.stringify({error:'connection_or_timeout'})));
  if (body !== null) req.write(body);
  req.end();
});
'''


def command(args, data=None, timeout=15):
    try:
        result = subprocess.run(args, input=data, capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        raise DeliveryError("runtime_command_unavailable_or_timeout") from None
    if result.returncode:
        raise DeliveryError("runtime_command_failed")
    return result.stdout


class ContainerTransport:
    def __init__(self, label, container, port):
        self.label, self.container, self.port = label, container, port

    def request(self, method, gate, payload=None):
        packet = json.dumps(dict(method=method, gate=gate, payload=payload, port=self.port))
        output = command(["docker", "exec", "-i", self.container, "node", "-e", NODE_REQUEST], packet)
        try:
            result = json.loads(output)
            if result.get("error"):
                raise DeliveryError(result["error"])
            status = result["status"]
            if not 200 <= status < 300:
                raise DeliveryError(f"http_{status}")
            return status, result["body"]
        except (ValueError, KeyError, TypeError):
            raise DeliveryError("invalid_container_response") from None


def read_registry(transport, payload):
    status, body = transport.request("GET", payload["gate_id"])
    if (status != 200 or not isinstance(body, dict)
            or body.get("gate_id") != payload["gate_id"] or not isinstance(body.get("items"), list)):
        raise DeliveryError("registry_contract_mismatch")
    return body["items"]


def recorded(items, payload):
    fields = ("gate_id", "item_key", "status", "evidence_ref", "verified_by", "detail")
    return any(isinstance(row, dict) and row.get("is_fresh") is True
               and all(row.get(key) == payload.get(key) for key in fields) for row in items)


def deliver_one(transport, payload):
    if recorded(read_registry(transport, payload), payload):
        return "already_recorded"
    try:
        status, body = transport.request("POST", payload["gate_id"], payload)
        if (status not in (200, 201) or not isinstance(body, dict) or body.get("ok") is not True
                or body.get("gate_id") != payload["gate_id"] or body.get("item_key") != payload["item_key"]):
            raise DeliveryError("write_acknowledgement_mismatch")
    except DeliveryError:
        # The write may have committed before a connection was lost. Confirm it
        # before selecting another route; that route also reads before writing.
        if recorded(read_registry(transport, payload), payload):
            return "confirmed_after_lost_response"
        raise
    if not recorded(read_registry(transport, payload), payload):
        raise DeliveryError("write_not_confirmed_by_readback")
    return "recorded_and_verified"


def deliver_routes(routes, payload, report):
    deadline = time.monotonic() + 120
    for route in routes:
        if time.monotonic() > deadline - 3 * HTTP_TIMEOUT:
            report.append({"route": "remaining_routes", "error": "delivery_time_budget_exhausted"})
            return False
        try:
            result = deliver_one(route, payload)
            report.append({"route": route.label, "result": result})
            return True
        except DeliveryError as error:
            report.append({"route": route.label, "error": str(error)})
    return False


def configured_urls(single, multiple):
    try:
        values = json.loads(multiple or "[]")
    except ValueError:
        raise DeliveryError("invalid_endpoint_list") from None
    if not isinstance(values, list) or any(not isinstance(v, str) for v in values) or len(values) > 8:
        raise DeliveryError("invalid_endpoint_list")
    return list(dict.fromkeys(([single] if single else []) + values))


def docker_routes(container, index, expected_sha=None):
    if not container.get("State", {}).get("Running"):
        raise DeliveryError("api_container_not_running")
    env = dict(row.split("=", 1) for row in container.get("Config", {}).get("Env", []) if "=" in row)
    if expected_sha and env.get("ARBX_DEPLOY_SHA") != expected_sha:
        raise DeliveryError("api_container_deploy_sha_mismatch")
    port = env.get("API_PORT", "")
    if not port.isdigit() or not 0 < int(port) < 65536:
        raise DeliveryError("api_container_port_not_configured")
    token = env.get("ARBX_ADMIN_TOKEN", "")
    if not token:
        raise DeliveryError("api_container_missing_admin_token")
    cid = container.get("Id", "")
    if not re.fullmatch(r"[0-9a-f]{64}", cid):
        raise DeliveryError("invalid_container_identity")
    routes = []
    settings = container.get("NetworkSettings", {})
    for binding in (settings.get("Ports", {}).get(port + "/tcp") or [])[:4]:
        host, published = binding.get("HostIp", ""), binding.get("HostPort", "")
        if host in ("", "0.0.0.0"):
            host = "127.0.0.1"
        elif host == "::":
            host = "::1"
        try:
            ip = ipaddress.ip_address(host)
        except ValueError:
            continue
        if not published.isdigit() or not 0 < int(published) < 65536:
            continue
        address = f"[{ip}]" if ip.version == 6 else str(ip)
        routes.append(HttpTransport(f"ssh_published_{index}_{len(routes)}",
            f"http://{address}:{published}{REGISTRY_PATH}", token, runtime=True))
    # Bridge IPs are discovered only from this compose-selected container.
    for network in list(settings.get("Networks", {}).values())[:4]:
        host = network.get("IPAddress", "")
        try:
            address = ipaddress.ip_address(host)
        except ValueError:
            continue
        routes.append(HttpTransport(f"ssh_bridge_{index}_{len(routes)}",
            f"http://{address}:{port}{REGISTRY_PATH}", token, runtime=True))
    routes.append(ContainerTransport(f"ssh_container_{index}", cid, int(port)))
    return routes


def remote_main(packet):
    report = []
    try:
        root = Path(packet["deploy_path"]).resolve(strict=True)
        compose = (root / packet["compose_file"]).resolve(strict=True)
        if not compose.is_relative_to(root):
            raise DeliveryError("compose_file_outside_deployment")
        ids = command(["docker", "compose", "--env-file", str(root / ".env"),
                       "-f", str(compose), "ps", "-q", packet["api_service"]]).split()
        if not ids or len(ids) > 4 or any(not re.fullmatch(r"[0-9a-f]{64}", cid) for cid in ids):
            raise DeliveryError("running_api_service_not_found")
        payload = packet["payload"]
        expected = (payload.get("detail") or {}).get("source_sha")
        routes = []
        for i, cid in enumerate(ids):
            data = json.loads(command(["docker", "inspect", cid]))[0]
            try:
                routes.extend(docker_routes(data, i, expected))
            except DeliveryError as error:
                report.append({"route": f"ssh_runtime_{i}", "error": str(error)})
        ok = deliver_routes(routes, payload, report)
    except (DeliveryError, OSError, ValueError, KeyError, TypeError, IndexError) as error:
        report.append({"route": "ssh_discovery", "error": str(error) if isinstance(error, DeliveryError) else "invalid_runtime_configuration"})
        ok = False
    print(json.dumps({"ok": ok, "attempts": report}))


def ssh_delivery(payload, env, report):
    key, host, user = (env.get(k, "") for k in ("SSH_KEY", "SSH_HOST", "SSH_USER"))
    if not key or not host or not user:
        report.append({"route": "ssh_runtime", "error": "ssh_configuration_missing"})
        return False
    port = env.get("SSH_PORT") or "22"
    if (not port.isdigit() or not 0 < int(port) < 65536
            or not re.fullmatch(r"[A-Za-z0-9_.-]+", user)
            or not re.fullmatch(r"[A-Za-z0-9_.:\[\]-]+", host) or host.startswith("-")):
        raise DeliveryError("invalid_ssh_configuration")
    bootstrap = "import json,sys;p=json.load(sys.stdin);g={'__name__':'evidence_remote'};exec(compile(p['source'],'evidence_delivery.py','exec'),g);g['remote_main'](p)"
    packet = dict(source=Path(__file__).read_text(), payload=payload,
                  deploy_path=env.get("DEPLOY_PATH") or "/opt/arbitragex-v2",
                  compose_file=env.get("COMPOSE_FILE") or "docker/compose.prod.yml",
                  api_service=env.get("API_SERVICE") or "api-server")
    with tempfile.TemporaryDirectory(prefix="arbx-evidence-") as directory:
        key_file = Path(directory) / "key"
        key_file.write_text(key + "\n")
        key_file.chmod(0o600)
        try:
            output = command(["ssh", "-i", str(key_file), "-p", port,
                "-o", "BatchMode=yes", "-o", "IdentitiesOnly=yes", "-o", "StrictHostKeyChecking=accept-new",
                "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3",
                f"{user}@{host}", "python3 -c " + shlex.quote(bootstrap)], json.dumps(packet), timeout=240)
            result = json.loads(output)
            # Remote stdout is protocol-only. Never print docker inspect/env or
            # stderr, which can contain credentials or internal host details.
            report.extend(result["attempts"])
            return result.get("ok") is True
        except (DeliveryError, ValueError, KeyError, TypeError):
            report.append({"route": "ssh_runtime", "error": "ssh_delivery_failed"})
            return False


def main():
    report = []
    try:
        path = Path(os.environ["PAYLOAD_FILE"])
        if path.stat().st_size > MAX_BODY:
            raise DeliveryError("payload_too_large")
        payload = json.loads(path.read_text())
        if not isinstance(payload, dict) or any(not isinstance(payload.get(k), str) or not payload[k]
                for k in ("gate_id", "item_key", "status", "evidence_ref", "verified_by")):
            raise DeliveryError("invalid_payload")
        try:
            urls = configured_urls(os.environ.get("EVIDENCE_URL", ""), os.environ.get("EVIDENCE_URLS", ""))
        except DeliveryError as error:
            report.append({"route": "configured_endpoints", "error": str(error)})
            urls = []
        routes = []
        for i, url in enumerate(urls):
            try:
                routes.append(HttpTransport(f"configured_{i}", url, os.environ.get("ADMIN_TOKEN", "")))
            except DeliveryError as error:
                report.append({"route": f"configured_{i}", "error": str(error)})
        ok = deliver_routes(routes, payload, report) or ssh_delivery(payload, os.environ, report)
    except (DeliveryError, OSError, ValueError, KeyError, TypeError) as error:
        report.append({"route": "configuration", "error": str(error) if isinstance(error, DeliveryError) else "invalid_delivery_configuration"})
        ok = False
    print(json.dumps({"delivered": ok, "attempts": report}))
    if not ok:
        print("::error::Evidence not delivered; payload retained. See per-route error codes.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
