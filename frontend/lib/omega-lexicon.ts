/**
 * OMEGA LEXICON — Sanitización léxica para interfaz de operador
 *
 * Propósito: Reemplazar terminología sensible ("bot", "snipe", "arbitrage")
 * con términos neutros para el operador del dashboard.
 *
 * Ghost Protocol: Esta sanitización solo afecta lo RENDERIZADO en UI.
 * Los datos internos, logs técnicos, y comunicación backend-backend
 * mantienen su nomenclatura original.
 *
 * R8 Fail-honest: la sustitución es 1:1 transparente, nunca altera semántica.
 */

/** Tabla de sustitución: término prohibido → término aceptado */
export const OMEGA_SUBSTITUTIONS: Readonly<Record<string, string>> = {
  // Términos prohibidos y sus sustitutos
  "bot": "worker",
  "bots": "workers",
  "snipe": "capture",
  "snipes": "captures",
  "sniping": "capturing",
  "sniped": "captured",
  "arbitrage": "cross-venue opportunity",
  "arbitrages": "cross-venue opportunities",
  "arb": "opportunity",
  "arbs": "opportunities",
  "mev": "network value",
  "mev extraction": "value capture",
  "sandwich": "reordering pattern",
  "frontrun": "pre-positioning",
  "backrun": "follow-through",
};

/** Flag para activar/desactivar sanitización globalmente */
export const LEXICON_ENABLED = true;

/**
 * Sanitiza una cadena de texto reemplazando términos prohibidos.
 * @param text Texto original (puede contener múltiples ocurrencias)
 * @returns Texto sanitizado, o el original si LEXICON_ENABLED es false
 */
export function sanitizeForDisplay(text: string): string {
  if (!LEXICON_ENABLED || !text) return text;

  let result = text;
  // Ordenar por longitud descendente para evitar sustituciones parciales
  // (ej: "arbitrage" debe reemplazarse antes que "arb")
  const entries = Object.entries(OMEGA_SUBSTITUTIONS).sort(
    ([a], [b]) => b.length - a.length
  );

  for (const [prohibited, substitute] of entries) {
    // Reemplazo case-insensitive con preservación de mayúsculas iniciales
    const regex = new RegExp(`\\b${prohibited}\\b`, "gi");
    result = result.replace(regex, (match) => {
      // Preservar capitalización: si el original empezaba con mayúscula
      if (match[0] === match[0].toUpperCase()) {
        return substitute[0].toUpperCase() + substitute.slice(1);
      }
      return substitute;
    });
  }

  return result;
}

/**
 * Sanitiza valores de texto dentro de un objeto, solo en campos específicos.
 * @param obj Objeto original (no mutado)
 * @param fields Nombres de campos a sanitizar
 * @returns Nuevo objeto con campos sanitizados
 */
export function sanitizeObject<T extends Record<string, unknown>>(
  obj: T,
  fields: string[]
): T {
  if (!LEXICON_ENABLED) return obj;

  const result = { ...obj };
  for (const field of fields) {
    if (typeof result[field] === "string") {
      (result as Record<string, unknown>)[field] = sanitizeForDisplay(
        result[field] as string
      );
    }
  }
  return result;
}

/**
 * Sanitiza un array de objetos aplicando sanitizeObject a cada elemento.
 */
export function sanitizeArray<T extends Record<string, unknown>>(
  arr: T[],
  fields: string[]
): T[] {
  if (!LEXICON_ENABLED) return arr;
  return arr.map((item) => sanitizeObject(item, fields));
}

/**
 * Verifica si un texto contiene términos prohibidos (útil para testing).
 */
export function containsProhibitedTerms(text: string): boolean {
  const entries = Object.keys(OMEGA_SUBSTITUTIONS);
  for (const term of entries) {
    const regex = new RegExp(`\\b${term}\\b`, "i");
    if (regex.test(text)) return true;
  }
  return false;
}
