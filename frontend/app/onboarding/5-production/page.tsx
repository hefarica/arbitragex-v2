import { OnboardingPhaseStub } from "@/features/onboarding/OnboardingPhaseStub";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default function Phase5Page() {
  const edgeUrl = process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";
  return <OnboardingPhaseStub n={5} edgeUrl={edgeUrl} />;
}
