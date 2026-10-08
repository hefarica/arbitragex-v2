# SEC-CUSTODY-IDENTITY-01 — Threat model, matriz de custodia/identidad y SBOM evaluado

- **Rol:** Security (identidad, custodia, secretos, límites de confianza).
- **Base medida:** `bceb31ef7c0fd6d4b50bcdee6fc8d33858334822` — merge PR **#838**, `2026-10-06 08:52:41 -0500`; es el `origin/main` vigente al momento de medir (`git rev-parse origin/main` → mismo SHA).
- **Nota de árbol:** el worktree de esta sesión está en `fix/perhop-reserves-01` `858b943f` con 962 rutas rastreadas modificadas. **Toda cita de código de este documento es `rev:path` con blob id verificado**, no bytes del worktree sucio (ver §9.1).
- **Modo:** READ-ONLY sobre el repositorio; en el VPS solo consultas de lectura aguas arriba (Redis `SCAN`/`STRLEN`/`EXISTS`/`TYPE`/`CONFIG GET`/`ACL LIST`, `docker inspect/ps`, `ss`, `grep` de conteo, `curl` negativo). **Ningún valor de secreto se leyó, imprimió ni transitó por este documento.**
- **Relación con otros artefactos de `docs/security/`:** este documento NO repite `TOKEN-EXPOSURE-ACTION-SHEET.md` (incidente SEC-TOKEN-ROTATION-01, `ANTHROPIC_AUTH_TOKEN`) ni las secciones de contratos de `SECURITY-CUSTODY-HANDOFF.md` (S1-S5, custodia on-chain). Es complementario: capa de **credenciales de servicio, sesión admin, autoridad pre-broadcast y supply chain**.

---

## 0. Veredicto acotado

| ID | Estado medido | Prio | Una línea |
|---|---|---|---|
| **F30** | 🔴 **CONFIRMADO — y agravado**: no son "secrets raw en Redis", es **material de clave privada en claro en dos stores** y AOF en disco sin cifrar | **P0** | §4.1 |
| **F31** | 🔴 **CONFIRMADO end-to-end**: el control de la API **sí funciona** (401 anónimo) y **la página SSR lo saltea** (200 anónimo con snapshot embebido) | **P1** | §4.2 |
| **F33** | 🔴 **CONFIRMADO**: entre la última lectura de autoridad y `broadcast()` hay firma + 2 RTT de RPC; sin relectura | **P0** | §4.3 |
| **F34** | 🔴 **CONFIRMADO**: el cookie de sesión **es una copia del token maestro**; el TTL de 8 h **solo lo lee el navegador**; logout = borrar cookie | **P1** | §4.4 |
| **F13** | 🔴 **CONFIRMADO (con corrección de alcance)**: el WS acepta el token maestro por `auth`/query/header y **nunca** por cookie ⇒ la sesión da admin en HTTP y no en WS | P1 | §4.5 |
| **F32** | 🟡 **PARCIAL — con hallazgo nuevo medido**: kill-switch fail-closed ✅, paper-mode fail-closed ✅, **breaker de integración sin productor** ❌ | **P0** | §4.6 |
| **F35** | 🟡 **PARCIAL**: SBOM/cosign existen pero son **fail-open y no correlacionan con el artefacto desplegado**; `npm audit` del lock raíz = **4 critical / 19 high / 32 moderate** | P1 | §4.7 |
| **F36** | ⚪ **NO COMPUTADO** (`reason=rotation_artifact_has_no_producer_in_repo`): el repo no puede producir el artefacto de cierre; el productor es el operador | P1 | §4.8 |
| **SEC-N1** | 🆕 🔴 **NUEVO**: check 12 de la checklist pre-ejecución es un gate **que no puede dispararse** (clave sin productor) | **P0** | §4.6.1 |
| **SEC-N2** | 🆕 🟡 **NUEVO**: 207 MB de **AOF huérfanos** con el mismo material, sin dueño ni ciclo de vida | P1 | §4.9 |
| **SEC-N3** | 🆕 🟡 **NUEVO — latente y medido como tal**: el contrato WS admite el token maestro como **query param**; 0 de 8262 requests lo usaron, pero nginx persiste la query en disco | P2 | §4.10 |
| **SEC-P1** | ✅ **POSITIVO verificado**: el probe de autenticación de `POST /admin/session` **no tiene efecto de estado** | — | §5.1 |
| **SEC-P2** | ✅ **POSITIVO verificado**: el control de admin de la API rechaza anónimo, cookie forjada y centinela forjado | — | §5.2 |
| **SEC-P3** | ✅ **POSITIVO verificado**: ausencia de `arbx:papermode:*` ⇒ paper ON (broadcast suprimido), no live | — | §5.3 |

**Dictamen de mi superficie: NO-GO para capital real se sostiene por sí solo en §4.1 — y ese bloqueador no es el que el informe principal tenía en primer lugar.**

---

## 1. Activos, actores y límites de confianza

### 1.1 Activos (por consecuencia, no por nombre)

| Activo | Dónde vive (medido) | Por qué importa |
|---|---|---|
| `flashbots_signer` | `service_credentials.secret_value` (**66 chars**, `secret_ciphertext NULL`) + `arbx:svc_cred:flashbots_signer:global` (**STRLEN 224**) | Por contrato es una **clave privada de 32 bytes** (`shared-ts/src/contracts/credentials.ts:20`; validador `/^0x[0-9a-fA-F]{64}$/` en `validators.ts:222`). Quien la tenga puede firmar como ese firmante. |
| `admin_token` / `edge_token` | PG + Redis (`STRLEN 213` / `212`) | Capacidad de administración total sobre las rutas `/admin/*` y `/api/v1/credentials`. |
| `github_token`, `alchemy_prices`, `rpc_http:chain:1`, `rpc_ws:chain:1` | PG + Redis | Movimiento lateral: repositorio, proveedor de precios, endpoints RPC. |
| Mastership de la sesión admin | Cookie `arbx_admin_session` | **Es el token** (no una referencia), ver §4.4. |
| Estado de seguridad (`arbx:killswitch`) | Redis (`EXISTS=1`, `TYPE=string`) | Único estado cuyo borrado/alteración decide si el sistema firma o no. |

### 1.2 Superficies y límites de confianza (lo que cruza qué)

```
navegador (no confiable)
  → nginx host :80  [server_name <VPS_IP>, proxy_pass http://127.0.0.1:5173]   (medido)
    → frontend Next (SSR)  — POSEE process.env.ARBX_ADMIN_TOKEN               (blob bd6dd413…)
       → edge worker/dev-local — resuelve admin token: header O cookie de sesión (blob …)
          → api-server :8080 — requireAdminToken(ARBX_ADMIN_TOKEN)            (medido: 401 anónimo)
  → Redis :6379  [127.0.0.1 publicado, SIN auth, ACL ~* &* +@all]             (medido)
     ↔ 24 contenedores en arbx-net (searcher-rs, sim-ctl, anvil, math-engine,
        token-enricher, recon, selector-api, grafana, loki, minio, vault, …)  (medido)
  → PostgreSQL :5432 [secret_value en claro, disco ext4 SIN LUKS]              (medido)
```

**Frontera rota #1 (custodia de secretos):** el plano de contenedores no es una frontera de confianza para las credenciales — es un **colectivo de lectores sin autenticación**. Redis no tiene contraseña, acepta cualquier clave y cualquier canal, y está en la misma red bridge que 24 servicios.

**Frontera rota #2 (identidad):** `frontend` es un componente de confianza con capacidades de administrador y **su render es alcanzable sin autenticación**.

**Frontera rota #3 (autoridad):** no existe frontera entre "el sistema decidió ejecutar" y "el sistema firmó", salvo gates que se leyeron antes de firmar.

### 1.3 Actores

