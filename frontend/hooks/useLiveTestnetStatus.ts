import { useState, useEffect } from "react";

export function useLiveTestnetStatus() {
  const [status, setStatus] = useState<any>(null);

  useEffect(() => {
    // Public edge path (the worker proxies it to the /api/v1 upstream); the
    // raw /api/v1 path is internal and 404s at the edge.
    fetch("/api/readiness/decision")
      .then((r) => r.json())
      .then(setStatus);
  }, []);

  return { status };
}
