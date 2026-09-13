#!/usr/bin/env python3
"""Validate a LOCAL VPS mapping manifest and sanitized evidence files.

This utility never connects to SSH/MCP, runs shell commands or writes files.
A passing result checks declared structure and evidence integrity, NOT authenticity,
true completeness, operating-system permissions, VPS health or live authorization.
Python 3.10+; standard library only.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import sys
from typing import Any

REQUIRED_CHECKS: dict[str, tuple[str, ...]] = {
    "identidad_acceso": ("host_y_huella", "usuario_y_alcance", "ruta_y_frontera"),
    "recursos": ("cpu_memoria_swap", "carga_y_procesos", "limites_y_oom"),
    "almacenamiento": ("filesystems_espacio_inodos", "volumenes_y_mounts", "crecimiento_y_picos"),
    "servicios": ("sistema_y_contenedores", "salud_y_reinicios", "dependencias_y_procesos"),
    "datos": ("catalogos_y_tamanos", "persistencia_y_durabilidad", "locks_slots_y_archivado"),
    "retencion_backups": ("jobs_y_politicas", "historial_y_crecimiento", "copias_y_restauracion"),
    "red_seguridad": ("listeners_y_firewall", "rutas_tls_y_tuneles", "acceso_y_exposicion"),
    "despliegue": ("repo_y_cambios", "imagenes_y_revision", "ci_locks_y_watchdogs"),
    "configuracion": ("nombres_y_requisitos", "origen_y_precedencia", "permisos_y_referencias"),
    "observabilidad": ("metricas_y_logs", "alertas_y_silencios", "salud_extremo_a_extremo"),
    "blockchain": ("redes_y_rpc", "dex_y_pools", "firmantes_y_limites"),
    "dependencias": ("grafo_y_propietarios", "activos_huerfanos", "conciliacion_y_paginacion"),
}
MAX_AGE = timedelta(hours=1)  # Initial package policy, not an infrastructure guarantee.
MAX_EVIDENCE_BYTES = 8 * 1024 * 1024
MAX_MANIFEST_BYTES = 2 * 1024 * 1024
ID_RE = re.compile(r"[A-Za-z0-9_.-]{1,96}\Z")
HASH_RE = re.compile(r"[0-9a-f]{64}\Z")
NOTICE = ("Valida estructura e integridad declaradas; NO certifica exhaustividad, "
          "hechos, salud ni permisos; NO autoriza escrituras ni trading.")


def has_link(path: Path) -> bool:
    """Reject symlinks and Windows reparse points, including junctions."""
    try:
        s = path.lstat()
    except FileNotFoundError:
        return False
    return stat.S_ISLNK(s.st_mode) or bool(
        getattr(s, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 1024)
    )


def safe_evidence_path(root: Path, value: Any) -> Path:
    if not isinstance(value, str) or not value or "\\" in value or ":" in value:
        raise ValueError("ruta de evidencia relativa no válida")
    relative = PurePosixPath(value)
    if relative.is_absolute() or ".." in relative.parts or not relative.parts:
        raise ValueError("ruta de evidencia fuera de alcance")
    root = root.absolute()
    for parent in (*reversed(root.parents), root):
        if has_link(parent):
            raise ValueError("raíz de evidencia contiene enlace/reparse point")
    result = root
    for part in relative.parts:
        result /= part
        if has_link(result):
            raise ValueError("evidencia contiene enlace/reparse point")
    resolved = result.resolve(strict=True)
    if not resolved.is_relative_to(root.resolve(strict=True)) or not resolved.is_file():
        raise ValueError("evidencia no es un archivo regular local dentro del alcance")
    return resolved


def parse_utc(value: Any) -> datetime:
    if not isinstance(value, str):
        raise ValueError("fecha ausente")
    stamp = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if stamp.tzinfo is None or stamp.utcoffset() != timedelta(0):
        raise ValueError("se requiere fecha UTC con zona explícita")
    return stamp


def validate(doc: Any, evidence_root: Path, *, now: datetime | None = None) -> list[str]:
    """Return all detected errors. Does not prove the mapper told the truth."""
    errors: list[str] = []
    now = now or datetime.now(timezone.utc)
    if now.tzinfo is None:
        raise ValueError("now must have a timezone")
    if not isinstance(doc, dict):
        return ["el manifiesto debe ser un objeto JSON"]
    if type(doc.get("schema_version")) is not int or doc.get("schema_version") != 1:
        errors.append("schema_version debe ser 1")
    if not isinstance(doc.get("snapshot_id"), str) or not ID_RE.fullmatch(doc["snapshot_id"]):
        errors.append("snapshot_id no válido")

    def stamp_check(value: Any, location: str) -> None:
        try:
            stamp = parse_utc(value)
            if stamp > now:
                errors.append(f"{location}: fecha futura")
            elif now - stamp > MAX_AGE:
                errors.append(f"{location}: evidencia más antigua que una hora")
        except (ValueError, OverflowError):
            errors.append(f"{location}: fecha UTC no válida")

    stamp_check(doc.get("captured_at_utc"), "snapshot")
    target = doc.get("target")
    if not isinstance(target, dict):
        target = {}
        errors.append("target debe ser objeto")
    for field in ("ssh_alias", "identity", "project_path"):
        if not isinstance(target.get(field), str) or not target[field].strip():
            errors.append(f"target.{field}: requerido")
    identity = target.get("identity")

    def indexed(value: Any, location: str) -> dict[str, dict[str, Any]]:
        out: dict[str, dict[str, Any]] = {}
        if not isinstance(value, list):
            errors.append(f"{location}: debe ser lista")
            return out
        for row in value:
            if not isinstance(row, dict) or not isinstance(row.get("id"), str) or not ID_RE.fullmatch(row["id"]):
                errors.append(f"{location}: entrada/id no válido")
                continue
            if row["id"] in out:
                errors.append(f"{location}: id duplicado {row['id']}")
            out[row["id"]] = row
        return out

    evidence = indexed(doc.get("evidence"), "evidence")
    if not evidence:
        errors.append("faltan archivos de evidencia")
    for eid, item in evidence.items():
        if item.get("target_identity") != identity:
            errors.append(f"{eid}: identidad de destino distinta")
        stamp_check(item.get("captured_at_utc"), eid)
        digest = item.get("sha256")
        if not isinstance(digest, str) or not HASH_RE.fullmatch(digest):
            errors.append(f"{eid}: sha256 no válido")
            continue
        try:
            path = safe_evidence_path(evidence_root, item.get("path"))
            with path.open("rb") as stream:
                contents = stream.read(MAX_EVIDENCE_BYTES + 1)
            if len(contents) > MAX_EVIDENCE_BYTES:
                raise ValueError("archivo de evidencia supera 8 MiB")
            if hashlib.sha256(contents).hexdigest() != digest:
                errors.append(f"{eid}: hash no coincide")
        except (OSError, ValueError) as exc:
            errors.append(f"{eid}: archivo de evidencia inválido ({type(exc).__name__})")

    def refs_check(value: Any, location: str) -> None:
        if not isinstance(value, list) or not value:
            errors.append(f"{location}: necesita referencias de evidencia")
            return
        seen: set[str] = set()
        for ref in value:
            if not isinstance(ref, str) or ref not in evidence:
                errors.append(f"{location}: referencia inexistente/no válida")
            elif ref in seen:
                errors.append(f"{location}: referencia repetida")
            else:
                seen.add(ref)

    refs_check([target.get("identity_evidence_id")], "target.identity")
    domains = indexed(doc.get("domains"), "domains")
    if set(domains) != set(REQUIRED_CHECKS):
        errors.append("deben figurar exactamente los 12 dominios definidos")
    assets = indexed(doc.get("assets"), "assets")
    for aid, asset in assets.items():
        if not isinstance(asset.get("domain_id"), str) or asset["domain_id"] not in REQUIRED_CHECKS:
            errors.append(f"{aid}: dominio no válido")
        if asset.get("verified") is not True:
            errors.append(f"{aid}: activo no verificado")
        if not isinstance(asset.get("label"), str) or not asset["label"].strip():
            errors.append(f"{aid}: descripción requerida")
        refs_check(asset.get("evidence_ids"), aid)
        dependencies = asset.get("depends_on")
        if not isinstance(dependencies, list):
            errors.append(f"{aid}: depends_on debe ser lista")
        else:
            seen_deps: set[str] = set()
            for dep in dependencies:
                if not isinstance(dep, str) or dep not in assets or dep == aid:
                    errors.append(f"{aid}: dependencia inexistente/no válida/autorreferente")
                elif dep in seen_deps:
                    errors.append(f"{aid}: dependencia repetida")
                else:
                    seen_deps.add(dep)

    for did, required in REQUIRED_CHECKS.items():
        domain = domains.get(did)
        if domain is None:
            continue
        if domain.get("enumeration_complete") is not True:
            errors.append(f"{did}: enumeración incompleta")
        refs_check(domain.get("enumeration_evidence_ids"), did + ".enumeration")
        owned = {aid for aid, a in assets.items() if a.get("domain_id") == did}
        listed = domain.get("asset_ids")
        if (not isinstance(listed, list) or not all(isinstance(a, str) for a in listed)
                or len(set(listed)) != len(listed) or set(listed) != owned):
            errors.append(f"{did}: inventario no conciliado")
        reason = domain.get("absence_reason")
        if not owned and (not isinstance(reason, str) or not reason.strip()):
            errors.append(f"{did}: ausencia de activos sin justificación")
        if owned and reason not in (None, ""):
            errors.append(f"{did}: ausencia declarada con activos presentes")
        checks = indexed(domain.get("checks"), did + ".checks")
        if not set(required).issubset(checks):
            errors.append(f"{did}: faltan comprobaciones obligatorias")
        for cid, check in checks.items():
            location = did + "." + cid
            if check.get("status") != "verified":
                errors.append(f"{location}: no verificado")
            if not isinstance(check.get("finding"), str) or not check["finding"].strip():
                errors.append(f"{location}: hallazgo/resultado requerido")
            refs_check(check.get("evidence_ids"), location)

    if not isinstance(doc.get("open_gaps"), list) or doc["open_gaps"]:
        errors.append("hay huecos abiertos o open_gaps no es una lista")
    attestation = doc.get("mapper_attestation")
    if not isinstance(attestation, dict):
        attestation = {}
    for key in ("inventory_sources_reconciled", "secrets_excluded", "no_intentional_remote_mutations"):
        if attestation.get(key) is not True:
            errors.append(f"mapper_attestation.{key}: falta declaración del responsable")
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--evidence-root", type=Path)
    args = parser.parse_args(argv)
    try:
        with args.manifest.open("rb") as f:
            raw = f.read(MAX_MANIFEST_BYTES + 1)
        if len(raw) > MAX_MANIFEST_BYTES:
            raise ValueError("manifiesto supera 2 MiB")
        doc = json.loads(raw)
        errors = validate(doc, args.evidence_root or args.manifest.parent)
    except (OSError, ValueError) as exc:
        print(f"INVALID_INPUT: {type(exc).__name__}", file=sys.stderr)
        print(NOTICE, file=sys.stderr)
        return 2
    print("MAPA_FORMAL_INCOMPLETO" if errors else "MAPA_FORMAL_COMPLETO")
    for error in errors:
        print("- " + error)
    print(NOTICE)
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
