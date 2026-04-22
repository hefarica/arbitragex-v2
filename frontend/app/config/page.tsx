import { getConfigCurrent } from "../../lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function ConfigPage() {
  const res = await getConfigCurrent();

  if (!res.ok) {
    return (
      <section>
        <h1>Current config</h1>
        <p style={{ color: "#f87171" }}>edge error: <code>{res.error}</code></p>
      </section>
    );
  }

  const c = res.data;

  return (
    <section>
      <h1>Current config</h1>
      <p style={{ color: "#9ca3af" }}>
        Loaded from <code>configs/app.toml</code> at service boot. Secrets are NEVER
        rendered here — if you need to rotate keys, see the <code>rotate-secrets</code> runbook.
      </p>

      <h2>System</h2>
      <KV obj={c.system} />

      <h2>Execution</h2>
      <KV obj={c.execution} />
      {c.execution["paper_mode"] === true && (
        <p style={{ color: "#22c55e" }}>✓ paper-mode is ON. Real capital is not at risk.</p>
      )}

      <h2>Risk</h2>
      <KV obj={c.risk} />

      <h2>Scoring</h2>
      <KV obj={c.scoring} />

      <h2>Chains</h2>
      <table style={{ borderCollapse: "collapse", fontSize: 13 }}>
        <thead>
          <tr>{["chain_id", "name", "enabled"].map((h) => (
            <th key={h} align="left" style={{ padding: "4px 10px", borderBottom: "1px solid #1f2937" }}>{h}</th>
          ))}</tr>
        </thead>
        <tbody>
          {c.chains.map((ch) => (
            <tr key={ch.chain_id}>
              <td style={{ padding: "4px 10px", borderBottom: "1px solid #111827" }}>{ch.chain_id}</td>
              <td style={{ padding: "4px 10px", borderBottom: "1px solid #111827" }}>{ch.name}</td>
              <td style={{ padding: "4px 10px", borderBottom: "1px solid #111827",
                           color: ch.enabled ? "#22c55e" : "#9ca3af" }}>
                {ch.enabled ? "yes" : "no"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h2>Relays</h2>
      <table style={{ borderCollapse: "collapse", fontSize: 13 }}>
        <thead>
          <tr>{["name", "enabled", "chains", "endpoint"].map((h) => (
            <th key={h} align="left" style={{ padding: "4px 10px", borderBottom: "1px solid #1f2937" }}>{h}</th>
          ))}</tr>
        </thead>
        <tbody>
          {c.relays.map((r) => (
            <tr key={r.name}>
              <td style={{ padding: "4px 10px", borderBottom: "1px solid #111827" }}>{r.name}</td>
              <td style={{ padding: "4px 10px", borderBottom: "1px solid #111827",
                           color: r.enabled ? "#22c55e" : "#9ca3af" }}>
                {r.enabled ? "yes" : "no"}
              </td>
              <td style={{ padding: "4px 10px", borderBottom: "1px solid #111827" }}>{r.chains.join(",")}</td>
              <td style={{ padding: "4px 10px", borderBottom: "1px solid #111827",
                           fontFamily: "monospace", color: "#9ca3af" }}>
                {r.endpoint || "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h2>Circuit breakers</h2>
      <KV obj={Object.fromEntries(c.circuit_breakers.map((cb) =>
        [cb.name, `threshold=${cb.threshold} window=${cb.window_ms}ms cooldown=${cb.cooldown_ms}ms`]
      ))} />
    </section>
  );
}

function KV({ obj }: { obj: Record<string, unknown> }) {
  const rows = Object.entries(obj);
  return (
    <table style={{ borderCollapse: "collapse", fontSize: 13, marginBottom: 12 }}>
      <tbody>
        {rows.map(([k, v]) => (
          <tr key={k}>
            <td style={{ padding: "2px 12px 2px 0", color: "#9ca3af", verticalAlign: "top" }}>{k}</td>
            <td style={{ padding: "2px 0", fontFamily: "monospace" }}>{String(v)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
