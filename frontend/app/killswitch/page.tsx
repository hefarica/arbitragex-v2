"use client";

import { useEffect, useState } from "react";
import { getStatus, toggleKillswitch, type KillSwitchState } from "../../lib/api-client";

export default function KillSwitchPage() {
  const [state, setState] = useState<KillSwitchState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [token, setToken] = useState<string>("");
  const [reason, setReason] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [lastOutcome, setLastOutcome] = useState<string | null>(null);

  useEffect(() => {
    const t = typeof window !== "undefined" ? window.localStorage.getItem("arbx_admin_token") ?? "" : "";
    setToken(t);
    void refresh();
  }, []);

  async function refresh() {
    setLoading(true);
    const r = await getStatus();
    setLoading(false);
    if (!r.ok) { setError(r.error); return; }
    setError(null);
    setState(r.data.killswitch);
  }

  async function onToggle(next: boolean) {
    if (!token) { setError("admin token required"); return; }
    if (!reason.trim()) { setError("reason is required for audit trail"); return; }
    setBusy(true);
    window.localStorage.setItem("arbx_admin_token", token);
    const r = await toggleKillswitch(next, reason.trim(), token);
    setBusy(false);
    if (!r.ok) { setError(r.error); setLastOutcome(null); return; }
    setError(null);
    setState(r.data);
    setLastOutcome(`kill-switch set to ${next ? "ENABLED" : "disabled"} at ${new Date().toLocaleTimeString()}`);
    setReason("");
  }

  return (
    <section>
      <h1>Kill-switch</h1>
      <p style={{ color: "#9ca3af" }}>
        Flipping to ENABLED causes every hot-path service to refuse new executions within ≤ 5 s.
        This is the first step of every incident runbook. Audit log records every toggle.
      </p>

      {loading && <p style={{ color: "#9ca3af" }}>loading current state…</p>}

      {state && (
        <div style={{
          padding: 16, borderRadius: 6, marginBottom: 20,
          background: state.enabled ? "#2a1010" : "#0a2010",
          border: `2px solid ${state.enabled ? "#ef4444" : "#22c55e"}`,
        }}>
          <div style={{ fontSize: 20, fontWeight: 700,
                        color: state.enabled ? "#ef4444" : "#22c55e" }}>
            {state.enabled ? "ARMED — executions blocked" : "DISABLED — executions permitted"}
          </div>
          {state.reason && <div style={{ marginTop: 6 }}>reason: <em>{state.reason}</em></div>}
          <div style={{ color: "#9ca3af", fontSize: 12, marginTop: 4 }}>
            updated {new Date(state.updated_at).toLocaleString()}
            {state.triggered_by && <> · by <code>{state.triggered_by}</code></>}
          </div>
        </div>
      )}

      {error && <p style={{ color: "#f87171" }}>error: <code>{error}</code></p>}
      {lastOutcome && <p style={{ color: "#22c55e" }}>{lastOutcome}</p>}

      <h2>Toggle</h2>
      <div style={{ display: "grid", gap: 10, maxWidth: 520 }}>
        <label style={{ fontSize: 13, color: "#9ca3af" }}>
          admin token (stored in localStorage)
          <input
            type="password"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            style={{ width: "100%", padding: 8, background: "#0f1114", color: "#e7e9eb",
                     border: "1px solid #1f2937", borderRadius: 4 }}
          />
        </label>
        <label style={{ fontSize: 13, color: "#9ca3af" }}>
          reason (required; goes to audit_log)
          <input
            type="text"
            value={reason}
            placeholder="e.g. investigating high revert rate"
            onChange={(e) => setReason(e.target.value)}
            style={{ width: "100%", padding: 8, background: "#0f1114", color: "#e7e9eb",
                     border: "1px solid #1f2937", borderRadius: 4 }}
          />
        </label>
        <div style={{ display: "flex", gap: 10 }}>
          <button
            disabled={busy}
            onClick={() => onToggle(true)}
            style={{ padding: "10px 16px", background: "#7f1d1d", color: "#fff",
                     border: "1px solid #ef4444", borderRadius: 4, cursor: busy ? "wait" : "pointer" }}
          >
            ARM (enable)
          </button>
          <button
            disabled={busy}
            onClick={() => onToggle(false)}
            style={{ padding: "10px 16px", background: "#166534", color: "#fff",
                     border: "1px solid #22c55e", borderRadius: 4, cursor: busy ? "wait" : "pointer" }}
          >
            disable
          </button>
          <button
            onClick={() => void refresh()}
            style={{ padding: "10px 16px", background: "#1f2937", color: "#e7e9eb",
                     border: "1px solid #374151", borderRadius: 4, cursor: "pointer" }}
          >
            refresh
          </button>
        </div>
      </div>
    </section>
  );
}
