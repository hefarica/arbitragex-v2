# CHECK QUEUE

| Orden | Check | Estado inicial | Prioridad | Motivo |
|---|---|---|---|---|
| 1 | lint-and-test-contracts | Failing | Alta | Base fundamental EVM, sin dependencias. |
| 2 | lint-and-test-frontend | Failing | Alta | Validar arreglos de Next.js linting recientes. |
| 3 | lint-and-test-node (20/22) | Failing | Alta | Validar arreglos de workspaces npm recientes. |
| 4 | lint-and-test-rust | Failing | Alta | Rust pipeline. |
| 5 | analyze (rust) | Failing | Media | CodeQL (espera fix de build Rust). |
| 6 | analyze (typescript) | Failing | Media | CodeQL (espera fix de frontend/node). |
| 7 | audit Dockerfiles for complete COPY coverage | Failing | Media | Verificación de Dockerfiles. |
| 8 | playwright | Failing | Baja | Pruebas E2E (depende de builds anteriores). |
| 9 | next build (production) | Failing | Baja | Build final (depende de lint frontend). |
| 10 | lint | Failing | Baja | Posiblemente cubierto por lint-and-test-*. |