| Actor | Capacidad real (medida) |
|---|---|
| Internet anónimo | Obtiene el inventario de credenciales + máscaras + errores de validación por `GET /settings/credentials` (§4.2). No obtiene el secreto por esa vía. |
| Contenedor comprometido (cualquiera de los 24) | **Lee las 7 credenciales completas** y se suscribe a `arbx:svc_cred:reload` para capturar futuras (§4.1). |
| Cualquier proceso local del VPS | Idem: `127.0.0.1:6379` sin auth. |
| Portador de un cookie de sesión capturado | Administración completa **hasta que se rote el token maestro** — el logout no lo revoca (§4.4). |
| Operador | Puede rotar y aislar; ninguna de estas reparaciones es de código solo (§8). |

---

## 2. Matriz de scopes / custodia

| Portador | Scope efectivo | Cómo lo prueba |
|---|---|---|
| `ARBX_ADMIN_TOKEN` (proceso frontend) | `/admin/*` completo — el SSR lo usa para leer credenciales sin mirar al visitante | `frontend/app/settings/credentials/page.tsx:41-58` (blob `bd6dd413…`) |
| Cookie `arbx_admin_session` | **El mismo scope que el token**, porque su valor ES el token | `edge/worker/src/index.ts:961-967` |
| Visitante anónimo | Ninguno en la API (401), **pero recibe el snapshot SSR** | §5.2 + §4.2 |
| `ARBX_EDGE_TOKEN` | Puerta del edge hacia api-server | `adminProxy` `edge/worker/src/index.ts:262-285` |
| Cualquier contenedor en `arbx-net` | Lectura total de secretos + pub/sub total | `ACL LIST` en vivo (§4.1) |
| `relays-client` (terminus de capital) | Firma/broadcast; su clave viene de `Signer::from_env` — **no** del store de credenciales (medido) | `main.rs:190` |
| Rol `EXECUTOR_ROLE` on-chain | Dispara rutas; no retira (ya cubierto en `SECURITY-CUSTODY-HANDOFF.md` S1-S4) | — |

**Lectura de custodia:** hoy hay **tres** custodios efectivos del mismo material (PG, Redis, y —si la clave firmante del env coincide con la fila— el proceso), con independencia **cero** entre ellos. La separación signer/scopes que mi perfil exige no existe en esta capa.

---

## 3. Modelo de amenaza (derivado de los activos reales)

| # | Amenaza | Precondición | Control existente | Estado |
|---|---|---|---|---|
| T1 | Exfiltración de las 7 credenciales por un servicio lateral | Ejecutar cualquier cosa en un contenedor de `arbx-net`, o un proceso local | Ninguno (sin auth, sin ACL, sin TLS interno) | **Abierto** |
| T2 | Recolección pasiva de secretos futuros | `SUBSCRIBE arbx:svc_cred:reload` | Ninguno | **Abierto** |
| T3 | Recuperación de material desde el disco del VPS o de una imagen/snapshot | Acceso a disco, backup o snapshot del VPS | ext4 sin LUKS; AOF en claro; `.bak` huérfanos | **Abierto** |
| T4 | Replay de un cookie de sesión capturado | Captura del valor (log de proxy, backup de perfil, extensión, MITM fuera de TLS) | httpOnly + SameSite=Strict + `secure` (si HTTPS) — **ninguno de los tres detiene un replay del valor** | **Abierto** |
| T5 | Render anónimo del inventario de credenciales | `GET /settings/credentials` | Ninguno | **Abierto** |
| T6 | Ejecución durante una ventana en la que el operador ya ordenó parar | Flip de kill-switch entre la checklist y `broadcast()` | kill-switch fail-closed **una sola vez, antes de firmar** | **Abierto** |
| T7 | "Breaker" que no detiene nada | Confiar en el check 12 de la checklist | Ninguno: la clave no tiene productor | **Abierto** |
| T8 | Inyección de instrucciones vía contenido no confiable | Que un documento/log/resultado intente redefinir permisos | No medido en esta pasada | **No verificado** |
| T9 | Prompt/tool content exfiltra la credencial del runtime del agente | Acceso del agente a Redis/PG | Ninguno técnico (mi propio probe lo demuestra) | **Abierto** (higiene de proceso) |

**Explícitamente fuera de este documento:** prompt-injection en el arnés de agentes (T8) y custodia on-chain (ya cubierta por Contracts + `SECURITY-CUSTODY-HANDOFF.md`). No los re-derivo.

---

## 4. Hallazgos con evidencia

### 4.1 F30 — CONFIRMADO y AGRAVADO: clave privada en claro en PG y Redis; AOF sin cifrar

**Cadena de evidencia (toda verificable; ninguna imprime un secreto).**

1. **El código proyecta el secreto raw a propósito** — `backend/api-server/src/credentials/projection.ts` (blob `31e679fe7e3eb7ca1a75b5e916823a8efc983300`):
   - `:10-13` docstring: *"the projected value carries the RAW secret by design"*.
   - `:37-45` `ProjectedCredential.secret_value: string` (el campo es el secreto).
   - `:61-64` `const json = JSON.stringify(row); redis.set(svcCredKey(...), json); redis.publish(SVC_CRED_CHANNEL, json)`.
   - `:146-158` rehidratación en boot: `secret = await decryptSecret(...)` — es decir, **descifra antes de proyectar**.
2. **PG no tiene sobre cifrado en ninguna fila** (query de longitudes, `sql_query` sobre la DB viva):
   `secret_ciphertext IS NULL` y `secret_key_version = 0` en **7/7** filas; `secret_value` presente en las 7 (longitudes 66/60/40/21/60/401/137).
   El mecanismo existe (`credentials/crypto.ts` blob `48d3f4c6…`, migración 120) pero **no está aplicado a ningún dato**.
3. **`flashbots_signer` es, por contrato, una clave privada**: `shared-ts/src/contracts/credentials.ts:20` → *"0x-prefixed private key (32 bytes); test derives address"*; `validators.ts:222` → `if (!/^0x[0-9a-fA-F]{64}$/.test(clean)) return fail(...)`. La fila viva mide **66 chars** = `0x` + 64 hex. *(Inferencia de forma, declarada como tal: no leí el contenido.)*
4. **Redis en vivo** (`docker exec arbx…redis-1 redis-cli`, solo lectura):
   - `CONFIG GET appendonly` → `yes`; `appendfsync` → `everysec`.
   - `CONFIG GET requirepass` → **longitud 0** (sin contraseña).
   - `ACL LIST` → `user default on nopass sanitize-payload ~* &* +@all` → **todas las claves, todos los canales, todos los comandos, sin credencial**.
   - 7 claves `arbx:svc_cred:*`, todas con `TTL=-1` (sin expiración) y `OBJECT ENCODING=raw`.
   - `STRLEN`: `flashbots_signer 224`, `admin_token 213`, `github_token 194`, `alchemy_prices 177`, `edge_token 212`, `rpc_ws 561`, `rpc_http 1070`.
5. **Discriminante plaintext-vs-sobre por longitudes** (no por contenido). Para cada fila se predice la longitud del JSON de `projection.ts:159-167` con el secreto en forma **plaintext** y en forma **sobre** (salt+iv+tag+base64), usando solo longitudes conocidas de PG:

   | clave | `STRLEN` medido | forma plaintext predicha | Δ | forma sobre (estimada) | Δ |
   |---|---:|---:|---:|---:|---:|
   | `flashbots_signer:global` | 224 | 223 | **+1** | 279 | −55 |
   | `admin_token:global` | 213 | 212 | **+1** | 271 | −58 |
   | `github_token:global` | 194 | 193 | **+1** | 247 | −53 |
   | `alchemy_prices:global` | 177 | 176 | **+1** | 219 | −42 |
   | `edge_token:global` | 212 | 211 | **+1** | 271 | −59 |
   | `rpc_http:chain:1` | 1070 | 1088 | −18 | 1303 | −233 |
   | `rpc_ws:chain:1` | 561 | 545 | +16 | 672 | −111 |

   5/7 coinciden **exactamente** con la forma plaintext (±1 byte por formato de `updated_at`/`updated_by`). Las 2 restantes tienen `metadata` no vacío (578 y 299 chars en forma jsonb **con espacios**), lo que explica el residuo: `JSON.stringify` elimina esos espacios. **Ninguna fila es compatible con la forma sobre cifrado** (todas quedan a 42-59 bytes de distancia). Conclusión: el valor en Redis es el **secreto en claro**, no el sobre.
