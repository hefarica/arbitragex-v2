export const meta = {
  name: "crypto-deep-analyser",
  description: "Eight-agent crypto deep-research pipeline ported from the operator Venice mind crypto-analyser-1372: tokenomics, market, community, news, risk, architecture, use-case, scaling - every agent carries the arbitragex-omniscience canon - then adversarial fact-check and confidence-weighted synthesis",
  phases: [
    { title: "Analysts", detail: "eight specialized analysts research the target in parallel" },
    { title: "Verify", detail: "adversarial cross-check of red flags and load-bearing claims" },
    { title: "Synthesize", detail: "confidence-weighted report with citations and data gaps" },
  ],
}

const TARGET = args && args.target ? String(args.target).trim() : ""
const LANG = args && args.lang ? String(args.lang) : "español"
const AS_OF = args && args.asOf ? String(args.asOf) : "2026-09-06"

if (!TARGET) {
  return { error: "Missing args.target. Call with { target: \"TOKEN or PROJECT\", lang: \"español\", asOf: \"YYYY-MM-DD\" }" }
}

// Omniscience canon injected into every agent (paths verified on disk 2026-09-06, repo-relative).
const OMNISCIENCE_PREAMBLE = [
  "You operate inside the ArbitrageX omniscience canon - the institutional DeFi/MEV research base of this repository (264 strategies, 31 math operators, 60 detectors, world-class research corpus).",
  "Canon resources on disk (paths relative to repo root - consult them with Read, Grep, or Glob whenever they strengthen your analysis):",
  "- skills/arbitragex-ultra/SUPER_SKILL.md - architecture and rules of the 264-strategy / 31-operator system",
  "- skills/arbitragex-ultra/knowledge_graph.jsonl - 2,511 edges Strategy-Operator-Detector",
  "- skills/arbitragex-ultra/capability_matrix.json - per-strategy state",
  "- skills/arbitragex-ultra/operators/op_XX/ and skills/arbitragex-ultra/strategies/MEV-XX-XXX/ - per-unit skill and data files",
  "- skills/arbitragex-ultra/world/graph-algorithms, world/mev-practice, world/defi-protocols, world/security-simulation, world/quant-math - state-of-the-art research corpus",
  "- docs/excel_strategies_extracted.json (267 strategies), docs/excel_operators_extracted.json (33), docs/excel_matrix_extracted.json (1,716 associations), docs/excel_detectors_extracted.json (60)",
  "- docs/ROUTES_CROWN_JEWEL_DOCTRINE.md - routing doctrine (fees on-chain, CFMM convexity)",
  "Canon reasoning rules you inherit:",
  "1. The workbook is 5 percent - never the limit; when you find something better, register it.",
  "2. Discovery (enumerating topology) is not evaluation (gates, sizing, expected value).",
  "3. Read fees and protocol parameters from primary sources or on-chain - never hardcode assumptions.",
  "4. Fail-honest: missing = data gap, zero = exactly zero. Never fabricate.",
  "5. Classify every claim: PRIMARY_SOURCE / CANON_INTERNAL / INFERRED / HYPOTHESIS / UNKNOWN.",
  "Evidence admissibility: web evidence carries its URL in source_url; canon evidence is equally admissible and must be labeled CANON_INTERNAL with the canon file path as its citation in source_url.",
].join("\n")

const ANALYST_SCHEMA = {
  type: "object",
  properties: {
    category: { type: "string" },
    summary: { type: "string" },
    findings: {
      type: "array",
      items: {
        type: "object",
        properties: {
          claim: { type: "string" },
          evidence: { type: "string" },
          source_url: { type: "string" },
          confidence: { type: "string", enum: ["high", "medium", "low"] },
        },
        required: ["claim", "evidence", "source_url", "confidence"],
      },
    },
    red_flags: { type: "array", items: { type: "string" } },
    data_gaps: { type: "array", items: { type: "string" } },
  },
  required: ["category", "summary", "findings", "red_flags", "data_gaps"],
}

const VERDICTS_SCHEMA = {
  type: "object",
  properties: {
    verdicts: {
      type: "array",
      items: {
        type: "object",
        properties: {
          claim: { type: "string" },
          verdict: { type: "string", enum: ["confirmed", "refuted", "unverifiable"] },
          note: { type: "string" },
          source_url: { type: "string" },
        },
        required: ["claim", "verdict", "note"],
      },
    },
  },
  required: ["verdicts"],
}

