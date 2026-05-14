# OMEGA CRUCIBLE — Checklist de Validacion (15 Puntos)

> **Version:** 1.0  
> **Fase:** Validacion antes de Mainnet  
> **Redes:** Holesky (17000) | Arbitrum Sepolia (421614) | Polygon Amoy (80002)  
> **Objetivo:** Confirmar que todo el sistema esta listo para deploy en mainnet

---

## Instrucciones

1. Ejecutar cada punto en orden.
2. Marcar `[x]` solo cuando el punto este **completamente verificado**.
3. Documentar tx hashes, addresses y metricas en los campos provistos.
4. Si algun punto FALLA, NO proceder al siguiente hasta resolverlo.
5. Al completar los 15 puntos, el Crucible esta listo para mainnet.

---

## Checklist

### 1. Fuzzing Local 10K Ciclos

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] EN PROGRESO &nbsp;&nbsp; [ ] PASS &nbsp;&nbsp; [ ] FAIL

```bash
make fuzz
# o manualmente:
forge test --fuzz-runs 10000 -vvv
```

**Resultado esperado:** Todos los tests pasan. 0 reverts inesperados.

**Log:**
```
Runs: 10000/10000
Reverts: 0 (expected)
Time: ___ minutos
```

**Notas:**

---

### 2. Fondeo Holesky

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

```bash
cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_17000
```

**Balance:** ___ ETH  
**Minimo requerido:** 0.01 ETH  
**Faucet usado:** ___  
**Fecha/hora:** ___

**Notas:**

---

### 3. Fondeo Arbitrum Sepolia

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

```bash
cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_421614
```

**Balance:** ___ ETH  
**Minimo requerido:** 0.01 ETH  
**Faucet usado:** ___  
**Fecha/hora:** ___

**Notas:**

---

### 4. Fondeo Polygon Amoy

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

```bash
cast balance $GAS_SPONSOR_ADDRESS --rpc-url $RPC_HTTP_80002
```

**Balance:** ___ MATIC  
**Minimo requerido:** 0.1 MATIC  
**Faucet usado:** ___  
**Fecha/hora:** ___

**Notas:**

---

### 5. Deploy Factory en Holesky

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

```bash
make deploy-holesky
```

**Factory address:** `0x________________________________________`  
**Deploy tx hash:** `0x________________________________________________________`  
**Gas usado:** ___  
**Block:** ___  
**Timestamp:** ___

**Verificacion:**
```bash
# Verificar que el contrato existe
cast code <FACTORY_ADDRESS> --rpc-url $RPC_HTTP_17000
# Debe retornar bytecode (no 0x)
```

**Notas:**

---

### 6. Deploy Factory en Arbitrum Sepolia

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

```bash
make deploy-arbitrum-sepolia
```

**Factory address:** `0x________________________________________`  
**Deploy tx hash:** `0x________________________________________________________`  
**Gas usado:** ___  
**Block:** ___  
**Timestamp:** ___

**Notas:**

---

### 7. Deploy Factory en Amoy

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

```bash
make deploy-amoy
```

**Factory address:** `0x________________________________________`  
**Deploy tx hash:** `0x________________________________________________________`  
**Gas usado:** ___  
**Block:** ___  
**Timestamp:** ___

**Notas:**

---

### 8. WalletTopology Deployed (3 Testnets)

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

| Red | WalletTopology Address | Tx Hash |
|-----|----------------------|---------|
| Holesky | `0x____________________________` | `0x________________________________` |
| Arbitrum Sepolia | `0x____________________________` | `0x________________________________` |
| Polygon Amoy | `0x____________________________` | `0x________________________________` |

**Verificacion post-deploy:**
```bash
# Leer topologia de cada red
cast call <WALLET_TOPOLOGY_ADDRESS> "getTopology()(address,address,address)" --rpc-url $RPC_HTTP_17000
cast call <WALLET_TOPOLOGY_ADDRESS> "getTopology()(address,address,address)" --rpc-url $RPC_HTTP_421614
cast call <WALLET_TOPOLOGY_ADDRESS> "getTopology()(address,address,address)" --rpc-url $RPC_HTTP_80002
```

**Resultado esperado:**
- gasSponsor = GAS_SPONSOR_ADDRESS
- executionSigner = EXECUTION_SIGNER_ADDRESS
- coldTreasury = COLD_TREASURY_ADDRESS

**Notas:**

---

### 9. CREATE2 Addresses Match Predicted

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

> **Nota:** En testnet, los contratos se deployan directamente (no via CREATE2 del factory), ya que las testnet chain IDs no estan en la lista de soportadas por DeterministicFactory. Este punto verifica que las direcciones deployadas son correctas y que el factory funciona correctamente para prediccion.

**Verificacion:**
```bash
# Predecir direccion y comparar con la deployada
# Para WalletTopology en Holesky:
# (ejecutar en cast o via forge script --sig "predictAddress(...)")
```

**Direccion predicha:** `0x________________________________________`  
**Direccion deployada:** `0x________________________________________`  
**Match:** [ ] SI &nbsp;&nbsp; [ ] NO

**Notas:**

---

### 10. ExecutionSigner Balance = 0

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

