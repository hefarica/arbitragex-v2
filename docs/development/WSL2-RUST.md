# Compilar Rust en esta máquina — la vía que funciona (WSL2)

> **Corrige la regla 4 de la sección 36 de `CLAUDE.md`** (2026-09-29).
> Antes decía: *"un worktree fresco tiene `target/` frío → `cargo check` falla por Windows AppControl (os error 4551); usa el árbol principal con `target/` caliente para compilar"* — **CORREGIDO 2026-09-29**.
> La causa que atribuía era **falsa** y el consejo **no arregla nada**. Este
> documento dice qué pasa de verdad y qué hacer.

## El error

```
error: failed to run custom build command for `serde_core v1.0.229`
Caused by: could not execute process `...\target\debug\build\serde_core-*\build-script-build`
Caused by: Una directiva de Control de aplicaciones bloqueó este archivo. (os error 4551)
```

## La causa real: Smart App Control en enforcement

Medido el 2026-09-29 en este equipo (Windows 11 25H2, build 26200):

| qué | valor | cómo se comprobó |
|---|---|---|
| Smart App Control | **ON / ENFORCED** | `VerifiedAndReputablePolicyState=1` + `SAC_EnforcementReason=1` |
| WDAC empresarial | **no desplegado** | sin `SiPolicy.p7b`; namespace `MSFT_CIPolicy` inexistente |
| AppLocker | **sin reglas** | `EXE.AppLocker` = 240 B (default vacío); **0 eventos 8004** |
| Modo | **enforced, no auditoría** | eventos 3033 ×460 y **3076/3077 = 0** (audit-only). Un `os error 4551` duro es imposible en modo auditoría |

**El criterio de SAC es FIRMA + REPUTACIÓN**, no la ruta ni la temperatura del `target/`:

- `rustfmt.exe` (misma toolchain, en `~\.rustup`) es `NotSigned` y **ejecuta** —
  está ampliamente distribuido, así que tiene predicción de reputación.
- Un `build-script-build.exe` recién compilado es `NotSigned` **y sin reputación**
  → **se bloquea**.
- No es Mark-of-the-Web: el ejecutable no tiene `Zone.Identifier`.

### Por qué la explicación vieja no se sostiene

1. El binario bloqueado lo compila **cargo, nuevo, en cada build** — da igual si el
   `target/` está caliente o frío. Un `target/` caliente evita *recompilar*, pero no
   puede evitar que los build-scripts que sí corren se ejecuten.
2. **El árbol principal falla igual**: el `cargo check` que reprodujo el 4551 se
   corrió ahí, con su `target/` presente.
3. Medido: los **64 worktrees** del repo **no tienen `target/`** — la premisa de
   partida ni siquiera se cumplía.
4. SAC **no ofrece allowlist ni exclusiones por ruta**. No hay "permitir solo cargo".

## La vía correcta: WSL2

En Linux no existen PE recién compilados, así que SAC nunca los evalúa. **No se toca
ninguna configuración de seguridad de Windows** y es 100% reversible (dejar de usarlo).

En esta máquina WSL2 ya estaba operativo: Ubuntu 26.04, WSL 2.7.3.0, kernel
6.6.114.1, 16 cores, `ext4.vhdx` en `%LOCALAPPDATA%\wsl\{GUID}\ext4.vhdx`.

### Setup (una sola vez)

En una terminal de Ubuntu (menú Inicio → "Ubuntu"), con **tu contraseña de Linux**:

```bash
sudo apt-get update && sudo apt-get install -y build-essential pkg-config libssl-dev
```

Eso trae `cc`/`gcc`/`make`/`pkg-config`, que Rust necesita como driver de enlace y
que varios crates `-sys` necesitan para compilar C.

> **Atajo descartado, para que nadie lo reintente:** la toolchain de rustup ya trae
> `rust-lld`, y es tentador montar un wrapper `~/bin/cc` que lo invoque para no
> instalar nada. **No funciona**: Rust le pasa flags estilo `-Wl,--as-needed`,
> `-B<dir>`, `-nodefaultlibs`, `-m64` que `ld.lld` no acepta (`rust-lld: error:
> unknown argument '-Wl,--as-needed'`). Hace falta un compilador C real.

### Compilar

**Importante: el `target/` va en el filesystem de Linux, no en `/mnt/c`.** Sobre NTFS
la compilación es órdenes de magnitud más lenta.

```bash
# copiar el fuente al FS de Linux (rápido: son ~23 MB sin target/ ni node_modules)
mkdir -p ~/arbx-src
cd "/mnt/c/Users/HFRC/Desktop/arbitragex-v2-main (17)"
tar -cf - --exclude=target --exclude=node_modules --exclude=.next --exclude=.git \
    backend shared-ts | (cd ~/arbx-src && tar -xf -)

# compilar
cd ~/arbx-src/backend
cargo check --workspace --locked
cargo test  --workspace --locked --lib
cargo clippy --workspace --locked -- -D warnings   # el gate de CI
cargo fmt -- --check                               # el gate de CI
```

La toolchain ya instalada es **1.91.0**, que es exactamente la que fija el
`rust-toolchain.toml` del repo — no hace falta instalar otra.

### Para verificar una rama concreta

```bash
cd ~/arbx-src
git clone --filter=blob:none "/mnt/c/Users/HFRC/Desktop/arbitragex-v2-main (17)" repo-tmp
cd repo-tmp && git fetch origin <rama> && git checkout <rama>
cd backend && cargo check --workspace --locked
```

(Clonar desde `/mnt/c` es lento; para uso repetido conviene mantener un clon en
`~` y sólo hacer `git fetch`.)

## Alternativa si no se puede usar WSL2

**CI.** Los gates de Rust corren en GitHub Actions (`Rust CI`, `cargo clippy
-D warnings`, `cargo test --workspace`) y son la verificación que manda antes de
mergear. El costo es el ciclo de iteración, no la validez. Está probado: el PR #722
de esta auditoría pasó 37 checks, incluido el test de regresión nuevo.

## Lo que NO hay que hacer

- **Apagar Smart App Control.** Es **irreversible** sin reinstalar Windows (Microsoft:
  *"can only be enabled on a clean install"*). No es un interruptor para destrabar un
  build, y con WSL2 no hace falta.
- **Exclusiones de Defender.** No aplica: esto es control de aplicaciones del kernel,
  no antivirus.
- **`cargo clean` + recompilar, o cambiar de carpeta.** Ya se probó: el bloqueo es la
  ejecución del build-script, no la caché ni la ruta.
- **Firmar los artefactos con un certificado propio.** SAC solo acepta firmas de una
  CA del Trusted Root Program; un self-signed **no sirve**, y los build-scripts son
  muchos y efímeros.
- **Compilar en el árbol principal "porque tiene target/ caliente".** Es exactamente
  el consejo que este documento corrige.

## Nota sobre disco

Un `target/` de este workspace pesa **~36 GB** y cada sesión que se cree su propio
`CARGO_TARGET_DIR` en `%TEMP%` suma otros ~5-11 GB. El 2026-09-26 ocho árboles de
build duplicados sumaron **108 GB** y dejaron C: al borde del colapso. Si compilás en
WSL2, el `target/` vive en `ext4.vhdx` (que también crece, pero se compacta con
`wsl --shutdown` + `Optimize-VHD`, y no compite por el espacio de C: de la misma
forma). Evitá crear `CARGO_TARGET_DIR` nuevos por sesión.
