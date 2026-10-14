# T20 — α2 Build imagen arbx:R-20261013T1545Z — VERDE (attempt 2)

2026-10-13 · attempt 448f9884 · ruling colchón α11 aplicado (df-watchdog vivo, abort <5GB,
guard 1.5× supersede-por-medición SOLO build) · regla permanente de tracciones respetada
(nohup-VPS + polls foreground; JAMÁS job-background-ssh).

## Resultado

- **Imagen**: `arbx:R-20261013T1545Z` · 185MB (== estimación t19 exacta).
- **DIGEST** (`docker image inspect --format '{{.Id}}'`):
  `sha256:a9b3ef50a8b326cddcacb1d2b630e3968c57a936e7d7f5fbb254a537e6a773f1`
- **Build**: `docker build --no-cache -f backend/searcher-rs/Dockerfile -t arbx:R-20261013T1545Z
  backend/` desde /opt/arbitragex-v2 bajo `flock /tmp/arbx-deploy.lock`, nohup+poll
  (evidencia build_launch.txt / build_watchdog.txt / build_finish.txt).
- **Fuente**: release aplicado byte-exacto — `gen/detector_policy.rs` del release R-t6 (VPS,
  sha `5698976fd2573be7…` == estación == release) copiado al árbol VPS (`git status` remoto:
  ` M backend/searcher-rs/src/detector_policy.rs` — la ÚNICA mutación de árbol, sancionada por
  la topología de release). Cero ediciones al vuelo (I1/§I OK).

## df-watchdog (tres mediciones pegadas, ruling α11)

| momento | libres | nota |
|---|---|---|
| ANTES | 18G (88%) | 0 dumps concurrentes verificados (pgrep pg_dump = 0) |
| DURANTE (pico) | 13G (92%) | polls 1-9 cada ~105s; nunca <5GB; cero aborts |
| DESPUÉS | 14G (91%) | imagen 185MB residente |

## BOOT PROBADO (prerequisito t21)

- 3 corridas `docker run --rm` consistentes: tini + entrypoint + user `arbx` ejecutan.
- Fail-fast EXPLÍCITO y legible: `Error: config file not found: /app/configs/app.toml`
  (boot_probe.txt / boot_probe_v2.txt) — sin garbage, sin hang: el binario arranca, parsea
  config y falla honesto. `--help` carga config primero (mismo error — comportamiento uniforme).
- `HEALTHCHECK` de la imagen = null (declarado; el health real vive en compose runtime).
- **Por qué NO monté el config productivo**: el searcher con config completo + network default
  podría alcanzar redis/pg mapeados al host y actuar como SEGUNDO productor contra el stack
  vivo. El arranque con config del stack es el re-drill de t21 (stack efímero aislado) — no se
  simula aquí. P5 (¬tocar MEV detection/searcher P&L) intacto.

## Digest → A1 + re-hash en cadena + ℛ (local == VPS verificado)

- A1_MANIFEST.md: fila `A10-imagen` añadida a la tabla §11-bis con el digest → A1 re-hasheado:
  `7e1612f3…` → **`b1371b8ce1d861f8e24c9f9cac40e849acd02970a16bf1f91395ddd84d1dde6d`**.
- ℛ.tsv (release R-t6): fila A1 actualizada (re-hash en cadena) + fila imagen digest añadida
  (verificador t21/t7) → **67 filas**, sha `29a8bbddb6fce78eddb3e3e34f17f87ce12e472ed2504c343fd12c5d79233f91`.
- Sincronización al gemelo VPS: primer intento por pipe textual CORROMPIÓ bytes (shas
  divergieron; el carácter ℛ U+211B no sobrevive pwsh→ssh texto) — corregido con tar Python +
  upload por redirect binario cmd `< archivo` + mv atómico remoto. Verificación final:
  A1 VPS `b1371b8c…` == local ✓; ℛ.tsv VPS `29a8bbdd…` == local ✓; /tmp/sync-t20 limpiado.

## t21 queda habilitado

Unidad de release lista para el re-drill: imagen real (no retag-espejo), digest en A1, rollback
= tag previo + up -d (mecánica ya medida 10.66s por t18; re-drill con imagen REAL pendiente en t21).
