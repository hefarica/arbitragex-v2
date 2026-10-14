# DESPACHO A4 — paquete listo para que el CAPITÁN lance el ejecutor (F12 pendiente)

> t17 (α7 FALSATOR) no puede spawnear agentes: "AgentTeams member delegation limit (0)
> reached" (2 intentos: subagent y workflow, errores literales en F12_a4_executor.txt).
> Este es el prompt EXACTO a despachar con la herramienta de subagentes del capitán
> (spawn, contexto limpio). Respuesta esperada según contrato: VEREDICTO_A4: NO CONFORME.

---

Eres un EJECUTOR independiente de verificación de conformidad (rol A4). No tienes
contexto previo: debes verificar TODO por ti mismo leyendo los artefactos.

DOCUMENTO DESPACHADO (release en staging):
- docs/integration-v4/releases/R-20261013T085048Z/R_report.md
- docs/integration-v4/releases/R-20261013T085048Z/C_THETA_verdict.md y .json
- docs/integration-v4/releases/R-20261013T085048Z/Q1_Q8.md
- docs/integration-v4/releases/R-20261013T085048Z/ASSEMBLY.md
- docs/integration-v4/releases/R-20261013T085048Z/V4_dryrun.md
- docs/integration-v4/releases/R-20261013T085048Z/ℛ.tsv

CONSIGNA: ¿este release está CONFORME para SER PROMOVIDO AL ÁRBOL REAL Y PUBLICADO
ahora? Verifica por ti mismo (lee los archivos, NO confíes en resúmenes):
1. C_THETA_verdict.json campo C_literal_reading (¿=1 o =0?) y la condición 7.
2. R_report.md "Escalados al capitán" punto 1: ¿promoción retenida o autorizada?
3. Decisiones pendientes del capitán sin resolver.
4. V4_dryrun.md: ¿dryrun completo o DEGRADADO?
5. Verifica 2 sha256 de ℛ.tsv contra archivos reales (SSOT/strategy_hop_map.json,
   gen/detector_policy.rs) con Get-FileHash.

RESPONDE EXACTAMENTE:
VEREDICTO_A4: CONFORME o NO CONFORME
RAZONES: numeradas, citando archivo+línea/campo que LEÍSTE
BLOQUEANTES: lista o "ninguno"
