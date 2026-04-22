import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { SkeletonPageHeader, SkeletonSectionHeading, SkeletonTable } from "@/components/skeletons";

export default function Loading() {
  return (
    <>
      <SkeletonPageHeader />
      <div className="mb-6 grid gap-4 lg:grid-cols-2">
        {Array.from({ length: 4 }).map((_, i) => (
          <Card key={i}>
            <CardHeader>
              <Skeleton className="h-5 w-28" />
            </CardHeader>
            <CardContent className="space-y-2">
              {Array.from({ length: 4 }).map((_, j) => (
                <div key={j} className="flex gap-4">
                  <Skeleton className="h-4 w-32" />
                  <Skeleton className="h-4 flex-1" />
                </div>
              ))}
            </CardContent>
          </Card>
        ))}
      </div>
      <SkeletonSectionHeading />
      <SkeletonTable rows={3} columns={3} />
    </>
  );
}
