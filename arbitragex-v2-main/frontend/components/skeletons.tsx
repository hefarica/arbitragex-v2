import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export function SkeletonPageHeader() {
  return (
    <div className="mb-8 flex flex-col gap-3 border-b pb-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex-1">
          <Skeleton className="h-8 w-64" />
          <Skeleton className="mt-3 h-4 w-full max-w-2xl" />
          <Skeleton className="mt-1.5 h-4 w-5/6 max-w-xl" />
        </div>
        <Skeleton className="h-9 w-24 shrink-0" />
      </div>
      <div className="flex flex-wrap gap-4">
        <Skeleton className="h-3 w-20" />
        <Skeleton className="h-3 w-32" />
        <Skeleton className="h-3 w-24" />
      </div>
    </div>
  );
}

export function SkeletonKpiGrid({ count = 3 }: { count?: number }) {
  return (
    <div
      className="mb-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6"
      aria-hidden
    >
      {Array.from({ length: count }).map((_, i) => (
        <Card key={i}>
          <CardHeader>
            <Skeleton className="h-4 w-20" />
            <Skeleton className="mt-2 h-8 w-28" />
            <Skeleton className="mt-2 h-3 w-24" />
          </CardHeader>
        </Card>
      ))}
    </div>
  );
}

export function SkeletonTable({
  rows = 8,
  columns = 6,
}: {
  rows?: number;
  columns?: number;
}) {
  return (
    <Card className="py-0" aria-hidden>
      <CardContent className="p-0">
        <div className="border-b px-4 py-3">
          <div className="flex gap-6">
            {Array.from({ length: columns }).map((_, i) => (
              <Skeleton key={i} className="h-3 flex-1" />
            ))}
          </div>
        </div>
        <div className="divide-y">
          {Array.from({ length: rows }).map((_, r) => (
            <div key={r} className="flex gap-6 px-4 py-3">
              {Array.from({ length: columns }).map((_, c) => (
                <Skeleton key={c} className="h-4 flex-1" />
              ))}
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

export function SkeletonSectionHeading() {
  return <Skeleton className="mb-4 mt-8 h-6 w-40" />;
}
