'use strict';
/**
 * arbx-socket-proxy — least-privilege Docker socket proxy.
 *
 * Only proxies 4 operations: list containers, inspect, start, stop.
 * EVERYTHING else (create / delete / exec / prune / images / networks / …)
 * is refused with 403 before ever touching the docker socket.
 *
 * Zero runtime dependencies (Node built-in `http` only) — supply-chain hardened.
 * Talks to /var/run/docker.sock internally; exposes a FILTERED HTTP API on the
 * arbx-net bridge only (no host port). Hardened in compose: read-only rootfs,
 * cap_drop ALL, no-new-privileges, nonroot (65532) + docker gid via group_add.
 *
 * The api-server is the only caller; it further restricts which service names
 * are controllable via an allowlist + regex, and audit-logs every action.
 */
const http = require('http');

const SOCKET_PATH = process.env.DOCKER_SOCKET || '/var/run/docker.sock';
const PORT = Number(process.env.LISTEN_PORT || 2375);
const HOST = process.env.LISTEN_HOST || '0.0.0.0';
const MAX_BODY = 64 * 1024; // 64 KiB — start/stop carry no meaningful body
const UPSTREAM_TIMEOUT_MS = 15_000;

// Least-privilege allowlist. The name segment is locked to [A-Za-z0-9_.-] so a
// caller cannot reach arbitrary docker API paths (no /, no .., no query injection).
// Version prefix (e.g. /v1.45/) optional, matches docker clients.
const NAME = '[A-Za-z0-9_.-]+';
const RULES = [
  { method: 'GET',  re: new RegExp(`^/(?:v[0-9.]+/)?containers/json(?:\\?[^#]*)?$`) },
  { method: 'GET',  re: new RegExp(`^/(?:v[0-9.]+/)?containers/${NAME}/json$`) },
  { method: 'POST', re: new RegExp(`^/(?:v[0-9.]+/)?containers/${NAME}/start$`) },
  { method: 'POST', re: new RegExp(`^/(?:v[0-9.]+/)?containers/${NAME}/stop(?:\\?t=\\d{1,4})?$`) },
];

const allowed = (method, url) =>
  RULES.some((r) => r.method === method && r.re.test(url));

function deny(res, code, msg) {
  if (res.headersSent) return;
  res.writeHead(code, { 'content-type': 'application/json' });
  res.end(JSON.stringify({ error: msg }));
}

const server = http.createServer((req, res) => {
  const { method, url } = req;

  // Filter FIRST. Unknown/forbidden paths never reach the socket.
  // (Docker's own /_ping, /version, /images/*, /exec/*, etc. all land here → 403.)
  if (!allowed(method, url)) {
    req.resume(); // drain to avoid socket hang
    return deny(res, 403, 'forbidden_by_socket_proxy');
  }

  // Defensive body cap (start/stop bodies are tiny/empty).
  let size = 0;
  req.on('data', (chunk) => {
    size += chunk.length;
    if (size > MAX_BODY) {
      req.destroy();
      return deny(res, 413, 'body_too_large');
    }
  });

  const upstream = http.request(
    {
      socketPath: SOCKET_PATH,
      path: url,
      method,
      headers: req.headers,
      timeout: UPSTREAM_TIMEOUT_MS,
    },
    (up) => {
      res.writeHead(up.statusCode || 502, up.headers);
      up.pipe(res);
    },
  );

  upstream.on('timeout', () => {
    upstream.destroy();
    deny(res, 504, 'upstream_timeout');
  });
  upstream.on('error', () => {
    deny(res, 502, 'upstream_error');
  });

  req.pipe(upstream);
});

server.listen(PORT, HOST, () => {
  // eslint-disable-next-line no-console
  console.log(
    JSON.stringify({
      msg: 'arbx-socket-proxy listening',
      port: PORT,
      host: HOST,
      socket: SOCKET_PATH,
      allowed: 'GET containers/json, GET containers/:id/json, POST containers/:id/start, POST containers/:id/stop',
    }),
  );
});

// Fail fast + loud on the socket disappearing (promtail-style honesty).
server.on('error', (err) => {
  // eslint-disable-next-line no-console
  console.error(JSON.stringify({ msg: 'socket_proxy_fatal', err: err.message }));
  process.exit(1);
});
