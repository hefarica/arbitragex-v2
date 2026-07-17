# SEMIOTIC-BRIDGE — PROTECTION PROTOCOL (INMUTABLE)

## SEMIOTIC-BRIDGE INVARIANCE (WORKSPACE-LEVEL)

$$ \forall \text{build}, \exists \text{semiotic-bridge} \implies \text{workspace.resolvable} = \text{TRUE} $$

## PROTECTION MECHANISMS

1. **WORKSPACE MANIFEST REFERENCE:**
   - `backend/Cargo.toml` declares `semiotic-bridge` in `members = [...]`
   - Removal from `members` without removing directory = build failure
   - Removal of directory without removing from `members` = build failure
   - BOTH must change atomically (never one without the other)

2. **DOCKERFILE CONTRACT:**
   - ALL workspace Dockerfiles MUST include:
     ```dockerfile
     COPY semiotic-bridge ./semiotic-bridge
     ```
   - Missing COPY = `failed to read /build/semiotic-bridge/Cargo.toml`
   - Verified in: recon, relays-client, searcher-rs, sim-ctl

3. **CI/CD GATE:**
   - Deploy fails on missing workspace member
   - Build error: `cargo build` exit code 101
   - No container deploys until resolved

4. **VERSION CONTROL GUARD:**
   - This file (PROTECTION.md) must exist in the directory
   - `.gitattributes` marks it as ` linguist-generated=false` (never hidden)
   - Pre-commit hook: verify `members` list and `Dockerfile` copies match

## REMOVAL PROTOCOL (INTENTIONAL ONLY)

If semiotic-bridge must be removed:
1. Remove from `backend/Cargo.toml` `members` list
2. Remove from ALL Dockerfile COPY statements
3. Remove `backend/semiotic-bridge/` directory
4. Update docker/compose*.yml if referenced
5. Run full `cargo build --release` locally to verify
6. Deploy to staging first, then production

## SEMIOTIC-BRIDGE PURPOSE

English-to-mathematical translation layer for the ArbitrageX v2 system.
- Fact-Forcing Gate translations
- Lexico-topological transforms
- Pure mathematics output enforcement

**DO NOT REMOVE WITHOUT OPERATOR SIGN-OFF AND FULL BUILD VERIFICATION.**

## INVARIANCE CHECK

```bash
# Run before any commit touching this directory:
grep -q 'semiotic-bridge' backend/Cargo.toml && \
grep -q 'semiotic-bridge' backend/*/Dockerfile && \
echo "PROTECTION VERIFIED" || echo "PROTECTION VIOLATION"
```
