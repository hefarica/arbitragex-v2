import { Skeleton } from "@/components/ui/skeleton";
import { SkeletonPageHeader, SkeletonSectionHeading, SkeletonTable } from "@/components/skeletons";

export default function Loading() {
  return (
    <>
      <SkeletonPageHeader />
      <Skeleton className="mb-6 h-20 w-full" />
      <SkeletonSectionHeading />
      <SkeletonTable rows={8} columns={5} />
    </>
  );
}
