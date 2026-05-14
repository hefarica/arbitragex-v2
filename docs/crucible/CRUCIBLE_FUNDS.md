# OMEGA CRUCIBLE — Guia de Fondeo para Testnets

> **Fase:** Validacion antes de Mainnet  
> **Fecha:** 2025-01  
> **Redes:** Holesky | Arbitrum Sepolia | Polygon Amoy

---

## Tabla de Contenidos

1. [Resumen de Fondos Necesarios](#1-resumen-de-fondos-necesarios)
2. [Preparacion de Wallets](#2-preparacion-de-wallets)
3. [Holesky (ETH)](#3-holesky-eth)
4. [Arbitrum Sepolia (ETH)](#4-arbitrum-sepolia-eth)
5. [Polygon Amoy (MATIC)](#5-polygon-amoy-matic)
6. [Verificacion de Balances](#6-verificacion-de-balances)
7. [Troubleshooting](#7-troubleshooting)

---

## 1. Resumen de Fondos Necesarios

| Red | Moneda | Minimo Recomendado | Para que alcanza |
|-----|--------|-------------------|------------------|
| Holesky | ETH | 0.5 ETH | Deploy completo + tests |
| Arbitrum Sepolia | ETH | 0.3 ETH | Deploy completo + tests |
| Polygon Amoy | MATIC | 5 MATIC | Deploy completo + tests |

**Notas importantes:**
- Arbitrum Sepolia requiere ETH de la red Sepolia (L1) y hacer bridge. O usar faucet directo de L2.
- Los faucets tienen limites diarios. Planificar el fondeo con 1-2 dias de anticipacion.
- La ExecutionSigner wallet **NO debe recibir fondos nunca**. Es una clave de firma pura.

---

## 2. Preparacion de Wallets

### Generar wallets nuevas

```bash
# Generar 3 wallets separadas para los roles
forge wallet new --number 3

# O generar con prefijo personalizado (lento)
forge wallet vanity --starts-with 0xC0

# O usar una wallet existente
cast wallet address --private-key $PRIVATE_KEY
```

### Asignacion de roles

| Rol | Wallet | Proposito | Balance esperado |
|-----|--------|-----------|-----------------|
| GAS_SPONSOR | Wallet 1 | Pagar gas de deploys y txs | > 0.5 ETH/MATIC |
| EXECUTION_SIGNER | Wallet 2 | Firmar transacciones de ejecucion | **0 siempre** |
| COLD_TREASURY | Wallet 3 | Recibir yield y ganancias | Variable |

### Configurar .env

```bash
cp .env.crucible .env
# Editar .env con las direcciones y claves privadas generadas
nano .env
source .env
```

---

## 3. Holesky (ETH)

### Opcion A: Google Cloud Web3 Faucet (Recomendado)

1. Ir a: https://cloud.google.com/application/web3/faucet/ethereum/holesky
2. Ingresar la direccion del GAS_SPONSOR
3. Recibir 0.05 ETH/dia

### Opcion B: Holesky Faucet Oficial

1. Ir a: https://www.holeskyfaucet.io/
2. Pegar la direccion del GAS_SPONSOR
3. Recibir ETH (cantidad variable)

### Opcion C: Mining Faucet (mas ETH, requiere tiempo)

1. Ir a: https://holesky-faucet.pk910.de/
2. Introducir la direccion del GAS_SPONSOR
3. Dejar la pestana abierta minando (PoW)
4. Reclamar los ETH acumulados

### Verificar balance

```bash
source .env
cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_17000

# Ejemplo de salida:
# 500000000000000000  (0.5 ETH en wei)
```

### Enviar ETH a otra direccion (si es necesario)

```bash
# Enviar 0.1 ETH desde el gas sponsor a otra direccion
cast send <DESTINO> \
  --value 0.1ether \
  --rpc-url $RPC_HTTP_17000 \
  --private-key $GAS_SPONSOR_PRIVATE_KEY
```

---

## 4. Arbitrum Sepolia (ETH)

**IMPORTANTE:** Arbitrum Sepolia es una L2. El gas se paga en ETH, pero hay dos formas de obtenerlo:

### Opcion A: Bridge desde Sepolia (L1 -> L2)

**Paso 1 — Fundar Sepolia ETH (L1):**

1. Ir a: https://cloud.google.com/application/web3/faucet/ethereum/sepolia
2. O: https://sepoliafaucet.com/ (Alchemy, requiere cuenta)
3. Ingresar la direccion del GAS_SPONSOR
4. Recibir 0.5 ETH de Sepolia

**Paso 2 — Bridge a Arbitrum Sepolia:**

1. Ir a: https://bridge.arbitrum.io/
2. Seleccionar: Source = Sepolia, Destination = Arbitrum Sepolia
3. Conectar wallet (Metamask con la red Sepolia)
4. Ingresar cantidad a bridgear (recomendado: 0.2 ETH)
5. Confirmar transaccion
6. Esperar 10-15 minutos

### Opcion B: Faucet Directo de L2 (mas rapido)

1. Ir a: https://faucet.quicknode.com/arbitrum/sepolia
2. Ingresar la direccion del GAS_SPONSOR
3. Recibir ETH en Arbitrum Sepolia directamente

### Opcion C: L2 Faucet Alternativo

1. Ir a: https://www.l2faucet.com/arbitrum
2. Ingresar la direccion del GAS_SPONSOR
3. Recibir ETH

### Verificar balance

```bash
source .env
cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_421614

# Ejemplo de salida:
# 300000000000000000  (0.3 ETH en wei)
```

---

## 5. Polygon Amoy (MATIC)

### Opcion A: Faucet Oficial de Polygon (Recomendado)

1. Ir a: https://faucet.polygon.technology/
2. Seleccionar la red "Amoy"
3. Ingresar la direccion del GAS_SPONSOR
4. Recibir 1 MATIC (limite: 1 vez cada 24h por IP)

### Opcion B: Alchemy Faucet

1. Ir a: https://www.alchemy.com/faucets/polygon-amoy
2. Crear cuenta gratuita en Alchemy (si no tienes)
3. Ingresar la direccion del GAS_SPONSOR
4. Recibir hasta 5 MATIC/dia

### Opcion C: ThirdWeb Faucet

1. Ir a: https://thirdweb.com/amoy
2. Conectar wallet
3. Recibir 0.1 MATIC

### Verificar balance

```bash
source .env
cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_80002

# Ejemplo de salida:
# 5000000000000000000  (5 MATIC en wei)
```

---

## 6. Verificacion de Balances

### Script de verificacion rapida

```bash
#!/bin/bash
source .env

echo "=== CRUCIBLE BALANCE CHECK ==="
echo "GAS_SPONSOR: $GAS_SPONSOR_ADDRESS"
echo ""

echo "Holesky:"
BAL17000=$(cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_17000 2>/dev/null)
if [ $? -eq 0 ]; then
  BAL_ETH=$(cast from-wei $BAL17000)
  echo "  $BAL_ETH ETH"
  if [ $(echo "$BAL_ETH > 0.01" | bc) -eq 1 ]; then
    echo "  [OK] Suficiente para deploy"
  else
    echo "  [WARN] Balance bajo. Fundar via faucet."
  fi
else
  echo "  [ERROR] No se pudo consultar balance"
fi

echo ""
echo "Arbitrum Sepolia:"
BAL421614=$(cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_421614 2>/dev/null)
if [ $? -eq 0 ]; then
  BAL_ETH=$(cast from-wei $BAL421614)
  echo "  $BAL_ETH ETH"
  if [ $(echo "$BAL_ETH > 0.01" | bc) -eq 1 ]; then
    echo "  [OK] Suficiente para deploy"
  else
    echo "  [WARN] Balance bajo. Fundar via faucet."
  fi
else
  echo "  [ERROR] No se pudo consultar balance"
fi

echo ""
echo "Polygon Amoy:"
BAL80002=$(cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_80002 2>/dev/null)
if [ $? -eq 0 ]; then
  BAL_MATIC=$(cast from-wei $BAL80002)
  echo "  $BAL_MATIC MATIC"
  if [ $(echo "$BAL_MATIC > 0.1" | bc) -eq 1 ]; then
    echo "  [OK] Suficiente para deploy"
  else
    echo "  [WARN] Balance bajo. Fundar via faucet."
  fi
else
  echo "  [ERROR] No se pudo consultar balance"
fi
```

Guardar como `check_balances.sh` y ejecutar:
```bash
chmod +x check_balances.sh
./check_balances.sh
```

---

## 7. Troubleshooting

### Problema: "RPC error: rate limited"

**Solucion:** Los RPCs publicos tienen rate limits. Opciones:
1. Esperar y reintentar
2. Usar RPC alternativo del .env.crucible
3. Crear cuenta en Alchemy/Infura para RPC propio (gratis)

```bash
# Ejemplo con Alchemy (gratis)
# Crear proyecto en https://dashboard.alchemy.com/
# Obtener URL del RPC y usarla:
export RPC_HTTP_17000=https://eth-holesky.g.alchemy.com/v2/TU_API_KEY
```

### Problema: "insufficient funds for gas"

**Causa:** Balance demasiado bajo para cubrir el gas del deploy.

**Solucion:**
```bash
# Verificar balance exacto
cast balance $GAS_SPONSOR_ADDRESS --rpc-url <RPC>

# Fundar via faucet (ver secciones 3-5)
```

### Problema: "nonce too low" o "replacement transaction underpriced"

**Causa:** Transaccion pendiente o nonce desincronizado.

**Solucion:**
```bash
# Verificar nonce pendiente
cast tx-count $GAS_SPONSOR_ADDRESS --rpc-url <RPC> --pending

# Esperar a que se confirme la transaccion pendiente
# O reiniciar el nonce con --nonce flag
```

### Problema: Faucet no funciona / rate limited

**Solucion:** Probar multiples faucets alternativos:

| Red | Faucet 1 | Faucet 2 | Faucet 3 |
|-----|----------|----------|----------|
| Holesky | Google Cloud | holeskyfaucet.io | pk910.de |
| Arbitrum Sepolia | QuickNode | L2Faucet | Bridge desde Sepolia |
| Polygon Amoy | polygon.technology | Alchemy | ThirdWeb |

### Problema: "cannot connect to RPC"

**Solucion:**
```bash
# Probar conectividad
curl -X POST $RPC_HTTP_17000 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'

# Salida esperada: {"jsonrpc":"2.0","id":1,"result":"0x4268"} (17000 en hex)
```

---

## Checklist Pre-Deploy

- [ ] GAS_SPONSOR_ADDRESS configurada en .env
- [ ] EXECUTION_SIGNER_ADDRESS configurada en .env
- [ ] COLD_TREASURY_ADDRESS configurada en .env
- [ ] GAS_SPONSOR_PRIVATE_KEY configurada en .env
- [ ] Holesky: balance > 0.01 ETH
- [ ] Arbitrum Sepolia: balance > 0.01 ETH
- [ ] Polygon Amoy: balance > 0.1 MATIC
- [ ] ExecutionSigner balance = 0 en las 3 redes
- [ ] .env NO esta commiteado en git

---

*Documento generado para OMEGA CRUCIBLE — Testnet Deployment Phase*
