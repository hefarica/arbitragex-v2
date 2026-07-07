import { SkeletonKpiGrid, SkeletonPageHeader, SkeletonSectionHeading, SkeletonTable } from "@/components/skeletons";

export default function Loading() {
  return (
    <>
      <SkeletonPageHeader />
      <SkeletonKpiGrid count={3} />
      <SkeletonSectionHeading />
      <SkeletonTable rows={6} columns={3} />
    </>
  );
}
