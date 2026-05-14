import {
  ActivityIcon,
  AlertTriangleIcon,
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
} from "lucide-react";

export type NavItem = {
  href: string;
  label: string;
  icon: LucideIcon;
  group: "observe" | "control" | "setup";
  exact?: boolean;
};

export const NAV_ITEMS: NavItem[] = [
  { href: "/",              label: "Home",                   icon: HouseIcon,         group: "observe", exact: true },
  { href: "/status",        label: "System status",          icon: ActivityIcon,      group: "observe" },
  { href: "/opportunities", label: "Convergence signals",    icon: SatelliteDishIcon, group: "observe" },
  { href: "/executions",    label: "Resolutions",            icon: ZapIcon,           group: "observe" },
  { href: "/risk",          label: "Entropy & alerts",       icon: AlertTriangleIcon, group: "observe" },
  { href: "/recon",         label: "Recon & yield",          icon: GaugeIcon,         group: "observe" },
  { href: "/operations",    label: "Convergence metrics",    icon: TrendingUpIcon,    group: "observe" },
  { href: "/sed",           label: "SED Pipeline",           icon: ActivityIcon,      group: "observe" },
  { href: "/config",        label: "Config",                 icon: SettingsIcon,      group: "control" },
  { href: "/strategies",    label: "Resolution engines",     icon: SlidersHorizontalIcon, group: "control" },
  { href: "/killswitch",    label: "Kill-switch",            icon: PowerIcon,         group: "control" },
  { href: "/audit-logs",    label: "Audit logs",             icon: ShieldCheckIcon,   group: "control" },
  { href: "/live-readiness",label: "Live readiness",         icon: ListChecksIcon,    group: "control" },
  { href: "/wallets",        label: "Observers",             icon: WalletIcon,        group: "setup" },
  { href: "/dex-registry",  label: "Exchange registry",      icon: DatabaseIcon,      group: "setup" },
  { href: "/onboarding",    label: "Onboarding",             icon: ListChecksIcon,    group: "setup" },
  { href: "/settings/credentials", label: "Credentials",     icon: KeyRoundIcon,      group: "setup" },
  { href: "/settings",      label: "Settings",               icon: SettingsIcon,      group: "setup" },
];
