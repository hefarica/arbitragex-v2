import { describe, it, expect } from "vitest";
import {
  requestProviders,
  resolveWalletAvailability,
  isProtocolConnector,
  type EIP6963ProviderDetail,
} from "./wallet-discovery";
import { installUrlFor, isAllowedInstallUrl, WALLET_INSTALL_REGISTRY, allWalletIds } from "./wallet-registry";

function detail(rdns: string, uuid: string, name = rdns): EIP6963ProviderDetail {
  return { info: { uuid, name, icon: "", rdns }, provider: {} };
}

describe("wallet-registry — install URL allowlist", () => {
  it("installUrlFor returns ONLY the registry's official URL", () => {
    expect(installUrlFor("metamask")).toBe("https://metamask.io/download");
    for (const id of allWalletIds()) {
      const url = installUrlFor(id);
      expect(url).not.toBeNull();
      expect(isAllowedInstallUrl(url!)).toBe(true);
    }
  });

  it("blocks any URL not exactly in the registry", () => {
    expect(isAllowedInstallUrl("https://metamask.io.evil.com/download")).toBe(false);
    expect(isAllowedInstallUrl("https://metamask.io/download/../phish")).toBe(false);
    expect(isAllowedInstallUrl("javascript:alert(1)")).toBe(false);
    expect(isAllowedInstallUrl("http://metamask.io/download")).toBe(false); // non-https
    expect(isAllowedInstallUrl("")).toBe(false);
  });

  it("every registry install URL is https", () => {
    for (const e of WALLET_INSTALL_REGISTRY) {
      expect(e.installUrl.startsWith("https://")).toBe(true);
    }
  });
});

describe("wallet-discovery — EIP-6963 availability (rdns as signal)", () => {
  it("installed when an announced provider's rdns matches the registry allowlist", () => {
    const announced = [detail("io.metamask", "u1", "MetaMask")];
    const r = resolveWalletAvailability("metamask", announced);
    expect(r.installed).toBe(true);
    expect(r.matchedRdns).toBe("io.metamask");
  });

  it("NOT installed when no announced provider matches", () => {
    const r = resolveWalletAvailability("rabby", [detail("io.metamask", "u1")]);
    expect(r.installed).toBe(false);
    expect(r.matchedRdns).toBeNull();
  });

  it("matches an alternate rdns from the allowlist (rabby debank alias)", () => {
    const r = resolveWalletAvailability("rabby", [detail("com.debank.rabby", "u2")]);
    expect(r.installed).toBe(true);
  });

  it("WalletConnect is a protocol connector — never 'installed' via discovery", () => {
    expect(isProtocolConnector("walletconnect")).toBe(true);
    const r = resolveWalletAvailability("walletconnect", [detail("io.metamask", "u1")]);
    expect(r.installed).toBe(false);
  });
});

describe("requestProviders — EIP-6963 collection", () => {
  function mockTarget(announces: EIP6963ProviderDetail[]) {
    const listeners: Record<string, Array<(ev: Event) => void>> = {};
    return {
      addEventListener: (t: string, l: (ev: Event) => void) => {
        (listeners[t] ??= []).push(l);
      },
      removeEventListener: (t: string, l: (ev: Event) => void) => {
        listeners[t] = (listeners[t] ?? []).filter((x) => x !== l);
      },
      dispatchEvent: (ev: Event) => {
        if (ev.type === "eip6963:requestProvider") {
          for (const a of announces) {
            const cev = { type: "eip6963:announceProvider", detail: a } as unknown as Event;
            for (const l of listeners["eip6963:announceProvider"] ?? []) l(cev);
          }
        }
        return true;
      },
    };
  }

  it("collects announced providers and dedupes by uuid", async () => {
    const target = mockTarget([detail("io.metamask", "u1"), detail("io.rabby", "u2"), detail("io.metamask", "u1")]);
    const got = await requestProviders({ target, timeoutMs: 5 });
    expect(got.map((d) => d.info.uuid).sort()).toEqual(["u1", "u2"]);
  });

  it("SSR-safe: no target → empty", async () => {
    expect(await requestProviders({ target: null, timeoutMs: 5 })).toEqual([]);
  });
});