const CHARTERS = [
  {
    key: "tokenomics",
    name: "Tokenomics Analyst",
    charter: "Supply mechanics (max / total / circulating supply, emission schedule, fee burns), distribution and allocation across team / investors / community / treasury / ecosystem, vesting and unlock calendar with dates and amounts, current inflation rate, and token utility (governance, staking, gas, fee capture, discounts). Quantify with numbers and as-of dates.",
    canon: "Cross-reference against the institutional strategy taxonomy: skills/arbitragex-ultra/capability_matrix.json and docs/excel_strategies_extracted.json show whether and how the analyzed asset is integrated into the 264-strategy universe; docs/ROUTES_CROWN_JEWEL_DOCTRINE.md covers fee-capture and on-chain fee reading doctrine.",
  },
  {
    key: "market",
    name: "Market and Price Analyst",
    charter: "Price action across horizons (7d / 30d / 1y, drawdowns, distance from ATH), trading volume and liquidity (spot venues plus DEX pools, order book depth on major exchanges), market structure (market cap, FDV, FDV/MCAP ratio, listing venues, derivatives availability), and correlations with BTC, ETH, and sector peers.",
    canon: "skills/arbitragex-ultra/world/quant-math (Kyle microstructure, VPIN flow toxicity, Kelly sizing, EVT tail risk) is your market-structure lens; skills/arbitragex-ultra/world/mev-practice grounds real execution margins and adversarial flow.",
  },
  {
    key: "community",
    name: "Community and Sentiment Analyst",
    charter: "Social footprint and sentiment (X/Twitter following and engagement, Reddit activity, Discord or Telegram size and health), developer activity (GitHub commits, contributors, open issues over the last 3 to 12 months), governance participation rates, holder distribution signals, and community health indicators including bot-activity and sentiment shifts.",
    canon: "If the target IS ArbitrageX (or closely related), the repo itself is a primary dev-activity source - inspect the working tree and git history. Canon context: skills/arbitragex-ultra/SUPER_SKILL.md.",
  },
  {
    key: "news",
    name: "News and Developments Analyst",
    charter: "Material developments from the last 90 days: protocol upgrades and hard forks, partnerships, exchange listings or delistings, funding rounds, regulatory actions, layoffs or team changes, and roadmap progress versus previously promised milestones. Include dates and links for every item.",
    canon: "When a development touches routing, execution, or fee doctrine, check alignment against docs/ROUTES_CROWN_JEWEL_DOCTRINE.md and note agreement or divergence.",
  },
  {
    key: "risk",
    name: "Risk and Security Analyst",
    charter: "Contract audit coverage (auditor names, dates, severity findings, remediation status), exploit and hack history (incident reports, rekt-style databases), regulatory exposure by jurisdiction, team transparency (doxxed founders, KYC status, track record), centralization and admin-key / upgradeability risks, cryptographic concerns, and every red flag you can substantiate.",
    canon: "skills/arbitragex-ultra/world/security-simulation (attack surfaces, REVM simulation, formal verification) and skills/arbitragex-ultra/world/mev-practice (real searcher economics, incident and exploitation patterns) are your technical risk lens - use them to evaluate audit scope and unlisted attack surfaces.",
  },
  {
    key: "architecture",
    name: "Technical Architecture Analyst",
    charter: "Protocol design (consensus, execution model, data availability strategy), technical innovations versus prior art, scalability approach, client diversity, open technical risks or publicly debated design tradeoffs. Ground every statement in the whitepaper, official specs, or technical docs.",
    canon: "skills/arbitragex-ultra/world/defi-protocols (UniV4, Morpho, Hyperliquid, intents) and skills/arbitragex-ultra/world/graph-algorithms (CFMM routing theory, convex routing) benchmark the design against prior art.",
  },
  {
    key: "usecase",
    name: "Use-case and Competition Analyst",
    charter: "Real-world adoption evidence (active users or addresses, transaction counts, TVL or protocol revenue where applicable, integrations and live deployments), target market size, competitive positioning versus 3 to 5 direct competitors with named alternatives, and moat analysis (network effects, switching costs, liquidity, licenses).",
    canon: "skills/arbitragex-ultra/world/defi-protocols for adoption and integration patterns; skills/arbitragex-ultra/capability_matrix.json for how the asset fits (or fails to fit) institutional strategy taxonomies.",
  },
  {
    key: "scaling",
    name: "Scaling Analyst",
    charter: "Measured versus theoretical throughput (TPS observed on mainnet today versus marketing claims, with source), block time and finality characteristics, Layer 2 or rollup strategy and maturity stage (L2Beat stage 0-2 where applicable), historical stress-test behavior, and congestion and fee behavior under real load events.",
    canon: "skills/arbitragex-ultra/world/graph-algorithms plus world/defi-protocols for L2 and rollup patterns; skills/arbitragex-ultra/world/mev-practice for throughput behavior under adversarial load.",
  },
]

