# Skill 10: TypeScript Domain Modeling Master

## 1. Propósito
Blindar la aplicación a través de tipado estricto y aserciones en el dominio lógico. Eliminar `any` y `unknown` peligrosos, crear contratos precisos mediante tipos de unión discriminada (Discriminated Unions) para manejos de errores, flujos y eventos de red (WebSocket Payloads), garantizando la integración continua E2E entre Node y Rust.

## 2. Aplicación directa en ARBITRAGEX
El pipeline entre el mempool (`scanner.rs`), la base de datos (Postgres), y la interfaz requiere contratos estrictos. Las "Opportunities" emitidas por el `PrioritizationSpine` hacia Redis y despachadas al navegador deben poseer un tipado exacto (ej. `profit_usd`, `gas_cost`, `risk_score`) sincronizado y validadas con Zod.

## 3. Problemas que resuelve
- Errores de "undefined is not an object" en Runtime de Producción.
- APIs que cambian silenciosamente sin que el frontend lo detecte, causando UI rotas (White screens).
- Tipados mentirosos (Type assertions `as MyType` que engañan al compilador).
- Manejo crudo de errores asíncronos (`try/catch` sin tipar el objeto Error atrapado).

## 4. Reglas Inmutables
- **Prohibido el uso de `any`**. Reemplazar siempre por `unknown` si el dato de entrada es dinámico, y validarlo.
- Usar **Uniones Discriminadas** para Respuestas de Red o Eventos de WebSocket en lugar de objetos gigantes opcionales (`{ type: "SUCCESS" | "ERROR", payload?: Data, error?: string }`).
- Los contratos críticos del lado del cliente (APIs, Edge payloads) deben ser protegidos mediante validadores en tiempo de ejecución (ej. **Zod** o Valibot). Si el payload del motor de Rust cambia de forma, Zod debe escupir un log detallado y la UI debe procesar el error "gracefully", en lugar de fallar en una prop del componente.
- Abstraer el dominio (Entidades: `Opportunity`, `NodeStatus`, `Alert`) en archivos globales `/types` o schemas.

## 5. Nivel de Madurez
Maestría - Type-Driven Development y End-to-End Type Safety.
