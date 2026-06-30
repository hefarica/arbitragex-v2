/**
 * /operator/presets — Operator Risk Presets console.
 *
 * Server component (R1): no data fetch (the preset math is pure and client-side),
 * so this just sets metadata and mounts the client panel inside the shell.
 */
import type { Metadata } from "next";
import { PresetsClient } from "./PresetsClient";

export const metadata: Metadata = {
  // `absolute` overrides any layout title template → exactly this string.
  title: { absolute: "Operator Risk Presets · ArbitrageX" },
  description:
    "Risk-posture presets that resolve the 7 mandatory risk limits from operator capital.",
};

export default function OperatorPresetsPage() {
  return <PresetsClient />;
}
