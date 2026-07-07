import { ZapIcon } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";

export function ExecutionsEmpty() {
  return (
    <Card className="py-14">
      <CardContent className="flex flex-col items-center gap-3 text-center">
        <div className="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <ZapIcon className="size-6" />
        </div>
        <div className="text-lg font-medium">No executions yet.</div>
        <p className="max-w-md text-sm text-muted-foreground">
          This table fills when simulated opportunities reach the relay submission path.
        </p>
      </CardContent>
    </Card>
  );
}
