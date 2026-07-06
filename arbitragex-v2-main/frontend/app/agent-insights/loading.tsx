import { SkeletonPageHeader, SkeletonKpiGrid } from "@/components/skeletons";

export default function Loading() {
  return (
    <>
      <SkeletonPageHeader />
      <SkeletonKpiGrid count={6} />
    </>
  );
}
