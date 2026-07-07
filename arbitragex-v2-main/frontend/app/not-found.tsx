import Link from "next/link";
import { CompassIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";

export default function NotFound() {
  return (
    <>
      <PageHeader
        title="Route not found"
        lede="The path you tried isn't part of the operator console. Use the sidebar to reach a real view."
      />
      <FocusOnMount>
        <Alert variant="warning" className="mb-6">
          <CompassIcon />
          <AlertTitle>404 — no such page</AlertTitle>
          <AlertDescription>
            Every panel in this app maps to a real edge endpoint. If you typed a URL
            that doesn't resolve, that's because the panel doesn't exist yet — not
            because we're hiding it.
          </AlertDescription>
        </Alert>
      </FocusOnMount>

      <div className="flex flex-wrap gap-2">
        <Button asChild>
          <Link href="/">Back to console</Link>
        </Button>
        <Button variant="outline" asChild>
          <Link href="/status">Check system status</Link>
        </Button>
      </div>
    </>
  );
}
