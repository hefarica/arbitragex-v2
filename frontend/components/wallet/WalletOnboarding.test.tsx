// SSR-branch + static-source tests for the Wallet Onboarding Guard surface.
// The frontend test env is `node` (no jsdom), so the two presentational leaf
// components are rendered to static HTML via react-dom/server. The orchestrator
// (WalletOnboardingGuard) wires wagmi hooks + click handlers → covered by
// Playwright E2E, not here. The last block is a static source scan asserting the
// entire wallet surface stays key-less / signer-less / auto-connect-less.

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, it, expect } from "vitest";

import { WalletInstallPrompt } from "./WalletInstallPrompt";
import { WalletConnectConfirm } from "./WalletConnectConfirm";
import { getWalletEntry, installUrlFor, isAllowedInstallUrl } from "@/lib/web3/wallet-registry";

const metamask = getWalletEntry("metamask")!;

describe("WalletInstallPrompt (SSR)", () => {
  it("renders the official download link from the allowlist only", () => {
    const html = renderToStaticMarkup(
      <WalletInstallPrompt entry={metamask} onRedetect={() => {}} onCancel={() => {}} />,
    );
    const url = installUrlFor("metamask")!;
    expect(html).toContain(`href="${url}"`);
    expect(isAllowedInstallUrl(url)).toBe(true);
    // Anchor must open in a new tab with the tab-nabbing guard.
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
    expect(html).toContain("wallet-install-link");
  });

  it("shows the seed-phrase / private-key warning and never a key input", () => {
    const html = renderToStaticMarkup(
      <WalletInstallPrompt entry={metamask} onRedetect={() => {}} onCancel={() => {}} />,
    );
    expect(html).toContain("wallet-seed-warning");
    expect(html.toLowerCase()).toContain("seed phrase");
    expect(html.toLowerCase()).toContain("private key");
    expect(html).not.toContain('type="password"');
    expect(html.toLowerCase()).not.toContain("<input");
  });

  it("renders re-detect + cancel affordances", () => {
    const html = renderToStaticMarkup(
      <WalletInstallPrompt entry={metamask} onRedetect={() => {}} onCancel={() => {}} />,
    );
    expect(html).toContain("wallet-redetect");
    expect(html).toContain("wallet-cancel");
  });
});

describe("WalletConnectConfirm (SSR)", () => {
  it("states the minimal read-only permission contract (no sign/approve/broadcast)", () => {
    const html = renderToStaticMarkup(
      <WalletConnectConfirm entry={metamask} connecting={false} onConnect={() => {}} onCancel={() => {}} />,
    );
    expect(html).toContain("wallet-permissions");
    const lower = html.toLowerCase();
    expect(lower).toContain("dirección"); // read address
    expect(lower).toContain("red"); // read chain
    expect(lower).toContain("no firma");
    expect(lower).toContain("no aprueba");
    expect(lower).toContain("no hace broadcast");
    expect(html).toContain("wallet-connect-mode");
  });

  it("disables the connect button while connecting (no double-fire)", () => {
    const html = renderToStaticMarkup(
      <WalletConnectConfirm entry={metamask} connecting onConnect={() => {}} onCancel={() => {}} />,
    );
    expect(html).toContain("wallet-connect");
    expect(html).toContain("disabled");
    expect(html).toContain('aria-disabled="true"');
  });

  it("fail-honest: when no connector resolves, disables connect + shows the reason (no dead button)", () => {
    const html = renderToStaticMarkup(
      <WalletConnectConfirm
        entry={metamask}
        connecting={false}
        connectable={false}
        reason="no_matching_connector"
        onConnect={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(html).toContain("wallet-connect-unavailable");
    expect(html).toContain("no_matching_connector");
    expect(html).toContain("No disponible");
    expect(html).toContain('aria-disabled="true"');
  });

  it("fail-honest for WalletConnect with no project id surfaces the WC-specific reason", () => {
    const wc = getWalletEntry("walletconnect")!;
    const html = renderToStaticMarkup(
      <WalletConnectConfirm
        entry={wc}
        connecting={false}
        connectable={false}
        reason="walletconnect_project_id_missing"
        onConnect={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(html).toContain("wallet-connect-unavailable");
    expect(html).toContain("walletconnect_project_id_missing");
    expect(html).toContain('aria-disabled="true"');
  });
});

// ── Static source scan: the onboarding-guard surface this PR adds must be key-less,
// signer-less, broadcast-less and auto-connect-less. Scoped to THIS feature's files —
// the doctrine-aligned directory-wide sweep (which correctly permits legible EIP-712
// signTypedData elsewhere and forbids only blind eth_sign) is enforced by the real gate
// automation/tools/wallet-security-gate.sh. Here the invariant is stricter (no signing at
// all) because the onboarding surface never signs anything.
describe("onboarding-guard surface — static security invariants", () => {
  const here = dirname(fileURLToPath(import.meta.url));
  const walletDir = here; // frontend/components/wallet
  const ONBOARDING_FILES = ["WalletOnboardingGuard.tsx", "WalletInstallPrompt.tsx", "WalletConnectConfirm.tsx"];
  const files = ONBOARDING_FILES.map((name) => ({ name, src: readFileSync(join(walletDir, name), "utf8") }));

  // Match USAGE forms (calls / hooks / RPC-method strings), not bare words — otherwise a doc
  // comment enumerating the prohibitions ("no signTypedData, no broadcast") trips its own scan.
  // Same philosophy as the real gate's `writeContract\s*\(` / `.sendTransaction\s*\(`.
  const FORBIDDEN: Array<[string, RegExp]> = [
    ["private key import", /privateKeyToAccount\s*\(|mnemonicToAccount\s*\(|generatePrivateKey\s*\(/],
    ["raw send", /eth_sendTransaction|sendTransaction\s*\(|useSendTransaction/],
    ["raw sign", /useSignTypedData|useSignMessage|signTypedData\s*\(|signMessage\s*\(|['"]eth_sign['"]|personal_sign/],
    ["contract write", /writeContract\s*\(|useWriteContract/],
    ["password input", /type=["']password["']/],
    ["auto-connect flag", /autoConnect\s*:\s*true/],
  ];

  it("scans the guard + both leaf components with real content", () => {
    expect(files).toHaveLength(3);
    for (const { name, src } of files) {
      expect(src.length, `${name} is non-empty`).toBeGreaterThan(100);
    }
  });

  it("contains no key/signer/auto-connect patterns", () => {
    for (const { name, src } of files) {
      for (const [label, re] of FORBIDDEN) {
        expect(re.test(src), `${name} must not contain ${label}`).toBe(false);
      }
    }
  });

  it("never opens openConnectModal inside a useEffect (no auto-popup)", () => {
    for (const { name, src } of files) {
      // Tolerate both `useEffect(` and the namespaced `React.useEffect(` form (the guard uses the latter).
      const effectBlocks = src.match(/(?:React\.)?useEffect\([\s\S]*?\)\s*;/g) ?? [];
      for (const block of effectBlocks) {
        expect(block.includes("openConnectModal"), `${name} useEffect must not open the connect modal`).toBe(false);
        expect(block.includes("connect("), `${name} useEffect must not auto-connect`).toBe(false);
      }
    }
  });
});
