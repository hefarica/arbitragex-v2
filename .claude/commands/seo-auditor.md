---
description: Audit a URL for technical + on-page SEO (read-only) — meta tags, headings, canonical, robots, sitemap, structured data, Open Graph, mobile, Core Web Vitals — and emit prioritized findings with exact fixes. Never mutates the site, never fabricates metrics.
argument-hint: "<url> (a page URL, site root, or sitemap.xml)"
---

# /seo-auditor

Invoke the **seo-auditor** skill and run a technical + on-page SEO audit.

**Target:** `$ARGUMENTS`

## Constraints (read-only / honest)
- GET only. Never submit forms, never POST, never log in, never modify the target.
- Never fabricate a metric or score. If not measured, say "not measured".
- Note (don't bypass) auth walls, paywalls, or `robots.txt` disallows.

## Steps
1. Load `~/.claude/skills/seo-auditor/SKILL.md` and follow it.
2. **Acquire:** `curl -sIL "$ARGUMENTS"` for the status chain + headers; fetch HTML
   (WebFetch / `curl -s`). If unreachable/auth-gated, report and stop.
3. **Inventory** head + content signals (Phase 1) using `resources/seo-audit-checklist.md`.
4. **Technical SEO** (Phase 2): robots.txt, sitemap.xml, HTTPS/redirects, indexability, mobile.
5. **On-page** (Phase 3): title/description length, single H1, headings, alt, links.
6. **Score & prioritize** (Phase 4): CRITICAL → LOW per the checklist thresholds.
7. **Deliver fixes** (Phase 5): the literal markup/config to add; framework-native
   snippet if a framework is detected (verify current API via context7).
8. **Core Web Vitals:** real numbers only if measured via the chrome-devtools MCP
   `lighthouse_audit`; otherwise "not measured".