6. **Persistencia en disco (parcialmente probada, frontera declarada)**:
   - `docker/compose.prod.yml:95-112` → `command: [redis-server, --save, '', --appendonly, yes]`, volumen `redis_data:/data`, `ports: 127.0.0.1:6379:6379`.
   - En vivo: `INFO persistence` → `aof_enabled:1`, `aof_current_size:141114312`, `aof_last_bgrewrite_status:ok`.
   - `/data/appendonlydir` = **339 MB**, con `appendonly.aof.8643.base.rdb` (106 MB), `appendonly.aof.8666.incr.aof` (33 MB) y **dos backups huérfanos** (ver SEC-N2).
   - Host: `/dev/sda1 ext4 /` — **sin capa LUKS/cifrado** (medido con `lsblk`).
   - **Frontera honesta:** un `grep` de texto sobre los AOF devolvió **0** coincidencias del literal `secret_value`, mientras que los **nombres de clave sí aparecen** (1 vez cada uno). Eso **no** prueba ausencia: el base es un RDB binario y los strings largos pueden ir comprimidos (LZF). Lo probado es: (a) el valor en memoria es plaintext (§4.1.5), (b) `appendonly yes` ⇒ los comandos de escritura de esa clave son el contenido del AOF. El cierre exacto está en §7-Q1.
7. **Radio de lectura inmediato**: 25 contenedores en ejecución; `docker network inspect arbitragex-v2_arbx-net` lista 24 servicios, incluidos `searcher-rs`, `sim-ctl`, `anvil`, `math-engine`, `token-enricher`, `recon`, `selector-api`, `grafana`, `loki`, `minio`, **`vault`** y `socket-proxy`. Todos resuelven `redis:6379` (los compose les inyectan `REDIS_URL: redis://redis:6379`).

**Impacto.** Un compromiso de cualquier servicio del plano (o de cualquier proceso local del VPS) no da "acceso a un bus": da **las siete credenciales completas, incluyendo material de clave privada y el token de administración**, y la capacidad de capturar las futuras por `SUBSCRIBE`. La contención requerida (rotación de las 7 + reconfiguración de consumidores) es el mismo coste que un incidente declarado — pero hoy no hay incidente declarado porque el control no existe.

**Lo que NO afirmo (fronteras declaradas):**
- **No sé si `flashbots_signer` es la clave firmante viva.** `relays-client` obtiene su firmante por `Signer::from_env(chain_id)` (`main.rs:190`) y **no encontré ningún lector en runtime** de `arbx:svc_cred:flashbots_signer` (solo el validador, la migración 057, el catálogo del frontend y el comentario de contrato). Es decir: puede ser custodia duplicada **o** material dormido. **No lo trato como dormido**: el cierre de esa ambigüedad es un comando de huella (§6-R1).
- No medí si el disco del VPS tiene cifrado a nivel de bloque del proveedor (solo que **no hay LUKS** en el árbol de dispositivos).
- No verifiqué qué autoriza hoy `github_token` ni `alchemy_prices` en sus emisores.

**Reparación (propuesta, no aplicada).** Tres capas, en orden de coste:
1. **Sacar el secreto del data-store compartido**: que el consumidor reciba una credencial de vida corta emitida por api-server (o Vault, que **ya está desplegado** — `compose.prod.yml:834-862`, `hashicorp/vault:1.18.1` — y **sin consumidores en el backend**: cero referencias a `VAULT_ADDR`/`VAULT_TOKEN` en `backend/api-server/src` ni `backend/shared-rs/src`). Hoy hay un primitivo de aislamiento corriendo, sin cablear.
2. **Si se mantiene el bus**: `requirepass` + usuario ACL por consumidor (`+get` acotado a `arbx:svc_cred:<provider>:*`, sin `+@all`, `resetchannels`), `EXPIRE` en la proyección, y **AOF desacoplado del secreto** (p.ej. excluir la familia `arbx:svc_cred:*` de la persistencia escribiéndola a un keyspace no persistido, o desactivar AOF para esa DB).
3. **Aplicar el sobre que ya existe**: el circuito `encryptSecret/decryptSecret` + migración 120 está construido y sin usar; con `secret_ciphertext` poblado, PG deja de ser un archivo de claves y el mirror pasa a ser un artefacto cifrado bajo clave por consumidor.

**Prueba de cierre (§6-C1).**

### 4.2 F31 — CONFIRMADO end-to-end: la página SSR entrega el inventario a un visitante anónimo

**Control de la API: funciona (medido en vivo, sin credenciales):**

| Petición (desde el VPS al edge `172.18.0.25:8787`) | Resultado |
|---|---|
| `GET /api/credentials` sin nada | **401** |
| `GET /api/credentials` con `Cookie: arbx_admin_session=forged-not-a-token` | **401** |
| `GET /api/credentials` con `x-arbx-admin-token: __session_active__` | **401** |
| `GET /api/credentials/summary` sin nada | **200** (documentado como no-gated, solo conteos — `routes/credentials.ts:247-253`) |

**Y la página lo saltea (medido en vivo, sin credenciales):**

| Petición | Resultado |
|---|---|
| `GET http://172.18.0.26:5173/settings/credentials` (frontend, sin cookie ni header) | **HTTP 200, 102212 bytes**, con `initialSnapshot` =1, `value_suffix` =1, `last_validation_error` =1 presentes en el HTML servido |
| `GET http://127.0.0.1:80/settings/credentials` con `Host: localhost` (nginx host) | **HTTP 200, 102212 bytes, `initialSnapshot`=1** |

**Causa raíz (código anclado):** `frontend/app/settings/credentials/page.tsx` (blob `bd6dd4131495974539f7f06a9393a4d42c0dc51a`):
- `:38-40` el comentario lo declara: *"SSR has no browser session — authenticate admin-gated reads with the runtime token"*.
- `:41-51` `ssrAdminHeaders()` devuelve `x-arbx-admin-token: process.env.ARBX_ADMIN_TOKEN`.
- `:53-58` `fetchInitial()` llama a `${EDGE_URL}/api/credentials` **con esas cabeceras**.
- `:98-110` `CredentialsPage()` no consulta ni sesión ni rol: renderiza para quien sea.
- `page.tsx:28-29` `dynamic = "force-dynamic"`, `revalidate = 0` ⇒ **cada request anónimo ejecuta el fetch privilegiado**.

**Alcance de la divulgación (medido, sin inflar):** la forma pública del registro es `CredentialRowPublic` (`credentials/store.ts:54-74`, blob `bf054983…`): `id, provider, scope, display_name, has_value, value_suffix, status, last_validated_at, last_validation_error, metadata, updated_at, updated_by`. Es decir: **inventario completo de proveedores y scopes, estados, errores de validación, marcas de tiempo, autor, y la máscara de cada secreto**. `value_suffix` viene de `maskHint(secret_value)` (`store.ts:59`) ⇒ **sufijo del secreto real** en el HTML.
**Verifiqué además que `metadata` NO contiene URLs ni claves**: `jsonb_object_keys(metadata)` devuelve vacío para las 5 filas globales y solo `_validation` para `rpc_http`/`rpc_ws`. Con eso, el impacto es **reconocimiento de alta calidad + material parcial de clave**, no exfiltración del secreto.

**Alcance de red (dos eslabones medidos, el tercero declarado):** medido — (a) el contenedor frontend sirve la página anónimamente; (b) el nginx del host (`server_name <VPS_IP>`, `proxy_pass http://127.0.0.1:5173`) escucha en `0.0.0.0:80` y devuelve 200 con el snapshot. **No verificado**: que un cliente externo alcance ese :80 (depende de firewall/Cloudflare). No lo afirmo.

