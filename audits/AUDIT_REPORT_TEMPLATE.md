# Auditoría VPS Wiring — C10 Fase 0 (TEMPLATE DE REFERENCIA)

> Este archivo es **documentación humana**: muestra el formato que el
> workflow `audit-vps-wiring.yml` genera dinámicamente. No lo edites
> esperando que el workflow lo lea — los reportes reales se crean con
> nombre `AUDIT-<short_sha>-<utc_timestamp>.{json,md}`.

**Timestamp:** `{{timestamp_utc}}` · **Run:** [#{{run_number}}]({{run_url}}) · **SHA:** `{{short_sha}}`

> Si no puedes medirlo, no está cableado. Si no puedes revertirlo, no está listo. Si no puedes trazar entrada y salida, no es HFT: es un dashboard.

## Tabla principal

| Capa | Estado | Evidencia | Riesgo | Acción recomendada |
|---|---|---|---|---|
| GitHub default branch | `{{estado_a}}` | `{{evidencia_a}}` | `{{riesgo_a}}` | `{{accion_a}}` |
| VPS HEAD | `{{estado_b_head}}` | `{{evidencia_b_head}}` | `{{riesgo_b_head}}` | `{{accion_b_head}}` |
| Remote origin VPS | `{{estado_b_remote}}` | `{{evidencia_b_remote}}` | `{{riesgo_b_remote}}` | `{{accion_b_remote}}` |
| Working tree | `{{estado_b_tree}}` | `{{evidencia_b_tree}}` | `{{riesgo_b_tree}}` | `{{accion_b_tree}}` |
| Stashes operador | `{{estado_b_stash}}` | `{{evidencia_b_stash}}` | `{{riesgo_b_stash}}` | `{{accion_b_stash}}` |
| Permisos .env | `{{estado_c_env}}` | `{{evidencia_c_env}}` | `{{riesgo_c_env}}` | `{{accion_c_env}}` |
| Docker containers | `{{estado_d_ps}}` | `{{evidencia_d_ps}}` | `{{riesgo_d_ps}}` | `{{accion_d_ps}}` |
| Edge /health | `{{estado_e_edge}}` | `{{evidencia_e_edge}}` | `{{riesgo_e_edge}}` | `{{accion_e_edge}}` |
| api-server /health | `{{estado_e_api}}` | `{{evidencia_e_api}}` | `{{riesgo_e_api}}` | `{{accion_e_api}}` |
| Frontend /api/health | `{{estado_e_front}}` | `{{evidencia_e_front}}` | `{{riesgo_e_front}}` | `{{accion_e_front}}` |
| Prometheus /-/healthy | `{{estado_e_prom}}` | `{{evidencia_e_prom}}` | `{{riesgo_e_prom}}` | `{{accion_e_prom}}` |
| Nginx routing | `{{estado_f_nginx}}` | `{{evidencia_f_nginx}}` | `{{riesgo_f_nginx}}` | `{{accion_f_nginx}}` |
| Backups Docker | `{{estado_d_backups}}` | `{{evidencia_d_backups}}` | `{{riesgo_d_backups}}` | `{{accion_d_backups}}` |

## Detalle por capa

### Capa A — GitHub

```json
{
  "default_branch": "main",
  "head_sha": "{{gh_head}}",
  "workflows": ["..."],
  "secrets": ["..."],
  "environments": ["..."],
  "branch_protection": { "required_status_checks": "...", "enforce_admins": "..." }
}
```

### Capa B — VPS Git metadata

```json
{
  "head": "{{vps_head}}",
  "branch": "main",
  "remote_url": "{{vps_remote_url}}",
  "remote_broken": false,
  "stashes": ["stash@{0}: ...", "stash@{1}: ..."],
  "workflows_present": ["audit-vps-wiring.yml", "..."]
}
```

### Capa C — VPS Filesystem

```json
{
  "env_perm": "600",
  "env_size_bytes": 0,
  "env_meta": "-rw------- root root <size>",
  "disk": "<filesystem> <size> <used> <avail> <use%> <mount>"
}
```

### Capa D — Docker

```json
{
  "containers": [
    { "name": "arbitragex-v2-edge-1", "status": "Up", "image": "arbitragex-v2-edge:latest", "ports": "..." }
  ],
  "images": [
    { "ref": "arbitragex-v2-edge:latest", "id": "...", "created": "...", "size": "..." }
  ],
  "backup_tags": ["arbitragex-v2-edge:pre-<sha>"]
}
```

### Capa E — Health endpoints

```json
{
  "edge": "200 0.012s",
  "api_server": "200 0.008s",
  "frontend": "GAP",
  "prometheus": "200 0.003s"
}
```

### Capa F — Routing

```json
{
  "config_summary": "<server_name|proxy_pass|location entries>",
  "gap": false
}
```

### Capa G — Análisis de riesgo

```json
{
  "remote_broken": false,
  "head_desync": false,
  "working_tree_dirty": false,
  "dirty_count": 0,
  "stashes_present": true,
  "stashes_count": 2,
  "env_perm_correct": true
}
```

## Veredicto Fase 0

- **READY para Fase 1:** `{{ready}}`
- **Bloqueos detectados:**

`{{lista_de_bloqueos}}`

- **Próxima acción:** Operador revisa este reporte y autoriza Fase 1 (blindaje metadata Git VPS).