const analystPrompt = (c) => [
  OMNISCIENCE_PREAMBLE,
  "",
  "ROLE:",
  "You are the " + c.name + " inside a crypto deep-research pipeline (8 parallel analysts, each carrying the omniscience canon above).",
  "Research target: " + TARGET + ".",
  "As-of date: " + AS_OF + ". Prefer web sources from the last 90 days and always state the as-of date of any figure.",
  "",
  "CHARTER:",
  c.charter,
  "",
  "YOUR CANON ANGLE (in addition to the web):",
  c.canon,
  "",
  "METHOD:",
  "- Use WebSearch and WebFetch. Prioritize primary sources: official docs, whitepaper, specs, GitHub, block explorers, auditor reports, governance forums. Use reputable aggregators for metrics: CoinGecko, CoinMarketCap, Messari, DefiLlama, L2Beat, token unlock trackers, rekt-style incident databases.",
  "- Every finding MUST carry a real source_url you actually consulted (web URL, or canon file path labeled CANON_INTERNAL per the preamble). Zero fabrication: if you cannot verify something, put it in data_gaps instead of inventing it.",
  "- confidence: high = corroborated by two or more independent primary sources; medium = single reputable source; low = inference, project self-claims, or stale data.",
  "- Separate verified facts from project self-claims; label self-claims as such in the evidence text.",
  "- If the target is a comparison (A vs B), produce findings for each asset.",
  "- Return between 3 and 10 findings: the most load-bearing ones, not everything you saw.",
  "- Set category to \"" + c.name + "\".",
].join("\n")

phase("Analysts")
log("Dispatching 8 analysts for target: " + TARGET)
const analystResults = await parallel(
  CHARTERS.map((c) => () =>
    agent(analystPrompt(c), { label: "analyst:" + c.key, phase: "Analysts", schema: ANALYST_SCHEMA })
  )
)
const ok = analystResults.filter(Boolean)
log(ok.length + "/8 analysts returned structured findings")
if (ok.length === 0) {
  return { error: "All 8 analysts failed or were skipped. No report can be produced without real data (zero-fabrication doctrine).", target: TARGET }
}

// Barrier justified: verification pool and synthesis need the full cross-category result set.
const redFlagClaims = ok.flatMap((r) =>
  (r.red_flags || []).map((f) => ({ claim: f, category: r.category, kind: "red_flag" }))
)
const loadBearing = ok.flatMap((r) =>
  (r.findings || [])
    .filter((f) => f.confidence === "high")
    .slice(0, 3)
    .map((f) => ({ claim: f.claim, category: r.category, kind: "finding", cited_url: f.source_url }))
)
let pool = redFlagClaims.concat(loadBearing)
if (pool.length > 30) {
  log("Claim pool capped at 30 of " + pool.length + " (red flags first) - remainder goes to synthesis unverified and must be labeled low-confidence")
  pool = pool.slice(0, 30)
}