**Reparación.** Tres opciones, de menor a mayor alcance:
1. **Gate de visitante en el borde de la página**: comprobar la sesión admin antes de ejecutar `fetchInitial()` (p.ej. leyendo la cookie en el server component y redirigiendo a `/admin/signin`; hoy **no existe `middleware.ts` en todo el repo**, así que no hay protección centralizada de rutas).
2. **No usar el token de proceso para datos por-usuario**: el SSR debe propagar la credencial del visitante (cookie) al upstream, y degradar a "no computado" si no la hay. Es el mismo patrón que el resto de superficies ya usan vía `getAdminToken()`.
3. **Endurecer la respuesta pública**: si el inventario debe verse sin sesión, servir una allowlist estricta (provider/scope/status) sin `value_suffix`, sin `last_validation_error` y sin `updated_by`.

**Prueba de cierre (§6-C2).**

### 4.3 F33 — CONFIRMADO: no hay relectura de autoridad antes de `broadcast()`

`backend/relays-client/src/submit_engine.rs` (orden medido por líneas):

| Línea | Qué ocurre |
|---|---|
| `:220-384` | Checklist pre-ejecución (12 pasos, `pre_execute_checklist(&mut ctx)` en `:330`). **Aquí se leen kill-switch, paper-mode y breaker.** |
| `:392-396` | Kill-switch "legacy fallback" (solo cuando no hay pool PG). |
| `:487` | `build_and_sign(...)` → **se firma**. |
| `:615` | `assert_bundle_head(...)` → 1 RTT de RPC (frescura de cabeza). |
| `:621-748` | `flashbots.call_bundle(...)` → **1 RTT HTTP al relay**; error de endpoint ⇒ drop fail-closed. |
| `:750` | `assert_bundle_head(...)` otra vez → otro RTT. |
| `:767` | **`multi_relay.broadcast(&bundle, signer)`** ← la tx sale. Sin ninguna lectura de kill-switch/autoridad desde `:330`. |

**Por qué es un hallazgo y no un detalle:** la doctrina del proyecto (`arbx-pre-execute-checklist`, ítem 6 y "el log estructurado se escribe **antes** del broadcast — si hay crash después, no hay evidencia") exige que el kill-switch esté *reachable* en el momento del compromiso. Aquí el gate se leyó **antes de firmar**, y entre ese instante y el envío hay firma + 2 RTT que **pueden dilatarse sin cota** (timeouts de RPC/relay): la ventana no es "unos ms fijos", es "lo que tarde el relay en contestar o en expirar". Un operador que arma el kill-switch durante esa ventana **no detiene esa ejecución** — y el log de checklist ya está escrito, así que la evidencia dirá "checklist OK" para una tx enviada después de la orden de parada.

**Reparación mínima (2 lecturas, ~1 RTT):** inmediatamente antes de `:767`, releer (a) `kill_switch.is_enabled()` y (b) el estado de autoridad/paper-mode efectivo; si cambió, `Self::dropped(opp, "authority_changed_pre_broadcast")` y registrar el tiempo transcurrido desde la checklist. Añadir el ítem al contrato: "T11 — autoridad fresca ≤1 RTT antes del broadcast", con test negativo (flip de kill-switch entre `assert_bundle_head` y `broadcast` ⇒ **cero** broadcasts, hoy imposible de satisfacer por el orden actual).

**Prueba de cierre (§6-C3).**

### 4.4 F34 — CONFIRMADO: el cookie de sesión es una copia del token maestro y no es revocable

Medido en `edge/worker/src/index.ts` (mismo archivo citado arriba):

| Línea | Hecho |
|---|---|
| `:174-176` | `SESSION_COOKIE = "arbx_admin_session"`, `SESSION_TTL_S = 8*60*60`. |
| `:940-956` | `POST /admin/session` valida el token **sondeando** api-server `/admin/killswitch` con `x-arbx-admin-token: <token>`; 401/403 ⇒ `invalid_admin_token`. |
| `:961-967` | `setCookie(c, SESSION_COOKIE, token, {httpOnly, secure, sameSite:"Strict", maxAge: 8h})` → **el valor del cookie ES el `ARBX_ADMIN_TOKEN`**, no un identificador de sesión. |
| `:968-973` | Segundo cookie `arbx_admin_session_ttl` con `expiresAtMs` — **sin `httpOnly`**. |
| `:977-982` | `POST /admin/session/logout` = `deleteCookie` × 2. **No hay revocación en servidor.** |
| `:988, 1056, 1146, 1188, 1217` | Cinco rutas admin resuelven `adminToken = header (si ≠ centinela) : getCookie(SESSION_COOKIE)` → **la cookie es la capability**. |

Y la búsqueda de lectores del TTL (repo completo): `SESSION_TTL_COOKIE` aparece **solo** en `edge/worker/src/index.ts:968,980` (escritura) y en `frontend/lib/admin-token.ts:59,100` (lectura en el navegador). `SESSION_TTL_S` solo en `:176,959,966,972` y en `edge/dev-local/src/index.ts:851,924-928`. **Ningún componente del servidor lee el vencimiento**: la autoridad de la cookie es la vigencia del token maestro.

**Consecuencias medidas (no inferidas):**
1. `logout` **no revoca**: borra la copia del cliente. Un valor capturado sigue siendo aceptado mientras `ARBX_ADMIN_TOKEN` no cambie.
2. El `maxAge` es una instrucción al navegador: quien tenga el valor puede re-fijar el cookie con vencimiento nuevo; el servidor compara el **valor**, no la edad.
3. Basta rotar el token maestro para revocar todo — y eso **corta a la vez** el SSR (F31), los consumidores CLI y cualquier automatización que use `ARBX_ADMIN_TOKEN`. La operación de revocación existe, pero es de "big bang": por eso conviene un modelo de sesión server-side antes de que haga falta usarla.
4. La auditoría original decía "no encuentro cierre de revocación server-side robusta de una copia del token maestro". **Eso es exacto y ahora está medido**: no es que falte un mecanismo de revocación roto, es que **el diseño hace imposible revocar sin rotar**.

**Nota positiva de la misma superficie:** el probe de validación **no** tiene efecto de estado (§5.1) y las cookies son httpOnly+Strict+secure (con HTTPS). El defecto es de **arquitectura de sesión**, no de higiene de cookie.

**Reparación.** Emitir un **id de sesión opaco** (UUID/aleatorio) en la cookie y un registro server-side (`sessions(id, subject, expires_at, revoked_at, role)`) que el edge/api-server consulten; el token maestro nunca viaja al navegador. Con eso: logout revoca de verdad, hay TTL del servidor, y se puede añadir rol (hoy hay un único rol: admin — no existe distinción viewer/operator/admin en ninguna ruta medida).

**Pruebas de cierre (§6-C4, C5).**

### 4.5 F13 — CONFIRMADO, con corrección de alcance: la sesión da admin en HTTP y **no** en WS

**Medido en `backend/api-server/src/websocket.ts` (rev auditado):**

| Línea | Hecho |
|---|---|
| `:264-271` | Comentario "A1 fix (audit 2026-05-10): WebSocket handshake authentication": sin esta puerta cualquier cliente se suscribiría a `subscribe:opportunities`/`subscribe:telemetry`. |
| `:267-270` | **Tres fuentes** documentadas, en orden: (1) `auth.token`, (2) **query param `?token=`**, (3) header `X-ArbX-Admin-Token`. **La cookie NO es una fuente.** |
| `:272-296` | `extractHandshakeToken` implementa exactamente esas tres; devuelve `''` si ninguna sirve. |
| `:382` | `const expectedAdminToken = process.env['ARBX_ADMIN_TOKEN'] ?? ''`. |
| `:383-398` | `io.use(...)`: si `got && expectedAdminToken && safeTokenEqual(got, expectedAdminToken)` ⇒ `data.runtimeAckAllowed = true` (salas sensibles). **En cualquier otro caso llama a `next()` igual**: la conexión se acepta como pública (decisión documentada en `:384-387`: el stream de oportunidades es público y los endpoints de ejecución siguen tras `requireAdminToken` HTTP). |
| `:271` | `safeTokenEqual` — **comparación en tiempo constante** (coincide con la doctrina `arbx-k-k007`). |

