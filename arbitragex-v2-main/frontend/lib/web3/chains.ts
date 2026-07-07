/**
 * web3/chains — maps the operator's doctrinal chain catalog to viem `Chain`
 * objects for wagmi. Single source of truth for the EVM mainnets is
 * `frontend/lib/chains.ts` (DOCTRINAL_CHAINS); here we resolve those chain IDs
 * to viem's canonical chain definitions.
 *
 * No-hardcode (RULE 00): the *enabled* set and the default chain come from env
 * (NEXT_PUBLIC_SUPPORTED_CHAINS / NEXT_PUBLIC_DEFAULT_CHAIN_ID). Chain IDs and
 * viem chain metadata are protocol-level constants (not operator productive
 * data), matching the doctrinal-static exception already used in lib/chains.ts.
 */

import { arbitrum, base, bsc, mainnet, optimism, polygon } from "wagmi/chains";
import type { Chain } from "viem";

import { DOCTRINAL_CHAINS } from "@/lib/chains";

/** chain_id → viem Chain for the doctrinal EVM mainnets. */
const VIEM_BY_ID: Record<number, Chain> = {
  [mainnet.id]: mainnet,
  [arbitrum.id]: arbitrum,
  [optimism.id]: optimism,
  [base.id]: base,
  [polygon.id]: polygon,
  [bsc.id]: bsc,
};

/** The full doctrinal viem chain set (all 6 EVM mainnets), in doctrinal order. */
export const DOCTRINAL_VIEM_CHAINS: readonly Chain[] = DOCTRINAL_CHAINS
  .map((c) => VIEM_BY_ID[c.chain_id])
  .filter((c): c is Chain => Boolean(c));

/**
 * Parse NEXT_PUBLIC_SUPPORTED_CHAINS ("1,42161,8453") into a viem chain list.
 * Falls back to the full doctrinal set when the env is absent/empty — the UI
 * must never blank, and these are protocol constants, not productive secrets.
 */
export function resolveSupportedChains(): readonly Chain[] {
  const raw = (process.env.NEXT_PUBLIC_SUPPORTED_CHAINS ?? "").trim();
  if (!raw) return DOCTRINAL_VIEM_CHAINS;
  const ids = raw
    .split(",")
    .map((s) => parseInt(s.trim(), 10))
    .filter((n) => Number.isFinite(n));
  const picked = ids.map((id) => VIEM_BY_ID[id]).filter((c): c is Chain => Boolean(c));
  return picked.length > 0 ? picked : DOCTRINAL_VIEM_CHAINS;
}

/** The operator-configured default chain id (NEXT_PUBLIC_DEFAULT_CHAIN_ID), or 1. */
export function resolveDefaultChainId(): number {
  const raw = (process.env.NEXT_PUBLIC_DEFAULT_CHAIN_ID ?? "").trim();
  const n = parseInt(raw, 10);
  return Number.isFinite(n) && n > 0 ? n : DOCTRINAL_VIEM_CHAINS[0]?.id ?? 1;
}

/** True when chain_id is in the operator's supported set. */
export function isSupportedChain(chainId: number | undefined): boolean {
  if (chainId === undefined) return false;
  return resolveSupportedChains().some((c) => c.id === chainId);
}
