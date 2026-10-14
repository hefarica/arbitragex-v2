# `docs/data/` — entregables del perfil **Data**

Directorio canónico de los entregables **versionados** del perfil Data de ArbitrageX v2.

## Qué va acá

Documentos `.md` que fijan **categorías, fuentes y fronteras** de la información de datos:

- clasificaciones contables (ESTIMADO / SIMULADO / REALIZADO) y sus reglas de categorización;
- desgloses de un valor con el comando y la fila exacta que lo produce;
- disposiciones de hallazgos de datos (qué se propone, dónde y en qué orden);
- contratos de esquema y de procedencia de campos.

## Qué NO va acá

**Nada que la regla `data/` de `.gitignore` existe para proteger**: datos locales, dumps, exports, `.sqlite`/`.db`, snapshots de trabajo, logs, salidas de herramientas.
Esa regla **sigue vigente** y este directorio **no** la debilita: la corrección de `GITIGNORE-DOCS-DATA-01` es una negación **anclada a esta ruta exacta** (`/docs/data/`, `.gitignore`), no una negación amplia. Cualquier otro directorio llamado `data` sigue ignorado.

## Nota histórica (por qué existía una trampa)

Hasta `GITIGNORE-DOCS-DATA-01`, la regla **desanclada** `data/` (`.gitignore`, bloque `# Local data`) matcheaba **cualquier** directorio llamado `data` a cualquier profundidad, incluido `docs/data/`. Consecuencia medida: `docs/data/` tenía **cero** archivos rastreados en todo el historial y el primer entregable del perfil (`ECON-ACCOUNTING-01`) sólo pudo entrar con `git add -f`.
Hoy la ruta está re-incluida de forma acotada y **un `git add` normal funciona**: este `README.md` y `GITIGNORE-DOCS-DATA-01.md` entraron así, sin `-f`.
