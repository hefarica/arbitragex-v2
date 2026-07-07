# SCAFFOLD APPLY PLAN

## 1. Archivos Nuevos vs Existentes
La mayoría de los archivos del scaffold ya fueron integrados previamente en PR #103 (o están en la rama `feat/omega-scaffold-v2`).
No se encontraron archivos estrictamente nuevos que falten. La carpeta `docs/` y `.github/workflows/` ya contienen todos los archivos del scaffold.

## 2. Diffs y Conflictos (Diff-First)
El intento de copiar `scaffold/*` sobre el directorio raíz resultó en modificaciones a `ci.yml`, `e2e.yml`, `security.yml`, etc.
Dado que la rama actual `feat/omega-scaffold-v2` ya contiene **arreglos específicos de CI/CD** (como el arreglo de `npm workspaces` para los tests de Node, y las correcciones de dependencias), sobrescribir estos archivos con el scaffold crudo **destruiría los fixes introducidos para pasar los tests**.

## 3. Resolución Diff-First
- Se revirtió la sobrescritura ciega (`git restore .`).
- El estado actual del repo respeta la estructura propuesta por OMEGA pero incluye las enmiendas necesarias para que el CI funcione en la realidad (por doctrina "El remoto manda. GitHub Actions manda").
- Se considerará la Fase 4 como "Aplicada con retención de fixes CI".

## 4. Rollback
Cualquier sobrescritura accidental se corrige con `git restore .` para mantener el estado seguro de la rama.
