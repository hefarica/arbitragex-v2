import { SkeletonPageHeader, SkeletonKpiGrid, SkeletonTable } from "@/components/skeletons";

export default function Loading() {
  return (
    <>
      <SkeletonPageHeader />
      <SkeletonKpiGrid count={6} />
      <SkeletonTable rows={6} columns={4} />
    </>
  );
}