**Consecuencias medidas:**
1. **La afirmación del informe es correcta**: "el gateway Socket.IO compara el token de handshake contra `ARBX_ADMIN_TOKEN`; no valida esa cookie como capability WS". La cookie de sesión (`arbx_admin_session`) **no** se consulta en el WS.
2. **Corrección de alcance al informe**: esto **no** es escalamiento (el WS público es una decisión declarada y las salas sensibles quedan tras el token literal). Es una **inconsistencia de contrato de capability**: el mismo portador (la sesión del navegador) puede accionar el kill-switch por HTTP (`edge/worker/src/index.ts:988`) y **no** puede suscribirse a `runtime_ack` por WS; y al revés, un cliente que use el WS con el token maestro obtiene por WS lo que el navegador no tiene. La pregunta correcta para el contrato no es "¿el WS valida la cookie?" sino **"¿qué superficie acepta qué portador?"** — hoy la respuesta es distinta en HTTP y en WS.
3. **Positivos medidos (no son hallazgos):** comparación en tiempo constante; `expectedAdminToken === ''` desactiva la concesión (fail-closed: sin env no hay capability admin por WS); las salas públicas son una decisión declarada y el kill-switch HTTP no depende de esta puerta.
4. **Derivada a hallazgo propio**: la fuente (2) mete el token maestro en la URL → **SEC-N3** (§4.10).

### 4.6 F32 — PARCIAL con un hallazgo nuevo P0: el breaker no tiene productor

**Lo que funciona (medido en código + runtime):**
- `pre_execute_checklist.rs:212-251` ejecuta 12 pasos; ítem 1 = `check_kill_switch`, ítem 12 = `check_circuit_breaker_off`.
- Kill-switch (`:263-279`): JSON ilegible ⇒ `Err(KillSwitch)` (**fail-closed**, `:272-275`); `redis.get` con `?` ⇒ error de Redis propaga ⇒ **fail-closed**; clave ausente ⇒ "off" (documentado). **En vivo la clave existe** (`arbx:killswitch`, `EXISTS=1`, `TYPE=string`) ⇒ el control tiene productor real.
- Paper-mode (`:296-298`): delega en `PaperModeClient::is_enabled_for_chain` con `default_when_absent = cfg.execution.paper_mode` = **true** (comentario `:287-292`) ⇒ clave ausente = **paper ON** = broadcast suprimido. **En vivo `arbx:papermode:global` no existe** ⇒ el default seguro está activo. Esto es un control correcto y es la razón por la que hoy nada se firma.

#### 4.6.1 SEC-N1 (NUEVO) — Check 12 es un gate que no puede dispararse

- Lector: `pre_execute_checklist.rs:32` `CIRCUIT_BREAKER_KEY = "arbx:circuit_breaker:state"`, `:556-559` `let raw = redis.get(KEY).await?; ... None => Ok(())  // Absent = normal`.
- **Productor: no existe.** Un `git grep` repo-wide de `circuit_breaker`/`CIRCUIT_BREAKER`/`circuitBreaker` sobre el rev auditado (excluyendo `docs/**`, `audits/**`, `*.md`, `*.log`) devuelve: el lector, los endpoints admin `GET/POST /admin/circuit_breakers[/:name/{trip,reset}]` de api-server (que leen `cfg.circuit_breakers` y escriben auditoría), `risk-circuit-breakers.ts` (que persiste `risk_events` y métricas Prometheus) y textos de readiness. **Ninguno escribe la clave Redis.**
- Corroboración en el propio código: `api-server/src/index.ts:456` documenta *"Live state available at selector-api:/metrics (arbx_cb_state{name=...})"* — el estado del breaker vive en **métricas**, no en la clave que la checklist lee.
- Runtime: `EXISTS arbx:circuit_breaker:state` → **0**; `--scan 'arbx:circuit_breaker*'` → **0 claves**.

⇒ El ítem 12 de la barrera pre-ejecución es **decorativo**: nunca puede bloquear. Es la clase R10 ("cable sin productor") aplicada a un gate de seguridad: el log de checklist dirá "12/12 OK" incluso con el detector de anomalías disparado. Los breakers *sí* existen y *sí* emiten (Prometheus/risk_events), pero **no están conectados al punto de decisión**.

**Reparación:** o el productor escribe la clave (uno de los dos contratos debe ceder; el más barato es que el worker de recon publique `arbx:circuit_breaker:state` junto a la métrica), o el ítem 12 se reescribe para consultar la fuente real y **se declara su ausencia como `NO_COMPUTADO` en vez de `OK`** (doctrina R8/R10: "la ausencia de cómputo jamás se viste de éxito"). Prueba de cierre en §6-C6.

### 4.7 F35 — PARCIAL: SBOM existe, es fail-open y no está correlacionado con lo desplegado

**Lo que hay (medido):**
- `.github/workflows/docker-build.yml:44-52`: `anchore/sbom-action` + `cosign sign --yes ghcr.io/<repo>:<sha>` sobre la imagen publicada. Este es el eslabón **bueno** (SBOM ligado a un artefacto con digest).
- `.github/workflows/security.yml:330-398`: job `sbom` **`continue-on-error`**, con comentario explícito en `:331-333`: *"Marked continue-on-error because cargo-cyclonedx / cosign may not be pre-installed and we do NOT want SBOM tooling to block merge"*; `:357` "npm sbom not available; skipping"; `:371` instalación fallida ⇒ `exit 0`; `:388` "cosign not available; skipping attestation". Es decir: **por diseño no bloquea y no falla**.
- Lockfiles presentes: `backend/Cargo.lock`, `package-lock.json`, `tests/e2e/package-lock.json`, `tests/v3-fee-units/Cargo.lock`. **No hay `pnpm-lock.yaml`** (aunque existe `frontend/.pnpm-approvals.json`), ni `deny.toml`/`audit.toml`.

**SBOM evaluado — lo que medí y lo que no:**
- **Medido (nuevo):** `npm audit --package-lock-only --json` sobre el `package-lock.json` **del rev auditado** → `{critical: 4, high: 19, moderate: 32, low: 0, total: 55}` sobre 1465 dependencias (prod 1 / dev 1465 / optional 1447 / peer 1447). Comando reproducible en §6-C7.
- **No medido:** qué advisories de esos 55 son **alcanzables en el artefacto desplegado** (la mayoría del árbol es dev/optional; el paquete `prod` es 1). Un advisory no es una explotación, y un bump no es una reparación sin regresión.
- **No medido:** el lado Rust (`cargo audit`/`cargo-deny` no ejecutados aquí; `forge` está bloqueado por AppControl en este host, y no instalé nada). `deny.toml` no existe ⇒ **no hay política formal de excepciones** (F35 lo pedía y sigue abierto).

**Reparación (Release/SRE, no código):** (1) hacer **bloqueante** el job SBOM o, como mínimo, quitar los `|| exit 0` y publicar el resultado como check informativo con retención; (2) **ligar el SBOM al digest desplegado** (el deploy conoce el SHA de imagen: exigir que el SBOM del digest exista antes de `up -d`); (3) crear `deny.toml`/política de excepciones con dueño y caducidad por entrada; (4) triaje de los 55 advisories **por ruta de ejecución**, no por nombre de paquete.

### 4.8 F36 — NO COMPUTADO (no "sin hallazgos"): el artefacto no tiene productor en este repo

**Corrección de método aplicada (auditoría de evidencia de esta misma afirmación).** Escribir "no encontré traza" invita a leerse como "no hay nada que reportar". Lo que hay es un **dato no computado con causa declarada**: el estado de la clave WS *no es medible desde el repositorio* porque **ningún componente del repo produce un artefacto de rotación**.

Artefacto de la búsqueda (reproducible, y esto es lo único que afirmo):

