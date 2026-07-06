import { SkeletonPageHeader, SkeletonTable } from "@/components/skeletons";

export default function Loading() {
  return (
    <>
      <SkeletonPageHeader />
      <SkeletonTable rows={12} columns={4} />
    </>
  );
}
