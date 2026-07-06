import type { ReadinessGroup, ReadinessItem } from "../types.js";

/**
 * Sentinel factory for items whose underlying system does not yet exist.
 *
 * Returns status="pending" with an explicit reason. When the corresponding
 * feature ships, REPLACE this call with a real verifier in verifiers/<id>.ts
 * and update verifyAll() in verifiers/index.ts.
 *
 * Honesty: a pending item is NEVER reported as green. The flip_blocked
 * summary always reflects reality.
 */
export function pending(args: {
  id: string;
  group: ReadinessGroup;
  label: string;
  doctrine?: string;
  reason?: string;
  now?: () => Date;
}): ReadinessItem {
  const item: ReadinessItem = {
    id: args.id,
    group: args.group,
    label: args.label,
    status: "pending",
    reason: args.reason ?? "feature not yet implemented",
    verified_at: (args.now ?? (() => new Date()))().toISOString(),
  };
  if (args.doctrine !== undefined) item.doctrine = args.doctrine;
  return item;
}
