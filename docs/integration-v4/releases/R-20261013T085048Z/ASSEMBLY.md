# ASSEMBLY — mecanismo de ensamblado del release desde LÍNEA BASE LIMPIA (capitán α11, orden (3))

El release R-20261013T085048Z se ensambla NUNCA desde el árbol compartido sucio (4092 M intocables,
declarados desde t1) sino desde la línea base limpia `feat/exp-20261013 @ 21feefb32` (HEAD del
equipo, contiene A4 de t1 y el manifest de t2) + SOLO los archivos intencionales de esta operación.

## Comandos literales ejecutados (esta sesión, attempt 2)

```powershell
$wt = "$env:TEMP\arbx-t9-v3base"
git worktree add --detach $wt 21feefb32          # 10347 archivos, base limpia
# (build V3 corre en ese worktree con CARGO_TARGET_DIR compartido — ver V3_build/)
New-Item -ItemType Directory -Force -Path "$wt\docs\integration-v4\releases"
Copy-Item -Recurse -Force docs/integration-v4/releases/R-20261013T085048Z  $wt/docs/integration-v4/releases/
Copy-Item -Recurse -Force docs/integration-v4/t9-evidence                 $wt/docs/integration-v4/
git -C $wt status --porcelain                    # diff intencional EXACTO (abajo)
git worktree remove --force $wt                  # limpieza en la MISMA sesión (R11)
```

## Diferencia intencional resultante (salida literal `git status --porcelain` del worktree ensamblado)

```
 M backend/searcher-rs/src/detector_policy.rs     # regenerado (+134/-0, puramente aditivo)
?? docs/integration-v4/releases/                   # release R-20261013T085048Z completo
?? docs/integration-v4/t9-evidence/                # probes ssh R12 (exit=255 stderr 0B)
```

- tracked modificados: 1 (detector_policy.rs, diff completo en gen/detector_policy.rs.diff).
- tracked ELIMINADOS: 0. staged: 0. Cero bytes de los 4092 M del árbol compartido entran al release.
- Insumos de la regeneración verificados byte-idénticos entre árbol compartido y HEAD limpio:
  detector_policy.rs FED8CFCC…, hop_cycle_bridge.rs EE13950A… (medido al montar el worktree).

## Para t6 (auditoría no-pérdida)

`diff(release, HEAD 21feefb32)` = exactamente la lista de arriba. El árbol compartido sucio no
participa: cualquier coincidencia con los 4092 M sería contaminación a reportar, no parte del release.
