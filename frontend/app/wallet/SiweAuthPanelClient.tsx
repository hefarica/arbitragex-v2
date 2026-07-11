"use client";

import dynamic from "next/dynamic";

const SiweAuthPanelDynamic = dynamic(
  () => import("@/app/wallet/SiweAuthPanel").then((m) => m.SiweAuthPanel),
  { ssr: false }
);

export function SiweAuthPanelClient() {
  return <SiweAuthPanelDynamic />;
}
