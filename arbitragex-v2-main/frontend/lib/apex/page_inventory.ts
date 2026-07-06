/**
 * Ω-S5++ C9.∞ APEX · Page Inventory
 * -------------------------------------------------------------------------
 * Catálogo tipado de las 37 rutas reales del frontend v5.
 * Cada entrada es la proyección del operador Ĥ_frontend sobre el espacio
 * de Hilbert de la UI: H_DOM = span{|page_i⟩}.
 *
 * REGLA: este archivo NO modifica las rutas existentes. Sólo las cataloga
 * para auditoría página-por-página (Mirror Law extendida C9).
 */

export type PageCategory =
  | 'public'
  | 'onboarding'
  | 'operations'
  | 'admin'
  | 'omega-s5-core'
  | 'omega-s5-registry'
  | 'omega-s5-operator';

export type OperatorRole = 'observer' | 'steward' | 'sovereign';

export interface PageDescriptor {
  readonly route: string;
  readonly file: string;
  readonly category: PageCategory;
  readonly purpose: string;
  readonly requiresAuth: boolean;
  readonly requiredRole: OperatorRole | null;
  readonly hasOperatorGate: boolean;
  readonly hasMutations: boolean;
  readonly hasRealtimeStream: boolean;
  readonly criticalActions: ReadonlyArray<string>;
}

/**
 * Inventario inmutable. La cardinalidad |PAGES| = 37 es invariante de norma.
 * Si se agrega/quita ruta sin actualizar este array, INV-1 (Norm Preservation)
 * se rompe y la auditoría falla.
 */
