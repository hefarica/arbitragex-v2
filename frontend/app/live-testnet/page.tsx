"use client";
import { useLiveTestnetStatus } from "@/hooks/useLiveTestnetStatus";
import { useEventStream } from "@/hooks/useEventStream";

export default function LiveTestnetPage() {
  const { status } = useLiveTestnetStatus();
  const { events, isConnected } = useEventStream(11155111);

  return (
    <div className="p-6">
      <h1>LIVE_TESTNET</h1>
      <div>Mode: {status?.mode}</div>
      <div>Connected: {isConnected ? "Yes" : "No"}</div>
      <div>Events: {events.length}</div>
    </div>
  );
}