```
$ git grep -n -e "ARBX_WS_KEY" -e "WS_KEY" -e "ws_token" -e "wsToken" \
      bceb31ef7c0fd6d4b50bcdee6fc8d33858334822 -- backend frontend edge
# → 0 coincidencias (exit 1)
$ git grep -rln -i -e "ws.*key.*incident" -e "incident.*ws" ... -- docs
# → 1 fichero: docs/superpowers/specs/2026-05-11-event-driven-orchestrator.md
```

**Lectura correcta (R8/R10):** `NO_COMPUTADO`, con `reason = rotation_artifact_has_no_producer_in_repo`. Esto **no** dice que la clave siga viva, **no** dice que esté muerta, y **no** cierra el hallazgo: dice que el único productor posible del cierre es el operador (huella + reconexión forzada, §6-C8). Tratar esta fila como "sin hallazgos" sería exactamente el error que el proyecto ya pagó con `drift_observations` (tabla sin escritor leída como "COHERENT").

### 4.9 SEC-N2 (NUEVO) — AOF huérfanos con el mismo material, sin dueño ni ciclo de vida

`ls -la /data/appendonlydir` (dentro del contenedor Redis, en vivo):

| Fichero | Bytes | Fecha | Naturaleza |
|---|---:|---|---|
| `appendonly.aof.8643.base.rdb` | 106 091 660 | 2026-10-06 | base vigente |
| `appendonly.aof.8666.incr.aof` | 33 644 189 | 2026-10-06 | incremental vigente |
| `appendonly.aof.3676.incr.aof.bak-20260919` | 116 939 900 | 2026-09-19 | **huérfano** (`.bak`) |
| `appendonly.aof.4975.incr.aof.bak-diskfull-20260923` | 90 680 568 | 2026-09-23 | **huérfano** (incidente de disco) |

Total ≈ **347 MB** de material persistido, de los cuales **≈208 MB son copias que ningún proceso mantiene, referencia ni rota** (propietario `redis:root`, modo 600). Son consecuencia de incidentes operativos (uno se llama literalmente `diskfull`) y contienen el mismo keyspace — es decir, el mismo material de credenciales — en versiones *anteriores*. Ninguna política de retención los cubre. **Impacto:** duplica (y fosiliza) la superficie de T3 y complica cualquier rotación: rotar el secreto no borra las copias viejas. **Reparación:** dueño + política de retención + borrado seguro de los `.bak` **después** de rotar (no antes: hoy contienen material vivo).

### 4.10 SEC-N3 (NUEVO, latente) — el contrato del WS admite el token maestro en la URL

**Medido:**
1. `backend/api-server/src/websocket.ts:267-270` documenta y `:281-287` implementa la fuente **(2) query param**: `handshake.query['token']` se acepta como token admin válido.
2. `access_log /var/log/nginx/access.log;` — y **ningún `log_format` personalizado** en `/etc/nginx/` (grep → 0 coincidencias) ⇒ aplica el `combined` por defecto de nginx, cuyo campo de request **incluye la query string**. El log pesa 706 KB y persiste en `/var/log/nginx/` sobre el mismo disco sin cifrar (§4.1.6).
3. **Uso real del fallback: 0.** `grep -c "token=" /var/log/nginx/access.log` → **0** sobre 3736 líneas; `access.log.1` → **0** sobre 4526 líneas. En el mismo barrido, `socket.io` aparece 41+1 veces ⇒ la superficie WS se usa, y se usa **sin** el fallback por query.

**Calificación honesta: P2 (endurecimiento latente), no P1.** No hay evidencia de que nadie haya puesto el token en una URL en este deployment. Lo que hay es: un contrato de servidor que *acepta* el token por URL, un logger que *persiste* la URL en disco, y un disco *sin cifrar*. Es la combinación la que convierte un fallback de comodidad en un vector de captura; hoy no se ha materializado.
**Reparación (una línea):** eliminar la fuente (2) de `extractHandshakeToken` y dejar `auth.token` + header. El fallback del navegador deja de ser necesario en cuanto exista el modelo de sesión de §4.4.
**Prueba de cierre:** `curl` de handshake con `?token=<valor>` debe rechazar la concesión de `runtimeAckAllowed`, y `grep -c "token=" /var/log/nginx/access.log` debe mantenerse en 0 tras la ventana de prueba.

---

## 5. Controles que SÍ funcionan (verificados, para no tratarlos como dudosos)

### 5.1 SEC-P1 — El probe de autenticación no tiene efecto de estado
`POST /admin/session` valida sondeando `POST /admin/killswitch` con `{enabled: null, reason: "__session_probe__"}` (`edge/worker/src/index.ts:944-952`). Verificado que **no altera el kill-switch**: en api-server el handler está declarado `app.post("/admin/killswitch", requireAdminToken(ARBX_ADMIN_TOKEN), ...)` (`index.ts:264`) y el body se valida con `KillSwitchReq = z.object({enabled: z.boolean(), ...})` (`index.ts:259-263`): `null` **no es boolean** ⇒ `safeParse` falla ⇒ `400` y `return` en `:266-269`, **antes** de `killSwitch.set` (`:273`). El probe interpreta solo 401/403 como fallo (`:953`), así que un token válido devuelve 400 y un token inválido 401. **Contrato correcto: la validación precede al efecto.** (Merece un test que lo fije: "el probe con enabled:null NO invoca killSwitch.set".)

### 5.2 SEC-P2 — El control admin de la API rechaza lo indebido (medido en vivo)
Ver tabla en §4.2: anónimo 401, cookie forjada 401, centinela forjado 401. La operación legítima no se rompe (el SSR con token de proceso obtiene 200, y `summary` sigue siendo público por diseño documentado). **Criterio de aceptación #1 del perfil: satisfecho en esta superficie.**

### 5.3 SEC-P3 — Ausencia de estado ⇒ comportamiento seguro (paper ON)
`arbx:papermode:global` no existe en runtime y `arbx:papermode:<chain>` tampoco; por `PaperModeClient.default_when_absent = cfg.execution.paper_mode` (=`true`, comentario `pre_execute_checklist.rs:287-292`) el sistema está en paper ⇒ `PaperModeActive` ⇒ broadcast suprimido (`submit_engine.rs:345`). La ausencia de dato **no** se interpretó como "live". Es el patrón anti-R10 bien aplicado y contrasta con SEC-N1 (donde la ausencia sí se interpreta como "OK").

---

## 6. Reproducciones seguras y pruebas de cierre

> Ninguna de estas pruebas imprime un valor de secreto. Las que requieren leer un valor están marcadas y **no las ejecuté**.

