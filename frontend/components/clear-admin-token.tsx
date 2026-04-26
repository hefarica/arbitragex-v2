"use client";

import { useState } from "react";
import { Trash2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { clearAdminToken } from "@/lib/admin-token";

/**
 * Clears the locally-stored admin token. The token is held in localStorage
 * so the operator doesn't have to paste it on every action; this button is
 * the explicit revocation path (e.g. shared workstation, end of shift,
 * suspected leak).
 */
export function ClearAdminTokenButton({
  onCleared,
  className,
}: {
  onCleared?: () => void;
  className?: string;
}) {
  const [cleared, setCleared] = useState(false);

  function clear() {
    clearAdminToken();
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
      aria-label="Clear admin token from local storage"
    >
      <Trash2Icon />
      {cleared ? "cleared" : "Clear admin token"}
    </Button>
  );
}
