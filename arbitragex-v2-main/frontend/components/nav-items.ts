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
} from "lucide-react";

export type NavItem = {
  href: string;
  label: string;
  icon: LucideIcon;
  group: "observe" | "control" | "setup";
  exact?: boolean;
};

export const NAV_ITEMS: NavItem[] = [
  // ── OBSERVE ──────────────────────────────────────────────────────────────
  { href: "/",                     label: "Home",                   icon: HouseIcon,              group: "observe", exact: true },
  { href: "/status",               label: "System status",          icon: ActivityIcon,           group: "observe" },
  { href: "/opportunities",        label: "Convergence signals",    icon: SatelliteDishIcon,      group: "observe" },
  { href: "/opportunities/by-strategy", label: "By strategy",      icon: LayersIcon,             group: "observe" },
  { href: "/executions",           label: "Resolutions",            icon: ZapIcon,                group: "observe" },
  { href: "/risk",                 label: "Entropy & alerts",       icon: AlertTriangleIcon,      group: "observe" },
  { href: "/recon",                label: "Recon & yield",          icon: GaugeIcon,              group: "observe" },
  { href: "/operations",           label: "Convergence metrics",    icon: TrendingUpIcon,         group: "observe" },
  { href: "/route-outcomes",       label: "Route outcomes",         icon: PercentIcon,            group: "observe" },
  { href: "/routes/discovery",     label: "Route discovery",        icon: RadarIcon,              group: "observe" },
  { href: "/sed",                  label: "SED Pipeline",           icon: ActivityIcon,           group: "observe" },
  { href: "/agent-insights",       label: "Agent insights",         icon: BotIcon,                group: "observe" },
  { href: "/paper/history",        label: "Paper history",          icon: FlaskConicalIcon,       group: "observe" },
  { href: "/pools",                label: "Pool registry",          icon: CoinsIcon,              group: "observe" },
  { href: "/chains",               label: "Chain registry",         icon: LinkIcon,               group: "observe" },
  { href: "/rpcs",                 label: "RPC health",             icon: ServerIcon,             group: "observe" },
  { href: "/worker-health",        label: "Worker health",          icon: HeartPulseIcon,         group: "observe" },
  { href: "/deploy-pipeline",      label: "Deploy pipeline",        icon: RocketIcon,             group: "observe" },
  { href: "/wallet",               label: "Wallet overview",        icon: WalletIcon,             group: "observe" },

  // ── CONTROL ───────────────────────────────────────────────────────────────
  { href: "/config",               label: "Config",                 icon: SettingsIcon,           group: "control" },
  { href: "/config/trading",       label: "Trading config",         icon: SlidersHorizontalIcon,  group: "control" },
  { href: "/strategies",           label: "Resolution engines",     icon: SlidersHorizontalIcon,  group: "control" },
  { href: "/strategies/forge",     label: "Cartridge Forge",        icon: HammerIcon,             group: "control" },
  { href: "/killswitch",           label: "Kill-switch",            icon: PowerIcon,              group: "control" },
  { href: "/audit-logs",           label: "Audit logs",             icon: ShieldCheckIcon,        group: "control" },
  { href: "/live-readiness",       label: "Live readiness",         icon: ListChecksIcon,         group: "control" },
  { href: "/operator/self-test",   label: "Self-test center",       icon: ClipboardCheckIcon,     group: "control" },
  { href: "/operator",             label: "Operator panel",         icon: WrenchIcon,             group: "control" },
  { href: "/apex/allocator",       label: "Apex allocator",         icon: CpuIcon,                group: "control" },

  // ── OMEGA-S5 SUITE ────────────────────────────────────────────────────────
  { href: "/omega-s5/core",        label: "Ω Core decoder",         icon: CpuIcon,                group: "control" },
  { href: "/omega-s5/crucible",    label: "Ω Crucible tracker",     icon: BoxIcon,                group: "control" },
  { href: "/omega-s5/factory",     label: "Ω Factory deployments",  icon: FactoryIcon,            group: "control" },
  { href: "/omega-s5/adapters",    label: "Ω DEX adapters",         icon: NetworkIcon,            group: "control" },
  { href: "/omega-s5/drift",       label: "Ω Drift detection",      icon: GitBranchIcon,          group: "control" },
  { href: "/omega-s5/operator",    label: "Ω Operator params",      icon: WrenchIcon,             group: "control" },
  { href: "/omega-s5/registry",    label: "Ω Entity registries",    icon: BookOpenIcon,           group: "control" },
  { href: "/omega-s5/wallets",     label: "Ω Wallet topology",      icon: WalletIcon,             group: "control" },

  // ── SETUP ─────────────────────────────────────────────────────────────────
  { href: "/wallets",              label: "Observers",              icon: WalletIcon,             group: "setup" },
  { href: "/dex-registry",         label: "Exchange registry",      icon: DatabaseIcon,           group: "setup" },
  { href: "/onboarding",           label: "Onboarding",             icon: ListChecksIcon,         group: "setup" },
  { href: "/admin/topology",       label: "Topology Vault",         icon: SatelliteDishIcon,      group: "setup" },
  { href: "/admin/chains",         label: "Admin chains",           icon: LinkIcon,               group: "setup" },
  { href: "/settings/credentials", label: "Credentials",            icon: KeyRoundIcon,           group: "setup" },
  { href: "/settings",             label: "Settings",               icon: SettingsIcon,           group: "setup" },
];
