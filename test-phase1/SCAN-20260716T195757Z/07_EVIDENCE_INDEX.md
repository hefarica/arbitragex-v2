# Evidence Index

| Phase | Source | Operation | Result | Exit | Duration ms | Note |
|---|---|---|---|---|---|---|
| Repository | local | open-existing-repository | C:\Users\HFRC\Desktop\arbitragex-v2-main (17) | 0 |  | No checkout, reset, pull, clean or write operation performed. |
| Git metadata | local | git status --porcelain=v1 | M .claude/settings.json<br> M .claude/settings.local.json<br>?? .claude/agents/skills/adidas-mode/<br>?? PLAN_MAESTRO_100_DEEPWIKI.md<br>?? fix_scanner.py<br>?? repo-vps-audits/ | 0 | 5880.0 |  |
| Git metadata | local | git rev-parse HEAD | aa28fee4ff421bc907b16358418cb1f5505887c4 | 0 | 39.8 |  |
| Git metadata | local | git branch --show-current | main | 0 | 38.8 |  |
| Git metadata | local | git remote get-url origin | https://github.com/hefarica/arbitragex-v2.git | 0 | 28.3 |  |
| Git metadata | local | git log -1 '--format=%H\|%cI\|%s' | aa28fee4ff421bc907b16358418cb1f5505887c4\|2026-07-15T22:29:32-05:00\|feat(live-testnet-v2): SSE endpoint, executor stub, E2E token from env, CI secrets | 0 | 31.6 |  |
| Repository inventory | local | walk-repository | 9020 files, 1918 directories, 56 routes | 0 | 443.0 |  |
| Compose | local | parse .devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse docker/compose.hotpath-test.yml | 7 services | 0 |  |  |
| Compose | local | parse docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse docker/compose.prod.yml | 22 services | 0 |  |  |
| Compose | local | parse docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| VPS | ssh | skip-vps-scan | VPS host not configured | 0 |  |  |