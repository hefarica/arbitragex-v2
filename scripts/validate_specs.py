#!/usr/bin/env python3
"""FASE 4 Inc 2 — deterministic structural validation of apis/*.yaml.

Avoids the @asyncapi/cli (environment-flaky here) by checking document integrity
directly: YAML well-formedness, required top-level fields, channel/operation
shape, $ref resolution, and the corrected admin-auth header. OpenAPI 3.1 deep
schema validation is handled separately by redocly in the same workflow.
"""
import os
import sys

import yaml

errs = []


def err(msg):
    errs.append(msg)


def resolve_ref(root, ref):
    if not isinstance(ref, str) or not ref.startswith("#/"):
        return None
    cur = root
    for part in ref[2:].split("/"):
        if not isinstance(cur, dict) or part not in cur:
            return None
        cur = cur[part]
    return cur


base = os.path.join(os.getcwd(), "apis")

# ---- OpenAPI ----
try:
    o = yaml.safe_load(open(os.path.join(base, "openapi.yaml")))
except Exception as e:  # noqa: BLE001
    err(f"openapi.yaml is not valid YAML: {e}")
    o = None
if isinstance(o, dict):
    if not str(o.get("openapi", "")).startswith("3."):
        err("openapi.yaml: `openapi` must be 3.x")
    if not (o.get("info") or {}).get("title"):
        err("openapi.yaml: info.title is required")
    if not isinstance(o.get("paths"), dict) or not o["paths"]:
        err("openapi.yaml: non-empty `paths` is required")
    admin = ((o.get("components") or {}).get("securitySchemes") or {}).get("AdminToken") or {}
    if admin.get("name") != "x-arbx-admin-token":
        err(f"openapi.yaml: AdminToken header must be x-arbx-admin-token (got {admin.get('name')!r})")
elif o is not None:
    err("openapi.yaml did not parse to a mapping")

# ---- AsyncAPI ----
try:
    a = yaml.safe_load(open(os.path.join(base, "asyncapi.yaml")))
except Exception as e:  # noqa: BLE001
    err(f"asyncapi.yaml is not valid YAML: {e}")
    a = None
if isinstance(a, dict):
    if not str(a.get("asyncapi", "")).startswith("2."):
        err("asyncapi.yaml: `asyncapi` must be 2.x")
    if not (a.get("info") or {}).get("title"):
        err("asyncapi.yaml: info.title is required")
    chans = a.get("channels")
    if not isinstance(chans, dict) or not chans:
        err("asyncapi.yaml: non-empty `channels` is required")
    else:
        for name, ch in chans.items():
            if not isinstance(ch, dict):
                err(f"asyncapi.yaml: channel {name} must be a mapping")
                continue
            ops = [k for k in ("publish", "subscribe") if k in ch]
            if not ops:
                err(f"asyncapi.yaml: channel {name} needs publish or subscribe")
            for op in ops:
                node = ch[op] if isinstance(ch[op], dict) else {}
                msg = node.get("message") if isinstance(node.get("message"), dict) else {}
                ref = msg.get("$ref")
                if ref and resolve_ref(a, ref) is None:
                    err(f"asyncapi.yaml: {name}.{op} message $ref does not resolve: {ref}")
    schemes = (a.get("components") or {}).get("securitySchemes") or {}
    for sname, sv in (a.get("servers") or {}).items():
        if not sv.get("protocol"):
            err(f"asyncapi.yaml: server {sname} is missing `protocol`")
        for sec in sv.get("security") or []:
            for k in sec or {}:
                if k not in schemes:
                    err(f"asyncapi.yaml: server {sname} references missing securityScheme {k}")
elif a is not None:
    err("asyncapi.yaml did not parse to a mapping")

if errs:
    print("Spec structural validation FAILED:", file=sys.stderr)
    for e in errs:
        print("  - " + e, file=sys.stderr)
    sys.exit(1)
print("Spec structural validation passed (openapi 3.x + asyncapi 2.x, refs resolve, auth header correct).")
