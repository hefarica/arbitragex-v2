"use client";

import { useState } from "react";
import { Trash2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { clearAdminToken } from "@/lib/admin-token";

/**
 * Clears the admin session. V-AT-1: the token is now stored in an httpOnly
 * cookie managed by the edge; this button calls the logout endpoint to
 * clear it server-side. This is the explicit revocation path (e.g. shared
 * workstation, end of shift, suspected leak).
 */
export function ClearAdminTokenButton({
  onCleared,
  className,
}: {
  onCleared?: () => void;
  className?: string;
}) {
  const [cleared, setCleared] = useState(false);

  async function clear() {
    await clearAdminToken();
    setCleared(true);
    onCleared?.();
    // Reset the "cleared" badge after a few seconds so a second click is meaningful.
    setTimeout(() => setCleared(false), 3000);
  }

  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={clear}
      className={className}
      aria-label="Clear admin session"
    >
      <Trash2Icon />
      {cleared ? "cleared" : "Clear admin session"}
    </Button>
  );
}