**Verificacion:**
```bash
echo "Holesky:"
cast balance $EXECUTION_SIGNER_ADDRESS --rpc-url $RPC_HTTP_17000
echo "Arbitrum Sepolia:"
cast balance $EXECUTION_SIGNER_ADDRESS --rpc-url $RPC_HTTP_421614
echo "Polygon Amoy:"
cast balance $EXECUTION_SIGNER_ADDRESS --rpc-url $RPC_HTTP_80002
```

| Red | Balance Esperado | Balance Real | OK? |
|-----|-----------------|--------------|-----|
| Holesky | 0 | ___ | [ ] |
| Arbitrum Sepolia | 0 | ___ | [ ] |
| Polygon Amoy | 0 | ___ | [ ] |

**Notas:**

---

### 11. 50 Simulaciones E2E

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] EN PROGRESO &nbsp;&nbsp; [ ] COMPLETE

**Script:**
```bash
# Ejecutar simulaciones E2E (usar forge test o script custom)
# Este punto requiere un script de simulacion adicional que ejecute
# escenarios end-to-end del protocolo.

# Ejemplo:
forge test --match-test "testE2E" -vvv --fuzz-runs 50
```

| Metrica | Valor |
|---------|-------|
| Simulaciones ejecutadas | ___/50 |
| Exitosas | ___ |
| Fallidas | ___ |
| Reverts esperados | ___ |
| Reverts inesperados | ___ |
| Tiempo total | ___ min |

**Notas:**

---

### 12. Latencia < 2s Promedio

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

**Medicion:**
```bash
# Medir latencia de RPCs
for rpc in "$RPC_HTTP_17000" "$RPC_HTTP_421614" "$RPC_HTTP_80002"; do
  echo "Testing $rpc"
  for i in {1..10}; do
    time curl -s -X POST "$rpc" \
      -H "Content-Type: application/json" \
      -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
      > /dev/null
  done
done
```

| Red | Latencia Promedio | < 2s? |
|-----|------------------|-------|
| Holesky | ___ ms | [ ] SI [ ] NO |
| Arbitrum Sepolia | ___ ms | [ ] SI [ ] NO |
| Polygon Amoy | ___ ms | [ ] SI [ ] NO |

**Notas:**

---

### 13. No Reverts Inesperados en Logs

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

**Revisar logs de deploy:**
```bash
# Revisar los logs de forge script ejecutados
# Buscar: revert, failed, error, panic

grep -i "revert\|failed\|error\|panic" out/deployment_crucible_*.json 2>/dev/null || echo "No deployment logs found"
```

**Reverts encontrados:**
- [ ] Ninguno (OK)
- [ ] ___ reverts esperados (documentar)
- [ ] ___ reverts inesperados (INVESTIGAR)

**Notas:**

---

### 14. Paper Mode ON

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] N/A

**Verificacion:**
```bash
# Paper mode = modo simulacion sin transacciones reales
# Verificar que la configuracion de paper trading esta activa
# en los archivos de configuracion del operador

# Verificar variable de entorno
echo $PAPER_MODE

# O verificar en config:
grep -i "paper" .env 2>/dev/null || echo "Paper mode not configured in .env"
```

**Estado esperado:** `PAPER_MODE=true`  
**Estado real:** ___

**Notas:**

---

### 15. Ghost Protocol Remains ENABLED

**Status:** [ ] PENDIENTE &nbsp;&nbsp; [ ] OK &nbsp;&nbsp; [ ] FAIL

**Verificacion:**
```bash
# Ghost Protocol = protocolo de seguridad que oculta/mascara
# las transacciones reales durante fase de validacion
# Verificar que esta habilitado en configuracion

echo $GHOST_PROTOCOL_ENABLED

# O verificar en codigo/config:
grep -i "ghost" .env 2>/dev/null || echo "Ghost protocol not configured in .env"
```

**Estado esperado:** `GHOST_PROTOCOL_ENABLED=true`  
**Estado real:** ___

**Notas:**

---

## Resumen Ejecutivo

| Punto | Item | Status | Evidencia |
|-------|------|--------|-----------|
| 1 | Fuzzing 10K | [ ] | |
| 2 | Fondeo Holesky | [ ] | |
| 3 | Fondeo Arbitrum Sepolia | [ ] | |
| 4 | Fondeo Amoy | [ ] | |
| 5 | Deploy Factory Holesky | [ ] | |
| 6 | Deploy Factory Arbitrum Sepolia | [ ] | |
| 7 | Deploy Factory Amoy | [ ] | |
| 8 | WalletTopology Deployed | [ ] | |
| 9 | CREATE2 Match Predicted | [ ] | |
| 10 | ExecutionSigner Balance = 0 | [ ] | |
| 11 | 50 Simulaciones E2E | [ ] | |
| 12 | Latencia < 2s | [ ] | |
| 13 | No Reverts Inesperados | [ ] | |
| 14 | Paper Mode ON | [ ] | |
| 15 | Ghost Protocol ENABLED | [ ] | |

**Resultado Global:**
- [ ] **CRUCIBLE COMPLETE** — Listo para mainnet
- [ ] **CRUCIBLE INCOMPLETE** — Requiere trabajo adicional

**Observaciones:**

---

**Ejecutado por:** _________________  
**Fecha:** _________________  
**Revisado por:** _________________  
**Fecha revision:** _________________

---

*Documento generado para OMEGA CRUCIBLE — Testnet Validation Phase*
