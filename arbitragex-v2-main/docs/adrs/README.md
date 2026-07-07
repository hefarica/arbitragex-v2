# Architecture Decision Records (ADRs)

This directory contains all Architecture Decision Records for the arbitragex-v2 project, following the [MADR 3.0](https://adr.github.io/madr/) template.

## Index

| # | Title | Status | Date |
|---|-------|--------|------|
| [0001](0001-zero-mocks.md) | Zero-Mocks — All Components Consume Live Data Only | accepted | 2026-05-18 |
| [0002](0002-r8-fail-honest.md) | R8 Fail-Honest — Transparent System State Reporting | accepted | 2026-05-18 |
| [0003](0003-bi-eje-scoreboard.md) | Bi-Eje Scoreboard — Quantitative Maturity Model | accepted | 2026-05-18 |
| [0004](0004-mkdocs-material-diataxis.md) | MkDocs Material with Diátaxis Documentation Framework | accepted | 2026-05-18 |
| [0005](0005-six-canonical-workflows.md) | Six Canonical CI/CD Workflows | accepted | 2026-05-18 |

## Status Legend

- **proposed** — Under discussion, not yet decided
- **accepted** — Decision made and being implemented
- **deprecated** — Decision was valid but is no longer recommended
- **superseded** — Replaced by a newer ADR (link provided)

## Template

Use [MADR 3.0 template](_template.md) for new ADRs.

## Contributing

1. Copy `_template.md` to `NNNN-title.md`
2. Fill in all sections
3. Set status to `proposed`
4. Open a PR for review
5. On acceptance, update status to `accepted` and merge
