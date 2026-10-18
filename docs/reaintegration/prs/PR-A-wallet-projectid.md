# PR-A — `/wallet` projectId (SEV-1) — 2026-10-18

**Gate:** `scripts/gates/g-wallet-projectid-01.sh` → **PASS** (3 condiciones).

## El defecto

`frontend/lib/web3/wagmiConfig.ts` pasaba el literal *placeholder* como `projectId` a
`getDefaultConfig` de RainbowKit. RainbowKit 2.2.x construye su conector WalletConnect sobre
**Reown AppKit**, así que la página inicializaba ese SDK y el sitio vivo llamaba a endpoints de
PRODUCCIÓN con un id falso:

```
POST https://pulse.walletconnect.org/e?projectId=<placeholder>&st=appkit   -> 400
GET  https://api.web3modal.org/appkit/v1/config?projectId=<placeholder>    -> 403
```

Y el estado degradado **no se declaraba**: el HTML SSR de `/wallet` no contenía ninguna traza
(medido). Sólo aparecía si el operador elegía WalletConnect en el guard (fase cliente).

## El arreglo (aditivo, revert de un paso)

1. **`frontend/lib/web3/walletconnect-config.ts`** (nuevo, server-safe): saca a un módulo puro
   (sin `wagmi`/`rainbowkit`) las funciones de configuración, para que un Server Component pueda
   leer el estado sin arrastrar el SDK.
2. **`wagmiConfig.ts`**: con id real → config RainbowKit normal. **Sin id real → misma superficie
   RainbowKit con lista de wallets inyectada (`injectedWallet`) y `projectId: ""`**: no se crea
   ningún conector WalletConnect ⇒ **no se inicializa AppKit** ⇒ no hay llamada upstream. Las
   wallets del navegador / EIP-6963 siguen funcionando.
3. **`WalletConnectDegradedNotice.tsx`** (nuevo, Server Component): declara el estado degradado en
   el **primer render** (SSR), con el motivo máquina y sin inventar nada.
4. **Renombre del motivo máquina**: `walletconnect_project_id_missing` → **`walletconnect_not_configured`**.
   El token viejo era *el valor placeholder* que se filtraba al SDK; usarlo también como código de
   estado hacía indistinguible "estado" de "valor que enviábamos". Un código de estado nombra el
   estado. (Se actualizan `wagmiConfig.test.ts` y `WalletOnboarding.test.tsx`.)

## Evidencia (salida real)

```
$ scripts/gates/g-wallet-projectid-01.sh
  info  built client chunks: 292
  PASS  (a) 0 built chunks contain the placeholder project-id literal (found 0)
  PASS  (c) /wallet responded with HTML
  PASS  (c) SSR HTML carries the degraded-notice testid
  PASS  (c) SSR HTML carries the machine reason (declared, not fabricated)
  info  headless probe: {"bad":[],"declared":1,"reason":"reason=walletconnect_not_configured"}
  PASS  (b) no >=400 response from pulse.walletconnect.org / api.web3modal.org
  PASS  (c) hydrated DOM keeps the declaration
G-WALLET-PROJECTID-01: PASS
```

- `npx tsc --noEmit` → **PASS**.
- `npx vitest run` → **155 archivos / 1639 tests, 0 fallos**.

## Revert

Un paso: `git revert` de este commit restaura el `projectId` placeholder y elimina el aviso SSR.
No se tocó ningún otro camino (ni firmantes, ni broadcast).
