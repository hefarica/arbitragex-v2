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
    if (!reason.trim()) { setError("reason is required for the audit log"); return; }
    setBusy(true);
    window.localStorage.setItem("arbx_admin_token", token);
    const r = await toggleKillswitch(next, reason.trim(), token);
    setBusy(false);
    if (!r.ok) { setError(r.error); setLastOutcome(null); return; }
    setError(null);
    setState(r.data);
    setLastOutcome(`kill-switch set to ${next ? "ARMED" : "disabled"} at ${new Date().toLocaleTimeString()}`);
    setReason("");
  }

  return (
    <>
      <section className="hero">
        <h1>Kill-switch</h1>
        <p className="hero__lede">
          Arming blocks every hot-path service from submitting new executions within ≤ 5&nbsp;s.
          This is the first step of every incident runbook. Every toggle is recorded in the audit log.
        </p>
      </section>

      {loading && <div className="muted">loading current state…</div>}

      {state && (
        <div className={state.enabled ? "banner banner--bad" : "banner banner--ok"}>
          <div className="row">
            <span className={state.enabled ? "dot dot--danger" : "dot dot--success"} aria-hidden />
            <span className="banner__title">
              {state.enabled ? "ARMED — executions blocked" : "DISABLED — executions permitted"}
            </span>
          </div>
          {state.reason && <div>reason: <em>{state.reason}</em></div>}
          <div className="banner__meta">
            updated {new Date(state.updated_at).toLocaleString()}
            {state.triggered_by && <> · by <code>{state.triggered_by}</code></>}
          </div>
        </div>
      )}

      {error && <div className="err mt-4">error: <code>{error}</code></div>}
      {lastOutcome && (
        <div className="banner banner--ok mt-4">
          <div>{lastOutcome}</div>
        </div>
      )}

      <h2>Toggle</h2>
      <div className="card max-w-form">
        <div className="col gap-4">
          <label className="label">
            admin token (stored in localStorage)
            <input
              className="input"
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              autoComplete="off"
            />
          </label>
          <label className="label">
            reason (required; goes to audit_log)
            <input
              className="input"
              type="text"
              value={reason}
              placeholder="e.g. investigating high revert rate"
              onChange={(e) => setReason(e.target.value)}
            />
          </label>
          <div className="row">
            <button
              type="button"
              disabled={busy}
              onClick={() => onToggle(true)}
              className="btn btn--danger"
            >
              Arm kill-switch
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => onToggle(false)}
              className="btn btn--success"
            >
              Disable
            </button>
            <button
              type="button"
              onClick={() => void refresh()}
              className="btn btn--ghost"
            >
              Refresh
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
