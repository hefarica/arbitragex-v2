<!--
ArbitrageX v2 — PR template.
Every PR MUST fill in the "No-hardcode doctrine" block below, even if the
answer is "not applicable". CI check `no-hardcode` enforces lint-level rules;
this template is for the claims lint can't make.
-->

## Summary
<!-- what the change does and why -->

## Linked spec / plan
<!-- link to the file in `docs/superpowers/specs/` if applicable -->

## Test plan
- [ ] Tests added / updated
- [ ] `npm run typecheck` (TS services)
- [ ] `cargo test --workspace` (Rust services)
- [ ] Manual evidence recorded below

### Manual evidence
<!-- logs, screenshots, curl outputs. Paste real output, no summaries. -->

---

## No-hardcode doctrine block (required)

Doctrine: `docs/governance/NO-HARDCODE-DOCTRINE.md`. If any question is N/A say so explicitly.

1. **Rules applied**
   <!-- Which doctrine rules were relevant here. e.g. "§Productive endpoints: relay URLs now loaded from DB table `relays`, not config file." -->

2. **Data requirements matrix delta**
   <!-- Which rows in `docs/governance/DATA-MATRIX.md` are added/changed. -->

3. **Progressive solicitation**
   <!-- In which phase (1–5) does each new datum live? Does this PR wire the UI/CLI to solicit it? -->

4. **Sensitive vs non-sensitive inventory delta**
   <!-- Any new entries? Where do they rest (Vault path / DB column / file)? -->

5. **Validation**
   <!-- How is each new datum validated at intake? Fail-closed on invalid? -->

6. **Storage**
   <!-- Exact Vault path, DB table+column, or env var name. -->

7. **Features pending real data**
   <!-- Anything you shipped in state PENDING_CREDENTIALS / PENDING_CONFIG / DESIGNED? How is the gap surfaced in UI/logs? -->

8. **No-hardcode checklist**
   - [ ] No credentials embedded
   - [ ] No productive endpoints embedded (non-canonical)
   - [ ] No contracts embedded without catalog indirection
   - [ ] No wallet/signer addresses embedded
   - [ ] No API keys embedded
   - [ ] No business thresholds embedded without config
   - [ ] No productive asset lists embedded without a real source
   - [ ] No productive risk parameters embedded
   - [ ] Every external dependency asks for its datum at the correct step
   - [ ] Every critical config is validated at boot (fail-fast)
   - [ ] Every sensitive config lives outside code
   - [ ] Every feature declares its data dependencies explicitly
   - [ ] The app knows when it can't operate and says so in the UI
   - [ ] The app never appears to operate without real data

9. **Validations executed**
   <!-- Grep commands run, tests run, manual probes. Paste outputs or link them. -->

10. **Open risks if data is missing**
    <!-- For each PENDING_* item, what does the user see? Does recon/observability make it obvious? -->
