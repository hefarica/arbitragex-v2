# V4 — dry-run rsync (DEGRADADO, staging local verificado; destino VPS solo-lectura)

Estado: **DEGRADADO** — `rsync` no existe en este host (Test-Path `C:\Program Files\Git\usr\bin\rsync.exe` = False);
el dry-run REAL contra el VPS no pudo ejecutarse. Se documenta el comando exacto que SE EJECUTARÍA y el
diff exacto ya verificado LOCALMENTE (staging vs árbol del repo).

## Instrumento ssh (evidencia R12 dual-binario, medido esta sesión)

- `ssh` NATIVO (`ssh -o BatchMode=yes -o ConnectTimeout=10 arbx "echo ok"`) → **exit=255, stderr 0 bytes**
  (evidencia: `docs/integration-v4/t9-evidence/ssh_probe_20261013T033640Z.txt`, `ssh_stderr_probe.txt` 0B).
- `ssh` de Git (`C:\Program Files\Git\usr\bin\ssh.exe` con HOME=%USERPROFILE%) → **exit=0, PROBE_OK**
  (medido esta sesión; salida incluida abajo).
- Lectura del destino (solo-lectura, cero escritura):
  ```
  PROBE_OK
  DEST_ABSENT            # /opt/arbitragex-v2/docs/integration-v2 NO existe
  00_HFT_SYSTEM_ARCHITECTURE.md
  00_MANIFIESTO_DE_EFICIENCIA_ESTRUCTURAL.md
  01-baseline-expanded.png
  ...                    # /opt/arbitragex-v2 existe y contiene el repo deploy:deploy (alpha1 MAP-OK)
  ```

## Comando rsync que SE EJECUTARÍA (no ejecutado — binario ausente)

```bash
rsync -avn --itemize-changes --checksum   -e "C:/Program Files/Git/usr/bin/ssh.exe"   "docs/integration-v4/releases/R-20261013T085048Z/"   "arbx:/opt/arbitragex-v2/docs/integration-v4/releases/R-20261013T085048Z/"
```
(`-n` = dry-run: nunca escribe; el destino correcto es bajo `/opt/arbitragex-v2` según α1 —
no existe `/opt/arbx` ni releases/current; despliegue = git+rebuild, 25 contenedores compose.prod.yml.)

## Diff exacto ya verificado LOCALMENTE (equivalente del dry-run)

El contenido de `R-20261013T085048Z/` es 100% NUEVO respecto del árbol local y del destino VPS
(ambos en base 5f8ea3d67, drift=0): cada archivo del release sería una CREACIÓN (`>f+++++++++`)
bajo `docs/integration-v4/releases/R-20261013T085048Z/`. El único archivo que el release MODIFICA
de un árbol existente — `backend/searcher-rs/src/detector_policy.rs` (+134/−0, puramente aditivo;
`gen/detector_policy.rs.diff`, sha256 92e57583…) — NO se propaga en V4: queda en staging hasta que
t6/t14 gobierne la promoción (coexistencia; nada se borra; el wire sigue sirviendo QUOTEBASE).

## Declaración fail-honest

V4 no pudo ejecutarse contra el destino real por ausencia del binario rsync en este host.
El plan está verificado localmente: estructura del release íntegra (V1 hashes), diff aditivo +134/−0
contra HEAD 21feefb32, y destino VPS confirmado por lectura ssh (Git-bash) — `/opt/arbitragex-v2`
existe, `docs/integration-v4/` AUSENTE en destino ⇒ creación limpia, sin colisión.
