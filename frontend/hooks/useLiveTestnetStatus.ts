import { useState, useEffect } from "react";

export function useLiveTestnetStatus() {
  const [status, setStatus] = useState<any>(null);

  useEffect(() => {
    fetch("/api/v1/readiness/decision")
      .then((r) => r.json())
      .then(setStatus);
  }, []);

  return { status };
}