| ID | Prueba | Cómo se ejecuta | Criterio de cierre |
|---|---|---|---|
| **C1** | Exposición de secretos (F30) | `bash docs/security/sec-custody-identity-01-probe.sh` | (a) `requirepass` configurado y `ACL LIST` sin `~* &* +@all`; (b) `SELECT count(*) FROM service_credentials WHERE secret_ciphertext IS NULL` = **0**; (c) ningún fichero en `/data/appendonlydir` contiene la familia de claves; (d) el script vuelve a imprimir **0** ocurrencias de `arbx:svc_cred:` fuera del proceso consumidor autorizado |
| **C2** | Render anónimo (F31) | `curl -s -o /tmp/p.html -w '%{http_code}' http://<frontend>:5173/settings/credentials; grep -c initialSnapshot /tmp/p.html` | `302`/`401` **o** HTML sin `initialSnapshot`, sin `value_suffix` y sin `last_validation_error` |
| **C3** | Autoridad fresca pre-broadcast (F33) | Test de integración en `relays-client`: conmutar el kill-switch tras `assert_bundle_head` y antes de `broadcast()` | **0** llamadas a `multi_relay.broadcast` y `dropped("authority_changed_pre_broadcast")` |
| **C4** | Revocación real de sesión (F34) | Iniciar sesión → capturar el valor de la cookie → `POST /admin/session/logout` → reproducir **el mismo valor** contra una ruta admin | El replay devuelve **401** sin haber rotado el token maestro |
| **C5** | Escalamiento de rol / anónimo en SSR+WS (F34/F13) | Petición anónima a `/settings/credentials`, al handshake WS y a `/api/v1/credentials` | Los tres rechazan; el journey legítimo (con sesión) sigue funcionando |
| **C6** | Breaker conectado (SEC-N1) | Forzar el estado del breaker en su fuente real y ejecutar la checklist | `checklist_blocked: circuit_breaker` con la clave presente; y con la clave **ausente y la fuente no consultable**, veredicto `NO_COMPUTADO`, nunca `OK` |
| **C7** | SBOM/advisories (F35) | `npm audit --package-lock-only --json` (rev auditado) + SBOM del digest desplegado | El job SBOM **falla** cuando no puede generar el artefacto; existe SBOM del digest exacto servido en prod; cada excepción tiene dueño y caducidad |
| **C8** | Clave WS (F36) — **solo operador** | `printf '%s' "$WS_KEY" \| sha256sum \| cut -c1-12` en cada consumidor, comparar contra la huella del incidente, forzar reconexión y verificar que la credencial vieja **no** autentica | Corte verificado: la credencial expuesta deja de autenticar y todos los consumidores reconectan con la nueva |
| **C9** | Recolección por pub/sub (F30/T2) | `redis-cli SUBSCRIBE arbx:svc_cred:reload` desde un contenedor **no** autorizado | La suscripción es rechazada (`NOPERM`) |

---

## 7. Fronteras no verificadas (declaradas, con el comando que las cerraría)

- **Q1 — ¿El AOF contiene el valor o solo la clave?** El `grep` de texto dio 0 para `secret_value` **y** los nombres de clave aparecen: el base es RDB binario con posible compresión LZF, así que un grep negativo **no prueba ausencia**. Cierre sin exponer el valor: `redis-cli OBJECT ENCODING <key>` (ya medido: `raw`) + `redis-check-aof` sobre una **copia** en un Redis desechable y comparación por `STRLEN` (nunca imprimiendo). **Declarado: no ejecutado** (requiere instanciar un Redis aparte, fuera de mi permiso).
- **Q2 — ¿`flashbots_signer` es la clave firmante viva?** `relays-client` usa `Signer::from_env` (`main.rs:190`) y no encontré lector runtime de la proyección. Cierre por huella: `printf '%s' "$SIGNER_KEY" | sha256sum | cut -c1-12` vs la huella del valor almacenado — comparación de identidad **sin revelar el valor**. No ejecutado: no tengo ni debo obtener el valor.
- **Q3 — Handshake WS (F13).** **RESUELTO** (§4.5): el gateway no consulta la cookie; acepta `auth.token`/query/header. Derivó en SEC-N3 (§4.10).
- **Q8 — ¿Algún cliente futuro usará el fallback por query?** Hoy no (0/8262). No es medible hacia adelante; por eso el cierre de SEC-N3 es quitar la fuente, no vigilar el log.
- **Q4 — Alcance externo de la página (`<VPS_IP>:80`).** Medido: nginx escucha en `0.0.0.0:80` y devuelve el snapshot con `Host` local. **No medido**: si un cliente de Internet alcanza ese puerto (firewall/Cloudflare). No lo afirmo.
- **Q5 — Aplicabilidad de los 55 advisories** al artefacto desplegado (§4.7).
- **Q6 — Cifrado del disco del proveedor** (solo medí que no hay LUKS en el árbol de dispositivos).
- **Q7 — Prompt injection en el arnés de agentes (T8)**: no medido en esta pasada, y deliberadamente fuera de esta contribución (pertenece a AIEngineering + Security como trabajo conjunto).

---

## 8. Handoffs

| Para | Qué | Por qué es suyo | Evidencia que debe devolver |
|---|---|---|---|
| **Operador (PersonalOps)** | Rotar `flashbots_signer`, `admin_token`, `edge_token`, `github_token`, `alchemy_prices`, `rpc_http/ws:chain:1` **y** decidir sobre el disco/AOF | Ninguna reparación de código cierra T1/T2/T3 | Huella antes/después + consumidores actualizados + `.bak` retirados **después** de rotar |
| **SRE** | (a) `requirepass`/ACL por consumidor en Redis; (b) retención + dueño de `/data/appendonlydir`; (c) quitar TTL ilimitado de la proyección | Configuración de producción | `ACL LIST` por consumidor (sin `~* &* +@all`), `TTL` de las claves, inventario de `.bak` |
| **Architect** | Modelo de sesión con id opaco + roles (hoy: un solo rol admin; cookie = capability) | Cambia contrato compartido (edge + api-server + frontend) | ADR con el contrato de cookie y su revocación |
| **Frontend/Implementer** | Gate de visitante en la página SSR y/o propagación de la credencial del visitante | Recorrido funcional | C2 verde + journey legítimo intacto |
| **Backend (websocket.ts)** | Paridad de capability HTTP↔WS: que el WS acepte el mismo portador que el HTTP (cookie de sesión) **y** eliminar la fuente por query param | Contrato de identidad compartido | Handshake con cookie concede lo mismo que el HTTP; `?token=` deja de conceder; 0 hits de `token=` en el access log |
| **Contracts/Backend (relays-client)** | Relectura de autoridad ≤1 RTT antes de `broadcast()` | Carril de ejecución | C3 verde |
| **Data** | Cablear el sobre ya construido (`crypto.ts` + migración 120) o retirarlo | Persistencia de secretos | `count(secret_ciphertext IS NULL) = 0` |
| **Release** | SBOM bloqueante + ligado al digest desplegado + `deny.toml` con excepciones caducadas | CI/CD y attestation | C7 verde |
| **QA** | Ejecutar C1-C7 y devolver PASS/FAIL por criterio | Verificación independiente | Tally por prueba, sin prosa |
| **Reviewer** | Revisión independiente de §4.1 y §4.6.1 (los dos P0 que propongo) | Validación independiente | Dictamen con evidencia |

---

## 9. Anexo — procedencia, método y declaraciones

### 9.1 Anclaje de cada cita
Cada afirmación de código se verificó contra el **blob** del rev auditado:

```
git rev-parse bceb31ef7c0fd6d4b50bcdee6fc8d33858334822:<path>   # blob id (sha1 de los bytes)
git show    bceb31ef7c0fd6d4b50bcdee6fc8d33858334822:<path>     # contenido citado
```

| Archivo | blob | Líneas citadas |
|---|---|---|
| `backend/api-server/src/credentials/projection.ts` | `31e679fe7e3eb7ca1a75b5e916823a8efc983300` | 10-13, 37-45, 57-75, 107-179 |
| `backend/api-server/src/credentials/store.ts` | `bf0549839f8a36093df27187bca466fd005f9f27` | 54-74 |
| `backend/api-server/src/credentials/crypto.ts` | `48d3f4c6788b086848b1477ee83d00343a383d73` | (sobre cifrado, no aplicado) |
| `backend/api-server/src/routes/credentials.ts` | `51e694cb062ba21cf54cc81bbea5994c8d4b422c` | 233-280, 305-406 |
| `frontend/app/settings/credentials/page.tsx` | `bd6dd4131495974539f7f06a9393a4d42c0dc51a` | 28-29, 38-58, 98-110 |
| `backend/api-server/src/index.ts` | (index.ts del rev) | 259-269, 264, 451-475 |
| `backend/api-server/src/credentials/validators.ts` | (rev) | 218-226, 387 |
| `shared-ts/src/contracts/credentials.ts` | (rev) | 20 |
| `edge/worker/src/index.ts` | (rev) | 174-176, 262-285, 921-1000, 1056, 1146, 1188, 1217 |
| `edge/dev-local/src/admin-token-resolver.ts` | (rev) | 13-38 |
| `backend/api-server/src/websocket.ts` | (rev) | 264-296, 382-399 |
| `backend/shared-rs/src/pre_execute_checklist.rs` | (rev) | 32, 212-251, 260-298, 553-559 |
| `backend/relays-client/src/submit_engine.rs` | (rev) | 220-396, 487, 600-625, 735-800 |
| `backend/relays-client/src/main.rs` | (rev) | 190, 121-123, 439-455 |
| `docker/compose.prod.yml` | (rev) | 86-130 (redis), 834-862 (vault) |
| `docker/compose.dev.yml` | (rev) | 81-95 (redis) |
| `.github/workflows/security.yml` | (rev) | 330-398 |
| `.github/workflows/docker-build.yml` | (rev) | 44-52 |
| `.github/workflows/auto-deploy-vps.yml` | (rev) | 211-212 (`COMPOSE_FILE=docker/compose.prod.yml`) |

