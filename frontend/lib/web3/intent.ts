// TransactionIntent — the EIP-712 typed object the operator signs INSTEAD of opaque calldata.
//
// The user never signs raw calldata. They sign this legible, bounded struct. `calldataHash` binds
// the intent to the exact calldata the backend simulated; the executor only proceeds if the
// on-chain/relayed calldata hashes to `calldataHash` (checked by the Policy Engine gate
// `calldata_hash_matches_sim`). This module is PURE: it builds the typed struct, its EIP-712
// domain/types, and content hashes. It NEVER signs, sends, or holds a key.

import { keccak256, stringToHex, maxUint256, type Hex, type Address, type TypedDataDomain } from "viem";

export const ARBX_INTENT_VERSION = "ARBX_INTENT_V1" as const;

export interface TransactionIntent {
  version: typeof ARBX_INTENT_VERSION;
  chainId: number;
  wallet: Address;
  executor: Address;
  routeHash: Hex;
  calldataHash: Hex;
  simulationId: string;
  tokenIn: Address;
  tokenOut: Address;
  amountIn: string; // decimal wei string
  maxGasWei: string; // decimal wei string
  maxSlippageBps: number;
  minProfitUsd: string; // decimal string
  deadline: number; // unix seconds
  nonce: string; // decimal string
  policyId: string;
  riskScore: number;
}

// EIP-712 types for the legible signature. Every field is a named, typed value the wallet renders.
export const ARBX_INTENT_EIP712_TYPES = {
  TransactionIntent: [
    { name: "version", type: "string" },
    { name: "chainId", type: "uint256" },
    { name: "wallet", type: "address" },
    { name: "executor", type: "address" },
    { name: "routeHash", type: "bytes32" },
    { name: "calldataHash", type: "bytes32" },
    { name: "simulationId", type: "string" },
    { name: "tokenIn", type: "address" },
    { name: "tokenOut", type: "address" },
    { name: "amountIn", type: "uint256" },
    { name: "maxGasWei", type: "uint256" },
    { name: "maxSlippageBps", type: "uint256" },
    { name: "minProfitUsd", type: "string" },
    { name: "deadline", type: "uint256" },
    { name: "nonce", type: "uint256" },
    { name: "policyId", type: "string" },
    { name: "riskScore", type: "uint256" },
  ],
} as const;

export function intentDomain(chainId: number, verifyingContract: Address): TypedDataDomain {
  return { name: "ArbitrageX Intent", version: "1", chainId, verifyingContract };
}

// Full EIP-712 payload ready for signTypedData. Callers MUST pass this (never raw calldata) to any
// signer. The primaryType is the named struct, so wallets show a human-readable field list.
export function buildIntentTypedData(intent: TransactionIntent) {
  return {
    domain: intentDomain(intent.chainId, intent.executor),
    types: ARBX_INTENT_EIP712_TYPES,
    primaryType: "TransactionIntent" as const,
    message: intent,
  };
}

export function hashCalldata(calldata: Hex): Hex {
  return keccak256(calldata);
}

export function hashRoute(parts: string[]): Hex {
  return keccak256(stringToHex(parts.join("|")));
}

const HEX32 = /^0x[0-9a-fA-F]{64}$/;
const ADDR = /^0x[0-9a-fA-F]{40}$/;

// A field-by-field legibility/bounds check. Returns the list of problems (empty = legible & bounded).
// The signing surface must refuse to sign an intent that fails this — this is what makes it
// "not blind": every field is present, typed, and within basic bounds before a preview is shown.
export function validateIntent(i: TransactionIntent, nowSec: number): string[] {
  const errs: string[] = [];
  if (i.version !== ARBX_INTENT_VERSION) errs.push("version mismatch");
  if (!Number.isInteger(i.chainId) || i.chainId <= 0) errs.push("invalid chainId");
  if (!ADDR.test(i.wallet)) errs.push("invalid wallet address");
  if (!ADDR.test(i.executor)) errs.push("invalid executor address");
  if (!ADDR.test(i.tokenIn) || !ADDR.test(i.tokenOut)) errs.push("invalid token address");
  if (!HEX32.test(i.routeHash)) errs.push("invalid routeHash (must be bytes32)");
  if (!HEX32.test(i.calldataHash)) errs.push("missing/invalid calldataHash — cannot bind to simulation");
  if (!i.simulationId) errs.push("missing simulationId");
  if (!isPositiveIntString(i.amountIn)) errs.push("invalid amountIn");
  if (!isPositiveIntString(i.maxGasWei)) errs.push("invalid maxGasWei");
  if (!Number.isFinite(i.maxSlippageBps) || i.maxSlippageBps < 0 || i.maxSlippageBps > 10_000) errs.push("slippage out of range (0..10000 bps)");
  if (!isNonNegIntString(i.nonce)) errs.push("invalid nonce");
  if (!Number.isInteger(i.deadline) || i.deadline <= nowSec) errs.push("deadline missing or already expired");
  if (!i.policyId) errs.push("missing policyId");
  if (!Number.isFinite(i.riskScore)) errs.push("invalid riskScore");
  return errs;
}

export interface IntentRow {
  label: string;
  value: string;
}

// Human-readable rows for the preview UI. No opaque bytes are ever presented as the thing being
// signed — the calldataHash is shown explicitly as a binding hash, not as executable content.
export function legibleIntentPreview(i: TransactionIntent): IntentRow[] {
  return [
    { label: "Version", value: i.version },
    { label: "Chain", value: String(i.chainId) },
    { label: "Wallet", value: i.wallet },
    { label: "Executor", value: i.executor },
    { label: "Token in", value: i.tokenIn },
    { label: "Token out", value: i.tokenOut },
    { label: "Amount in (wei)", value: i.amountIn },
    { label: "Max gas (wei)", value: i.maxGasWei },
    { label: "Max slippage (bps)", value: String(i.maxSlippageBps) },
    { label: "Min profit (USD)", value: i.minProfitUsd },
    { label: "Deadline (unix)", value: String(i.deadline) },
    { label: "Nonce", value: i.nonce },
    { label: "Policy id", value: i.policyId },
    { label: "Risk score", value: String(i.riskScore) },
    { label: "Route hash", value: i.routeHash },
    { label: "Calldata hash (bound to sim)", value: i.calldataHash },
    { label: "Simulation id", value: i.simulationId },
  ];
}

export const UNLIMITED_THRESHOLD = 2n ** 255n; // amounts >= this are treated as effectively unlimited
export function isUnlimitedWei(amount: string): boolean {
  try {
    return BigInt(amount) >= UNLIMITED_THRESHOLD;
  } catch {
    return false;
  }
}
export const MAX_UINT256 = maxUint256;

function isPositiveIntString(s: string): boolean {
  try {
    return /^\d+$/.test(s) && BigInt(s) > 0n;
  } catch {
    return false;
  }
}
function isNonNegIntString(s: string): boolean {
  try {
    return /^\d+$/.test(s) && BigInt(s) >= 0n;
  } catch {
    return false;
  }
}
