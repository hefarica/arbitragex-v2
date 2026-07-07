# Skill 16: Security-Oriented Frontend Architect

## 1. Propósito
Blindar la aplicación contra vectores de ataque Frontend: Cross-Site Scripting (XSS), Cross-Site Request Forgery (CSRF), fugas de variables de entorno (Secret Leakage), inyecciones en la hidratación, Clickjacking, y dominar las Content Security Policies (CSP). Asegurar la transmisión de tokens y permisos operativos críticos (ArbitrageX Admin Token).

## 2. Aplicación directa en ARBITRAGEX
El dashboard tiene controles como el "Kill Switch" que pueden detener el motor MEV. Si un token de administrador se filtra en el HTML (`NEXT_PUBLIC_ADMIN_TOKEN`), o si la aplicación es vulnerable a XSS (permitiendo que un script externo ejecute un click automático en el Kill Switch), las pérdidas financieras serían inmediatas.

## 3. Problemas que resuelve
- Exposición accidental de llaves privadas o URLs de bases de datos al navegador.
- Inyección de HTML arbitrario mediante el renderizado crudo de datos del exterior.
- Peticiones forjadas (CSRF) desde dominios de terceros.
- Cookies de sesión o Auth robadas por Javascript debido a falta de flag `HttpOnly`.

## 4. Reglas Inmutables
- **Regla del NEXT_PUBLIC:** NUNCA, jamás, utilices el prefijo `NEXT_PUBLIC_` para contraseñas, JWTs, llaves privadas o strings de conexión a base de datos.
- Prohibido utilizar `dangerouslySetInnerHTML` salvo que la entrada haya pasado estrictamente por un sanitizador como `DOMPurify`. Los Server Components son seguros por defecto al escapar texto, no desactives esta protección.
- Todos los Server Actions que ejecuten acciones destructivas (Ej. `killEngine()`, `updateConfig()`) DEBEN re-autenticar al usuario / token internamente antes de ejecutar. No confiar en que el botón estaba "oculto" en la UI.
- Implementar encabezados estrictos en `next.config.js` (X-Frame-Options: DENY, Strict-Transport-Security, Content-Security-Policy).

## 5. Nivel de Madurez
Arquitecto de Seguridad - El Frontend como la primera línea de defensa activa.