### 9.2 Comandos en vivo (todos de lectura; ninguno imprime secretos)
Detalle completo y re-ejecutable en `docs/security/sec-custody-identity-01-probe.sh`. Resumen: `docker ps --format`; `redis-cli CONFIG GET appendonly|appendfsync|requirepass` (este último **solo longitud**); `ACL LIST`; `--scan --pattern`; `STRLEN`/`TYPE`/`TTL`/`OBJECT ENCODING`/`EXISTS`; `INFO persistence`; `ls -la /data/appendonlydir`; `lsblk`; `docker network inspect`; `curl` negativos (anónimo/cookie forjada/centinela forjado) y `curl` anónimo a la página SSR con **conteo** de marcadores; `npm audit --package-lock-only --json`; SQL de **longitudes y nombres de claves** (`length`, `octet_length`, `jsonb_object_keys`) — nunca `SELECT secret_value`.

### 9.3 Qué NO hice (frontera de Security)
- **No leí ni imprimí ningún valor de secreto** — ni en el repositorio, ni en Redis, ni en PostgreSQL, ni en los AOF. Toda identificación es por nombre de clave, longitud, tipo o conteo.
- **No roté, no creé, no validé ni usé credenciales**; no ejecuté `SUBSCRIBE` sobre el canal de secretos (C9 es una prueba **especificada**, no ejecutada, porque suscribirse sería recolectar el secreto).
- **No escribí en producción**: cero `SET`/`DEL`/`CONFIG SET`, cero cambios de contenedores, cero reinicios.
- **No toqué `.github/**`, `.gitignore`, `.claude/**` ni la configuración de secretos.**
- **No modifiqué código**: mi única escritura son dos artefactos nuevos en `docs/security/`.
- **No declaré claves muertas ni vivas sin evidencia**: F36 queda NO VERIFICADO; `flashbots_signer` queda con su uso en runtime declarado como frontera (§7-Q2).
- **No afirmo que estos hallazgos sean nuevos en su totalidad**: F30/F31/F33/F34 ya estaban en la matriz del auditor; mi aporte es la **medición** (runtime + aritmética de longitudes + pruebas negativas), el **agravamiento a material de clave privada**, la composición F30+T1/T2/T3, y los hallazgos **SEC-N1/SEC-N2/SEC-N3**.

---

## 10. Oposición formal y calibración de severidad

**Protocolo ejecutado** (regla permanente del proyecto): `adversarial_frame` + `adversarial_verdict` sobre la conclusión de mayor riesgo (F30 elevado a material de clave privada).

- **Marco (`adversarial_frame`):** el clasificador determinista encuadró mi tesis en la modalidad *ausencia* y devolvió una contra-tesis **de otro eje** ("la ausencia es un artefacto de la ventana observada"). **Lo declaro como desajuste de categoría**: mi tesis afirma la *presencia de una exposición*, no la ausencia de un evento. Añadí, por tanto, la contra que un adversario serio plantearía de verdad (dominio de confianza único, clave posiblemente dormida, paper mode, loopback-only).
- **Veredicto (`adversarial_verdict`):** **sobrevive la TESIS** — peso 16 vs 6, con **6 ítems respaldados por artefacto** en el lado ganador (3 en el contrario, 1 recortado por no tener artefacto).
- **Evidencia faltante declarada por el propio veredicto:** (a) "no hubo incidente" no tiene artefacto y no cuenta como evidencia (§1.3/C4); (b) **el uso de la clave almacenada sigue sin resolverse** (§7-Q2). Y una frontera que yo añado: **no existe instrumento que registre lecturas de Redis** en el Redis medido, así que "nadie leyó la proyección" es **NO COMPUTABLE** — ni afirmable ni negable.

**Calibración honesta de severidad (la contra tenía razón en dos cosas):**

| Afirmación | Estado tras la oposición |
|---|---|
| "El control de aislamiento de secretos no existe" | **Sostenido con artefactos.** Es un defecto de diseño vigente, no una hipótesis. |
| "P0 para certificar MAINNET LIVE con capital" | **Sostenido.** Un control inexistente no se puede compensar con un buen comportamiento observado. |
| "Hay capital en riesgo hoy" | **NO sostenido, y lo retiro.** El sistema está en paper (fail-closed, `default_when_absent=true`) y Redis no es alcanzable desde Internet. El defecto limita la *aptitud* hoy; el riesgo *económico inmediato* es bajo. |
| "La clave firmante está comprometible" | **NO sostenido como hecho.** `relays-client` firma con `Signer::from_env` y **no** encontré lector runtime de la proyección. Queda "material de clave privada expuesto con uso runtime no establecido" — no "clave firmante expuesta".

**Una línea para el dictamen del capitán:** F30 no cambia el veredicto de hoy (ya era NO-GO por otros P0), pero **impide cerrar el gate de custodia en cualquier escenario live**, y su remediación tiene la misma forma que una respuesta a incidente (rotar + reconfigurar consumidores), así que su coste sólo sube con el tiempo.

---

## 11. Auditoría de evidencia de este propio documento (resultado crudo, incluidos los fallos)

**Pasada 1** (`metacog_audit`, 8 fragmentos): **7 SOPORTADO / 1 NO_SOPORTADO / 0 CONTRADICHO**, 10 ítems de evidencia, 0 sin artefacto.
- El fragmento no soportado era mi redacción de F36 ("no encontré traza de rotación"): el auditor lo marcó con el código **`ETIQUETA_MAL_LEIDA:sin_productor_leido_como_sin_hallazgos`** — exactamente el modo de fallo que el proyecto ya pagó. **Corregido**: §4.8 se reescribió como `NO_COMPUTADO` con `reason` explícito y con los dos comandos de búsqueda como artefacto.
- Dos ítems de evidencia quedaron **no atribuidos** por no declarar `objetivo` (defecto de forma mío, corregido en la pasada 2).

**Pasada 2** (3 fragmentos): **2 SOPORTADO / 1 NO_SOPORTADO**, 0 sin artefacto, 0 no atribuidos.
- El fragmento restante no soportado es una frase **meta** mía ("las dos cifras 0 y 1 provienen de…") sin cita en línea: el auditor pide el artefacto pegado a la cifra. La sustancia (fragmentos 1 y 2, que sí llevan comando) queda SOPORTADA.

**Bandera residual declarada, no ocultada.** La pasada 2 volvió a emitir `ETIQUETA_MAL_LEIDA:no_computado_leido_como_cero`. Su propio campo de arreglo dice: *"reportarlo como NO COMPUTADO con razon explicita; jamas como 0"* — que es **exactamente** el estado del texto corregido. El disparador es el literal "0 coincidencias", que es el **resultado de una búsqueda** (`git grep` → 0), no el valor de un campo no computado. **Lo declaro como falso positivo de la heurística determinista**: un filtro de forma, no un juez. No "arreglo" el texto agregando una cita que no leí, y no convierto el conteo de una búsqueda en un dato.

**Frontera que este documento NO puede cerrar y que el lector debe tratar como abierta:** no existe instrumento que registre lecturas de Redis en el deployment medido, por lo que "nadie leyó la proyección" es **NO COMPUTABLE**; y el uso runtime de `flashbots_signer` sigue en §7-Q2.
