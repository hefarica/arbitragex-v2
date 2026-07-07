import { RadarIcon } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";

export function OpportunitiesEmpty() {
  return (
    <Card className="py-14">
      <CardContent className="flex flex-col items-center gap-3 text-center">
        <div className="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <RadarIcon className="size-6" />
        </div>
        <div className="text-lg font-medium">No opportunities in flight.</div>
        <p className="max-w-md text-sm text-muted-foreground">
          Either the scanner has not connected to an RPC yet, or the market currently
          has nothing worth executing. Check <a className="underline underline-offset-4" href="/status">system status</a>.
        </p>
      </CardContent>
    </Card>
  );
}
