# Skill: Prevención de Reincidencia Operativa (Anti-Reincidencia)

## 1. Nombre de la skill
anti_reincidencia_operativa

## 2. Cuándo debe activarse
Esta skill se activa de manera automática e incondicional CADA VEZ que el agente enfrente la corrección de un bug crítico, una caída en producción (VPS) o un error arquitectónico severo (como el React Hydration Mismatch #425). Su propósito es garantizar que un problema resuelto jamás vuelva a repetirse.

## 3. Qué errores o comportamientos debe prevenir
- El "ciclo de la muerte": arreglar un síntoma sin entender ni documentar la causa raíz.
- Modificar archivos en caliente sin validar que el build o la compilación sea exitosa.
- Desplegar en producción sin forzar la invalidación de caché (ej. `docker build` sin `--no-cache`).
- Olvidar el paso de variables de entorno (ej. `--env-file .env` faltante en el pipeline de despliegue).
- Dejar conocimiento crítico en la memoria volátil del chat en lugar de documentarlo permanentemente.

## 4. Causa raíz del problema detectado (Ejemplo base: Hydration Cascade #425)
- **Causa arquitectónica:** Componentes marcados como `"use client"` evaluaban código no determinista (`Date.now()`, `Math.random()`, `getApiBaseUrl()` que leía entornos diferentes) directamente en el primer ciclo de renderizado.
- **Causa operacional:** El despliegue de correcciones al VPS no invalidó la caché de Docker (los _chunks_ de Next.js se servían obsoletos) y omitió el paso del archivo `.env` durante el build, provocando que variables críticas cayeran en `localhost` en el entorno productivo.

## 5. Reglas obligatorias para que no vuelva a pasar
1. **Regla de Cero Mismatch (React):** Toda página SSR en Next.js App Router debe entregar un snapshot inicial determinístico. El cliente inicia exactamente con ese snapshot y activa comportamiento dinámico solo dentro de `useEffect` (después de hidratar).
2. **Regla de Cache-Busting (Despliegues):** Todo despliegue correctivo a producción que involucre cambios de código JS/TS compilado DEBE llevar `--no-cache` en Docker Compose.
3. **Regla de Entorno Constante:** Al compilar un contenedor que embebe variables estáticas (como Next.js con `NEXT_PUBLIC_`), el comando de build DEBE incluir explícitamente `--env-file .env` para garantizar que la imagen final nazca con las variables de producción correctas.

## 6. Procedimiento paso a paso antes de actuar
1. **Pausar y Analizar:** Leer los logs completos. No proponer código sin entender el flujo completo.
2. **Auditar el Código Actual:** Buscar componentes similares que puedan sufrir el mismo problema latente.
3. **Aplicar la Corrección Estructural:** No parchear. Refactorizar siguiendo los patrones inmutables del proyecto.
4. **Validación Local Simulada:** Ejecutar `npm run build` o `cargo check` antes del despliegue.
5. **Despliegue Asertivo:** Ejecutar el flujo completo en el VPS con regeneración de caché y paso de credenciales correcto.

## 7. Validaciones obligatorias
- **Antes:** Comprobar si el bug es de estado, compilación o hidratación reproduciendo el problema con `browser_subagent` o inspeccionando los logs de red.
- **Durante:** Validar que la compilación (build) sea libre de errores.
- **Después:** Ejecutar verificación funcional mediante subagente de navegación o curl a los puertos locales. Comprobar que el error ha desaparecido por completo del log del cliente y del servidor.

## 8. Forma correcta de documentar nuevos aprendizajes
Todo aprendizaje estructural o "gotcha" descubierto en un fixing loop debe documentarse inmediatamente creando un archivo `.md` dentro de `.agents/skills/` y una bitácora en `.agents/memory/anti_reincidencia.md`. La documentación debe ser pragmática: "Qué pasó -> Por qué pasó -> Regla inmutable".

## 9. Checklist final antes de responder al usuario
- [ ] ¿Entendí la causa raíz del error o solo tapé el síntoma?
- [ ] ¿La solución aportada viola alguna regla inmutable preexistente?
- [ ] ¿El entorno productivo fue reconstruido con `no-cache` y variables inyectadas?
- [ ] ¿Actualicé la memoria persistente (`anti_reincidencia.md`) con este incidente?
- [ ] ¿Corroboré visual o funcionalmente (mediante subagente de navegador o API local) que la solución está operando estable en el VPS?
