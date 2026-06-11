/**
 * /operator — index of the operator program. PR-1+PR-2 ship the Self-Test
 * Center as its first (and currently only) surface, so this index redirects
 * there. Future operator surfaces hang under this segment.
 */

import { redirect } from "next/navigation";

export default function OperatorIndexPage() {
  redirect("/operator/self-test");
}
