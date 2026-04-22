import {
  ActivityIcon,
  AlertTriangleIcon,
  GaugeIcon,
  HouseIcon,
  ListChecksIcon,
  PowerIcon,
  SettingsIcon,
  type LucideIcon,
  SatelliteDishIcon,
  ZapIcon,
} from "lucide-react";

export type NavItem = {
  href: string;
  label: string;
  icon: LucideIcon;
  group: "observe" | "control" | "setup";
  exact?: boolean;
};

export const NAV_ITEMS: NavItem[] = [
  { href: "/",              label: "Home",             icon: HouseIcon,         group: "observe", exact: true },
  { href: "/status",        label: "System status",    icon: ActivityIcon,      group: "observe" },
  { href: "/opportunities", label: "Opportunities",    icon: SatelliteDishIcon, group: "observe" },
  { href: "/executions",    label: "Executions",       icon: ZapIcon,           group: "observe" },
  { href: "/risk",          label: "Risk & alerts",    icon: AlertTriangleIcon, group: "observe" },
  { href: "/recon",         label: "Recon & PnL",      icon: GaugeIcon,         group: "observe" },
  { href: "/config",        label: "Config",           icon: SettingsIcon,      group: "control" },
  { href: "/killswitch",    label: "Kill-switch",      icon: PowerIcon,         group: "control" },
  { href: "/onboarding",    label: "Onboarding",       icon: ListChecksIcon,    group: "setup" },
];
