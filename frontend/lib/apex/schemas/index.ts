/**
 * Ω-S5++ C9.∞ APEX · Schema barrel export
 * Punto único de importación. Cualquier código fuera de `apex/` debe
 * importar desde aquí. Prohibido importar archivos internos directamente.
 */
export * from './_primitives';
export * from './chain';
export * from './operational';
export * from './runtime';
export * from './realtime';
// FE-MASTER (2026-08-23): token universe / route-discovery telemetry / quote-base.
export * from './tokens';
export * from './telemetry';
export * from './quote';
// FE-MASTER tramo 2 (2026-08-24): pair intelligence / quotebase catalogs (P5-P7).
export * from './pairs';
export * from './strategies';
export * from './detectors';
// FE-MASTER (2026-08-24): canonical knobs snapshot (FE-0061 / XLS-CANON-01).
export * from './knobs';