let allVerdicts = []
if (pool.length === 0) {
  log("No red flags or high-confidence claims to verify - skipping Verify phase")
} else {
  phase("Verify")
  log("Adversarially verifying " + pool.length + " claims (" + redFlagClaims.length + " red flags + " + loadBearing.length + " load-bearing findings)")
  const buckets = [
    pool.filter((_, i) => i % 3 === 0),
    pool.filter((_, i) => i % 3 === 1),
    pool.filter((_, i) => i % 3 === 2),
  ].filter((b) => b.length > 0)
  const verifyPrompt = (bucket) => [
    "You are an adversarial fact-checker in a crypto research pipeline. Today is " + AS_OF + ".",
    "For EACH claim below, actively try to REFUTE it using WebSearch and WebFetch against independent sources (never just re-reading the URL the claim cites).",
    "You may also consult the omniscience canon on disk (skills/arbitragex-ultra/** including world/, plus docs/excel_*_extracted.json and docs/ROUTES_CROWN_JEWEL_DOCTRINE.md) to refute technical, MEV, or DeFi claims where web sources are thin - cite the canon file path labeled CANON_INTERNAL.",
    "Verdict rules:",
    "- confirmed: an independent source corroborates the claim.",
    "- refuted: a credible source contradicts it; say what contradicts it and give the URL.",
    "- unverifiable: searches cannot settle it. When in doubt, choose unverifiable, never confirmed.",
    "Return exactly one verdict per claim, echoing the claim text.",
    "",
    "CLAIMS (JSON):",
    JSON.stringify(bucket),
  ].join("\n")
  const verdictResults = await parallel(
    buckets.map((b, i) => () =>
      agent(verifyPrompt(b), { label: "verify:" + (i + 1), phase: "Verify", schema: VERDICTS_SCHEMA })
    )
  )
  allVerdicts = verdictResults.filter(Boolean).flatMap((v) => v.verdicts || [])
  const refuted = allVerdicts.filter((v) => v.verdict === "refuted").length
  log("Verdicts: " + allVerdicts.length + " total, " + refuted + " refuted")
}

phase("Synthesize")
const synthPrompt = [
  "You are the lead analyst of a crypto deep-research pipeline. Today is " + AS_OF + ".",
  "Target: " + TARGET + ".",
  "Write the final report in " + LANG + ".",
  "",
  "You receive (1) structured findings from 8 specialist analysts and (2) adversarial fact-check verdicts.",
  "",
  "OUTPUT CONTRACT - exact section order:",
  "1. Executive Summary - the 5 to 10 most decision-relevant findings, each weighted by its confidence.",
  "2. Detailed Findings - one subsection per analyst category (Tokenomics, Market and Price, Community and Sentiment, News and Developments, Risk and Security, Technical Architecture, Use-case and Competition, Scaling), merging verified findings with inline source URLs.",
  "3. Confidence Assessment - per section: High / Medium / Low plus one line explaining why.",
  "4. Source Citations - deduplicated list of every web URL AND every canon file path used in the report.",
  "5. Risk-Adjusted Conclusion - balanced view that explicitly incorporates uncertainty, red flags, and refuted claims.",
  "6. Data Gaps - everything the pipeline could NOT verify, stated plainly. Never paper over gaps.",
  "",
  "RULES:",
  "- Claims the fact-check REFUTED: drop them or present them as REFUTED with the contradicting evidence.",
  "- Claims the fact-check could not verify: keep only with an explicit low-confidence label.",
  "- Canon-internal evidence must appear labeled CANON_INTERNAL with its file path; classify each claim PRIMARY_SOURCE / CANON_INTERNAL / INFERRED / HYPOTHESIS / UNKNOWN.",
  "- Distinguish verified facts from project self-claims.",
  "- This is research, NOT financial advice: no buy or sell recommendations.",
  "- Be exhaustive but organized; use tables where they add clarity; keep every claim traceable to a URL or canon path.",
  "",
  "ANALYST OUTPUTS (JSON):",
  JSON.stringify(ok),
  "",
  "FACT-CHECK VERDICTS (JSON):",
  JSON.stringify(allVerdicts),
].join("\n")

const report = await agent(synthPrompt, { label: "synthesize", phase: "Synthesize" })

return {
  target: TARGET,
  lang: LANG,
  asOf: AS_OF,
  origin: "Venice mind crypto-analyser-1372 (operator, published 2026-09-06) - local port with adversarial verification + omniscience canon injected into every agent",
  analysts_returned: ok.length,
  analysts_total: CHARTERS.length,
  claims_verified: pool.length,
  verdicts_refuted: allVerdicts.filter((v) => v.verdict === "refuted").length,
  verdicts_unverifiable: allVerdicts.filter((v) => v.verdict === "unverifiable").length,
  data_gaps: ok.flatMap((r) => r.data_gaps || []),
  report,
}
