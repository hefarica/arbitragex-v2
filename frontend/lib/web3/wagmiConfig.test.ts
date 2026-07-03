import { describe, it, expect, afterEach } from "vitest";
import { walletConnectStatus, walletConnectProjectId } from "./wagmiConfig";

// Backs the WalletConnect half of the guard's fail-honest branch (WalletOnboardingGuard reads
// walletConnectStatus() → connectable=false / reason when the project id is absent). Pure env logic;
// getWagmiConfig()/getDefaultConfig are NOT invoked here.

const KEY = "NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID";
const original = process.env[KEY];

afterEach(() => {
  if (original === undefined) delete process.env[KEY];
  else process.env[KEY] = original;
});

describe("walletConnectStatus — fail-honest WalletConnect degradation", () => {
  it("absent project id → unavailable with the honest machine reason", () => {
    delete process.env[KEY];
    expect(walletConnectProjectId()).toBeNull();
    expect(walletConnectStatus()).toEqual({ available: false, reason: "walletconnect_project_id_missing" });
  });

  it("present project id → available, no reason", () => {
    process.env[KEY] = "test_project_id_1234";
    expect(walletConnectProjectId()).toBe("test_project_id_1234");
    expect(walletConnectStatus()).toEqual({ available: true, reason: null });
  });

  it("blank / whitespace-only project id → unavailable (honest, not silently 'available')", () => {
    process.env[KEY] = "   ";
    expect(walletConnectProjectId()).toBeNull();
    expect(walletConnectStatus().available).toBe(false);
  });
});