export const PAGES: ReadonlyArray<PageDescriptor> = [
  // ── Public / Home ───────────────────────────────────────────────────
  {
    route: '/',
    file: 'frontend/app/page.tsx',
    category: 'public',
    purpose: 'Landing principal con readiness summary y telemetría base',
    requiresAuth: false,
    requiredRole: null,
    hasOperatorGate: false,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  // ── Onboarding (5 fases) ─────────────────────────────────────────────
  {
    route: '/onboarding',
    file: 'frontend/app/onboarding/page.tsx',
    category: 'onboarding',
    purpose: 'Hub de onboarding 5 fases con progreso persistido',
    requiresAuth: false,
    requiredRole: null,
    hasOperatorGate: false,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/onboarding/1-init',
    file: 'frontend/app/onboarding/1-init/page.tsx',
    category: 'onboarding',
    purpose: 'Fase 1: inicialización operador + admin token',
    requiresAuth: false,
    requiredRole: null,
    hasOperatorGate: false,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['register_operator', 'store_admin_token'],
  },
  {
    route: '/onboarding/2-connect',
    file: 'frontend/app/onboarding/2-connect/page.tsx',
    category: 'onboarding',
    purpose: 'Fase 2: conectar RPCs/WS y verificar healthcheck',
    requiresAuth: false,
    requiredRole: null,
    hasOperatorGate: false,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['register_rpc', 'verify_rpc_health'],
  },
  {
    route: '/onboarding/3-advanced',
    file: 'frontend/app/onboarding/3-advanced/page.tsx',
    category: 'onboarding',
    purpose: 'Fase 3: configuración avanzada (relays, risk gates)',
    requiresAuth: false,
    requiredRole: null,
    hasOperatorGate: false,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['configure_risk_gates', 'configure_capital_gates'],
  },
  {
    route: '/onboarding/4-testing',
    file: 'frontend/app/onboarding/4-testing/page.tsx',
    category: 'onboarding',
    purpose: 'Fase 4: testing en testnets (Crucible inicial)',
    requiresAuth: false,
    requiredRole: null,
    hasOperatorGate: false,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/onboarding/5-production',
    file: 'frontend/app/onboarding/5-production/page.tsx',
    category: 'onboarding',
    purpose: 'Fase 5: gate de producción (Crucible ≥72h)',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: true,
    criticalActions: ['promote_mainnet'],
  },
  // ── Operations ───────────────────────────────────────────────────────
  {
    route: '/operations',
    file: 'frontend/app/operations/page.tsx',
    category: 'operations',
    purpose: 'Dashboard operativo principal',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/opportunities',
    file: 'frontend/app/opportunities/page.tsx',
    category: 'operations',
    purpose: 'Stream live de oportunidades (snapshot+delta)',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/executions',
    file: 'frontend/app/executions/page.tsx',
    category: 'operations',
    purpose: 'Histórico de ejecuciones con export CSV',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/recon',
    file: 'frontend/app/recon/page.tsx',
    category: 'operations',
    purpose: 'Reconciliación P&L (pnl_engine output)',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/audit-logs',
    file: 'frontend/app/audit-logs/page.tsx',
    category: 'operations',
    purpose: 'Audit trail completo (append-only)',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/live-readiness',
    file: 'frontend/app/live-readiness/page.tsx',
    category: 'operations',
    purpose: 'GO/NO-GO panel + blockers desglosados',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/status',
    file: 'frontend/app/status/page.tsx',
    category: 'operations',
    purpose: 'Estado global del sistema (heartbeat)',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/worker-health',
    file: 'frontend/app/worker-health/page.tsx',
    category: 'operations',
    purpose: 'Salud de workers searcher-rs/spine/recon',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  // ── Registries (catálogos) ──────────────────────────────────────────
  {
    route: '/chains',
    file: 'frontend/app/chains/page.tsx',
    category: 'admin',
    purpose: 'Vista pública de catálogo de cadenas',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/dex-registry',
    file: 'frontend/app/dex-registry/page.tsx',
    category: 'admin',
    purpose: 'Catálogo DEX por cadena',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['add_dex', 'edit_dex', 'disable_dex'],
  },
  {
    route: '/pools',
    file: 'frontend/app/pools/page.tsx',
    category: 'admin',
    purpose: 'Catálogo de pools',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['add_pool', 'disable_pool'],
  },
  {
    route: '/wallets',
    file: 'frontend/app/wallets/page.tsx',
    category: 'admin',
    purpose: 'Catálogo de wallets (Ghost: balance≡0)',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['register_wallet', 'disable_wallet'],
  },
  {
    route: '/rpcs',
    file: 'frontend/app/rpcs/page.tsx',
    category: 'admin',
    purpose: 'Catálogo de RPCs por cadena',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['add_rpc', 'verify_rpc', 'disable_rpc'],
  },
  {
    route: '/strategies',
    file: 'frontend/app/strategies/page.tsx',
    category: 'admin',
    purpose: 'Catálogo de estrategias topológicas',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['enable_strategy', 'disable_strategy'],
  },
  {
    route: '/risk',
    file: 'frontend/app/risk/page.tsx',
    category: 'admin',
    purpose: 'Risk gates + circuit breakers',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['set_risk_gate', 'set_circuit_breaker'],
  },
  {
    route: '/sed',
    file: 'frontend/app/sed/page.tsx',
    category: 'admin',
    purpose: 'SED (signed execution doctrine) core',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/killswitch',
    file: 'frontend/app/killswitch/page.tsx',
    category: 'admin',
    purpose: 'Kill switch global (parada de emergencia)',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['engage_killswitch', 'release_killswitch'],
  },
  {
    route: '/config',
    file: 'frontend/app/config/page.tsx',
    category: 'admin',
    purpose: 'Configuración general',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['update_config'],
  },
  {
    route: '/config/trading',
    file: 'frontend/app/config/trading/page.tsx',
    category: 'admin',
    purpose: 'Configuración de trading (paper/live, gates)',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['set_paper_mode', 'set_capital_cap'],
  },
  {
    route: '/settings',
    file: 'frontend/app/settings/page.tsx',
    category: 'admin',
    purpose: 'Settings de operador',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['update_preferences'],
  },
  {
    route: '/settings/credentials',
    file: 'frontend/app/settings/credentials/page.tsx',
    category: 'admin',
    purpose: 'Credenciales (read-only, rotación vía CLI)',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/admin/chains',
    file: 'frontend/app/admin/chains/page.tsx',
    category: 'admin',
    purpose: 'Admin de cadenas (CRUD completo)',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['add_chain', 'edit_chain', 'toggle_chain'],
  },
  // ── Ω-S5 routes (9) ──────────────────────────────────────────────────
  {
    route: '/omega-s5/core',
    file: 'frontend/app/omega-s5/core/page.tsx',
    category: 'omega-s5-core',
    purpose: 'Núcleo Ω-S5: manifest + readiness',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/omega-s5/adapters',
    file: 'frontend/app/omega-s5/adapters/page.tsx',
    category: 'omega-s5-core',
    purpose: 'Adapters DEX + flashloan',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/omega-s5/crucible',
    file: 'frontend/app/omega-s5/crucible/page.tsx',
    category: 'omega-s5-core',
    purpose: 'Crucible runs (72h gate)',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/omega-s5/drift',
    file: 'frontend/app/omega-s5/drift/page.tsx',
    category: 'omega-s5-core',
    purpose: 'Drift detection (config_hash mismatch)',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: true,
    criticalActions: [],
  },
  {
    route: '/omega-s5/factory',
    file: 'frontend/app/omega-s5/factory/page.tsx',
    category: 'omega-s5-core',
    purpose: 'DeterministicFactory + WalletTopology',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/omega-s5/operator',
    file: 'frontend/app/omega-s5/operator/page.tsx',
    category: 'omega-s5-operator',
    purpose: 'Panel Operator Sovereignty (12 registries)',
    requiresAuth: true,
    requiredRole: 'sovereign',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['promote_role', 'demote_role', 'reset_sovereign'],
  },
  {
    route: '/omega-s5/wallets',
    file: 'frontend/app/omega-s5/wallets/page.tsx',
    category: 'omega-s5-core',
    purpose: 'Wallet topology (Ghost: balance≡0)',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/omega-s5/registry',
    file: 'frontend/app/omega-s5/registry/page.tsx',
    category: 'omega-s5-registry',
    purpose: 'Hub de 12 registries × 7 capacidades',
    requiresAuth: true,
    requiredRole: 'observer',
    hasOperatorGate: true,
    hasMutations: false,
    hasRealtimeStream: false,
    criticalActions: [],
  },
  {
    route: '/omega-s5/registry/[entity]',
    file: 'frontend/app/omega-s5/registry/[entity]/page.tsx',
    category: 'omega-s5-registry',
    purpose: 'Vista dinámica por entidad (chain, dex, pool, ...)',
    requiresAuth: true,
    requiredRole: 'steward',
    hasOperatorGate: true,
    hasMutations: true,
    hasRealtimeStream: false,
    criticalActions: ['crud_entity'],
  },
] as const;

/**
 * Invariante de cardinalidad — INV-1 (Norm Preservation).
 * Si esta constante difiere de PAGES.length, la auditoría falla.
 */
export const EXPECTED_PAGE_COUNT = 37 as const;

/**
 * Discriminantes para query/group-by tipados (no usar string libre).
 */
export const CATEGORIES: ReadonlyArray<PageCategory> = [
  'public',
  'onboarding',
  'operations',
  'admin',
  'omega-s5-core',
  'omega-s5-registry',
  'omega-s5-operator',
] as const;

export const ROLES: ReadonlyArray<OperatorRole> = ['observer', 'steward', 'sovereign'] as const;

/**
 * Helper: páginas por categoría (tipado preservado).
 */
export function pagesByCategory(category: PageCategory): ReadonlyArray<PageDescriptor> {
  return PAGES.filter((p) => p.category === category);
}

/**
 * Helper: páginas con acciones críticas (mutations + sovereign).
 */
export function criticalPages(): ReadonlyArray<PageDescriptor> {
  return PAGES.filter((p) => p.hasMutations && p.requiredRole === 'sovereign');
}
