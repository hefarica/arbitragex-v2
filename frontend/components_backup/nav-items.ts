import {
  ActivityIcon,
  AlertTriangleIcon,
  ClipboardCheckIcon,
  GaugeIcon,
  HouseIcon,
  KeyRoundIcon,
  ListChecksIcon,
  PowerIcon,
  SettingsIcon,
  ShieldCheckIcon,
  SlidersHorizontalIcon,
  type LucideIcon,
  SatelliteDishIcon,
  TrendingUpIcon,
  ZapIcon,
  WalletIcon,
  DatabaseIcon,
  PercentIcon,
  BotIcon,
  RadarIcon,
  FlaskConicalIcon,
  LayersIcon,
  HammerIcon,
  NetworkIcon,
  ServerIcon,
  HeartPulseIcon,
  CoinsIcon,
  CpuIcon,
  BoxIcon,
  FactoryIcon,
  WrenchIcon,
  BookOpenIcon,
  GitBranchIcon,
  RocketIcon,
  LinkIcon,
  LogInIcon,
} from "lucide-react";

// Four operational groups matching the activation flow:
//   pipeline  → everything that MEASURES the system working (observe)
//   control   → everything that PROTECTS / operates capital (risk & control)
//   setup     → everything that CONFIGURES the system (credentials FIRST)
//   omega     → the advanced OMEGA-S5 suite (contracted to the end)
export type NavItem = {
  href: string;
  label: string;
  icon: LucideIcon;
  group: "pipeline" | "control" | "setup" | "omega";
  exact?: boolean;
};

export const NAV_ITEMS: NavItem[] = [
  // ── PIPELINE (observe) ─────────────────────────────────────────────────────
  { href: "/",                     label: "Home",                   icon: HouseIcon,              group: "pipeline", exact: true },
  { href: "/status",               label: "System status",          icon: ActivityIcon,           group: "pipeline" },
  { href: "/opportunities",        label: "Opportunities",          icon: SatelliteDishIcon,      group: "pipeline" },
  { href: "/opportunities/by-strategy", label: "By strategy",      icon: LayersIcon,             group: "pipeline" },
  { href: "/executions",           label: "Executions",             icon: ZapIcon,                group: "pipeline" },
  { href: "/paper/history",        label: "Paper history",          icon: FlaskConicalIcon,       group: "pipeline" },
  { href: "/recon",                label: "Recon & yield",          icon: GaugeIcon,              group: "pipeline" },
  { href: "/operations",           label: "Operations metrics",     icon: TrendingUpIcon,         group: "pipeline" },
  { href: "/route-outcomes",       label: "Route outcomes",         icon: PercentIcon,            group: "pipeline" },
  { href: "/routes/discovery",     label: "Route discovery",        icon: RadarIcon,              group: "pipeline" },
  { href: "/sed",                  label: "SED Pipeline",           icon: ActivityIcon,           group: "pipeline" },
  { href: "/agent-insights",       label: "Agent insights",         icon: BotIcon,                group: "pipeline" },
  { href: "/worker-health",        label: "Worker health",          icon: HeartPulseIcon,         group: "pipeline" },

  // ── RISK & CONTROL ─────────────────────────────────────────────────────────
  { href: "/risk",                 label: "Entropy & alerts",       icon: AlertTriangleIcon,      group: "control" },
  { href: "/killswitch",           label: "Kill-switch",            icon: PowerIcon,              group: "control" },
  { href: "/live-readiness",       label: "Live readiness",         icon: ListChecksIcon,         group: "control" },
  { href: "/operator",             label: "Operator panel",         icon: WrenchIcon,             group: "control" },
  { href: "/operator/self-test",   label: "Self-test center",       icon: ClipboardCheckIcon,     group: "control" },
  { href: "/audit-logs",           label: "Audit logs",             icon: ShieldCheckIcon,        group: "control" },
  { href: "/apex/allocator",       label: "Apex allocator",         icon: CpuIcon,                group: "control" },

  // ── CONFIGURATION (setup — credentials FIRST: the platform is blind without it)
  { href: "/settings/credentials", label: "Credentials",            icon: KeyRoundIcon,           group: "setup" },
  { href: "/config",               label: "Config",                 icon: SettingsIcon,           group: "setup" },
  { href: "/config/trading",       label: "Trading config",         icon: SlidersHorizontalIcon,  group: "setup" },
  { href: "/strategies",           label: "Strategies",             icon: SlidersHorizontalIcon,  group: "setup" },
  { href: "/strategies/forge",     label: "Cartridge Forge",        icon: HammerIcon,             group: "setup" },
  { href: "/onboarding",           label: "Onboarding",             icon: ListChecksIcon,         group: "setup" },
  { href: "/chains",               label: "Chain registry",         icon: LinkIcon,               group: "setup" },
  { href: "/rpcs",                 label: "RPC health",             icon: ServerIcon,             group: "setup" },
  { href: "/pools",                label: "Pool registry",          icon: CoinsIcon,              group: "setup" },
  { href: "/dex-registry",         label: "Exchange registry",      icon: DatabaseIcon,           group: "setup" },
  { href: "/wallets",              label: "Wallets",                icon: WalletIcon,             group: "setup" },
  { href: "/wallet",               label: "Wallet overview",        icon: WalletIcon,             group: "setup" },
  { href: "/deploy-pipeline",      label: "Deploy pipeline",        icon: RocketIcon,             group: "setup" },
  { href: "/admin/topology",       label: "Topology Vault",         icon: SatelliteDishIcon,      group: "setup" },
  { href: "/admin/chains",         label: "Admin chains",           icon: LinkIcon,               group: "setup" },
  { href: "/admin/signin",         label: "Admin sign-in",          icon: LogInIcon,              group: "setup" },
  { href: "/settings",             label: "Settings",               icon: SettingsIcon,           group: "setup" },

  // ── OMEGA-S5 SUITE ─────────────────────────────────────────────────────────
  { href: "/omega-s5/core",        label: "Ω Core decoder",         icon: CpuIcon,                group: "omega" },
  { href: "/omega-s5/crucible",    label: "Ω Crucible tracker",     icon: BoxIcon,                group: "omega" },
  { href: "/omega-s5/factory",     label: "Ω Factory deployments",  icon: FactoryIcon,            group: "omega" },
  { href: "/omega-s5/adapters",    label: "Ω DEX adapters",         icon: NetworkIcon,            group: "omega" },
  { href: "/omega-s5/drift",       label: "Ω Drift detection",      icon: GitBranchIcon,          group: "omega" },
  { href: "/omega-s5/operator",    label: "Ω Operator params",      icon: WrenchIcon,             group: "omega" },
  { href: "/omega-s5/registry",    label: "Ω Entity registries",    icon: BookOpenIcon,           group: "omega" },
  { href: "/omega-s5/wallets",     label: "Ω Wallet topology",      icon: WalletIcon,             group: "omega" },
];
