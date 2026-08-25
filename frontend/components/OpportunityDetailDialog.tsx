"use client";

import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { OpportunityDetailTabs } from "@/components/opportunities/OpportunityDetailTabs";
import { useRouteTick } from "@/lib/store/omni-store";
import type { OmniOpportunity } from "@/lib/store/types";

// FE-0034 (§37): the dialog is a thin Sheet shell around the tabbed body.
// The legacy inline `OpportunityDetail` mirror type is GONE (§26/§27 — one
// model; the mapper is the only constructor). Consumers pass OmniOpportunity
// directly — the old `as unknown as OpportunityDetail` cast is deleted.

// FE-0037 (§45): the ONLY selector site for the Latency tab — the tick is
// globally hydrated (FE-0008 provider); the tab body stays pure over props.
interface Props {
  opportunity: OmniOpportunity | null;
  onClose: () => void;
}

export function OpportunityDetailDialog({ opportunity, onClose }: Props) {
  const opp = opportunity;
  const tick = useRouteTick();

  return (
    <Sheet open={opp !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-[400px] sm:w-[560px] overflow-y-auto" data-opp-id={opp?.id}>
        <SheetHeader className="mb-6">
          <SheetTitle>Opportunity Detail</SheetTitle>
          <SheetDescription>
            {opp ? `${opp.strategy_kind ?? "—"} · ${opp.status}` : ""}
          </SheetDescription>
        </SheetHeader>

        {opp && (
          <OpportunityDetailTabs
            opp={opp}
            latencyRows={tick?.lat_candidates}
            latencyMeta={tick?.lat_candidates_meta}
          />
        )}
      </SheetContent>
    </Sheet>
  );
}
