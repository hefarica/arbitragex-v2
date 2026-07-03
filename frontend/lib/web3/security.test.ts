import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// frontend/lib/web3 -> frontend/
const FRONTEND = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

function collect(dir: string, acc: string[]): void {
  if (!existsSync(dir)) return;
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) {
      if (name === "node_modules" || name === ".next") continue;
      collect(p, acc);
    } else if (/\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name)) {
      acc.push(p);
    }
  }
}

// The wallet signing/connect surface (source only, tests excluded).
function walletSurface(): { path: string; src: string }[] {
  const dirs = [join(FRONTEND, "lib", "web3"), join(FRONTEND, "components", "wallet"), join(FRONTEND, "app", "wallet")];
  const files: string[] = [];
  for (const d of dirs) collect(d, files);
  // wallet + transaction hooks
  const hooks = join(FRONTEND, "hooks");
  if (existsSync(hooks)) {
    for (const n of readdirSync(hooks)) {
      if (/^(useWallet|useTransaction).*\.tsx?$/.test(n) && !/\.test\./.test(n)) files.push(join(hooks, n));
    }
  }
  return files.map((path) => ({ path, src: readFileSync(path, "utf8") }));
}

const surface = walletSurface();
const rel = (p: string) => p.slice(FRONTEND.length + 1).replace(/\\/g, "/");

describe("wallet surface HARD security invariants (static scan)", () => {
  it("has files to scan", () => {
    expect(surface.length).toBeGreaterThan(5);
  });

  it("NO private-key / mnemonic import (viem key-import functions)", () => {
    const hits = surface.filter((f) => /privateKeyToAccount|mnemonicToAccount|HDKey|generatePrivateKey/.test(f.src));
    expect(hits.map((h) => rel(h.path))).toEqual([]);
  });

  it("NO seed-phrase / private-key input field", () => {
    const hits = surface.filter((f) => /type=["']password["']|name=["'][^"']*(seed|mnemonic|private)/i.test(f.src));
    expect(hits.map((h) => rel(h.path))).toEqual([]);
  });

  it("NO frontend broadcast (sendTransaction / writeContract / eth_sendTransaction)", () => {
    const hits = surface.filter((f) => /\beth_sendTransaction\b|\.sendTransaction\s*\(|\bwriteContract\s*\(|useSendTransaction|useWriteContract/.test(f.src));
    expect(hits.map((h) => rel(h.path))).toEqual([]);
  });

  it("NO blind eth_sign (personal_sign / signTypedData for legible content are allowed)", () => {
    const hits = surface.filter((f) => /\beth_sign\b/.test(f.src));
    expect(hits.map((h) => rel(h.path))).toEqual([]);
  });

  it("NO auto-connect / auto-open of the wallet modal in an effect", () => {
    const hits = surface.filter((f) => /autoConnect\s*:\s*true/.test(f.src) || /useEffect\([^;]*openConnectModal\s*\(/.test(f.src));
    expect(hits.map((h) => rel(h.path))).toEqual([]);
  });

  it("the new pure lib/web3 core modules import no wallet/signer runtime (purity)", () => {
    const core = ["modes.ts", "policy.ts", "approval-risk.ts"].map((n) => join(FRONTEND, "lib", "web3", n));
    for (const p of core) {
      const src = readFileSync(p, "utf8");
      expect(/from ["']wagmi["']|from ["']@rainbow-me|window\.ethereum|useAccount|useSignTypedData/.test(src)).toBe(false);
    }
  });
});
