/**
 * FE-13 — /settings page.
 *
 * R1 (Mounted Snapshot Pattern): This Server Component renders no data from
 * localStorage. The client-side form reads and writes prefs exclusively inside
 * useEffect / event handlers in SettingsClient.tsx.
 *
 * ZERO MOCKS: No backend API call here. User prefs are client-only, stored in
 * localStorage. If localStorage is absent (SSR, private mode) the defaults are
 * shown — never fabricated values.
 */

import type { Metadata } from "next";
import { PageHeader } from "@/components/page-header";
import { SettingsClient } from "@/app/settings/SettingsClient";

export const metadata: Metadata = {
  title: "Settings | QuantumX",
};

export default function SettingsPage() {
  return (
    <>
      <PageHeader
        title="Settings"
        lede="User preferences persisted in this browser. Changes take effect immediately."
      />
      <SettingsClient />
    </>
  );
}
