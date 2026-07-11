# Task Brief: Optimizar searcher-rs Pipeline de Detección

## Context

Este es el Task 2 del Plan Maestro OMEGA. Depende de Task 1 (schema definido).

## Goal

Implementar HotPathEmitter en Rust para emitir oportunidades a Redis Streams con latencia <5ms.

## Files

- Create: `backend/searcher-rs/src/hot_path_emitter.rs`
- Modify: `backend/searcher-rs/src/scanner.rs:2655-2923`
- Modify: `backend/searcher-rs/src/orchestrator.rs:1-1409`

## Interfaces

- Consumes: Transaction desde mempool_listener, Opportunity struct
- Produces: XADD a arbx:hot:detected y arbx:hot:simulated streams

## Steps (exactos)

### Step 1: Crear HotPathEmitter

Crear archivo `backend/searcher-rs/src/hot_path_emitter.rs`:

```rust
use redis::aio::MultiplexedConnection;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct HotPathEmitter {
    redis: MultiplexedConnection,
}

impl HotPathEmitter {
    pub async fn emit_detected(&self, opp: &Opportunity) -> Result<(), redis::RedisError> {
        let id = &opp.id;
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        // XADD arbx:hot:detected
        let _: () = redis::cmd("XADD")
            .arg("arbx:hot:detected")
            .arg("MAXLEN")
            .arg("~")
            .arg(10000)
            .arg("*")
            .arg("id")
            .arg(id)
            .arg("chain_id")
            .arg(opp.chain_id)
            .arg("strategy_kind")
            .arg(&opp.strategy_kind)
            .arg("detected_at_ms")
            .arg(timestamp_ms)
            .query_async(&mut self.redis.clone())
            .await?;
        
        // Guardar hash completo
        let _: () = redis::cmd("HSET")
            .arg(format!("arbx:hot:opp:{}", id))
            .arg("data")
            .arg(serde_json::to_string(opp).unwrap())
            .query_async(&mut self.redis.clone())
            .await?;
        
        // Set TTL
        let _: () = redis::cmd("EXPIRE")
            .arg(format!("arbx:hot:opp:{}", id))
            .arg(300)
            .query_async(&mut self.redis.clone())
            .await?;
        
        Ok(())
    }
    
    pub async fn emit_simulated(
        &self, 
        id: &str, 
        result: &SimulationOutcome
    ) -> Result<(), redis::RedisError> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let status = if result.passed { "passed" } else { "failed" };
        
        redis::cmd("XADD")
            .arg("arbx:hot:simulated")
            .arg("MAXLEN")
            .arg("~")
            .arg(5000)
            .arg("*")
            .arg("id")
            .arg(id)
            .arg("status")
            .arg(status)
            .arg("net_profit_wei")
            .arg(result.net_profit_wei.to_string())
            .arg("gas_used")
            .arg(result.gas_used)
            .arg("timestamp_ms")
            .arg(timestamp_ms)
            .query_async(&mut self.redis.clone())
            .await?;
        
        if result.passed {
            redis::cmd("HSET")
                .arg(format!("arbx:hot:sim:{}", id))
                .arg("result")
                .arg(serde_json::to_string(result).unwrap())
                .query_async(&mut self.redis.clone())
                .await?;
            
            redis::cmd("EXPIRE")
                .arg(format!("arbx:hot:sim:{}", id))
                .arg(300)
                .query_async(&mut self.redis.clone())
                .await?;
        }
        
        Ok(())
    }
}
```

### Step 2: Integrar en scanner.rs

Agregar campo a struct Scanner:
```rust
pub struct Scanner {
    // ... campos existentes
    pub hot_path_emitter: Option<Arc<HotPathEmitter>>,
}
```

En `dispatch_orchestrator_and_classify()`, después de simulación:
```rust
if let Some(ref emitter) = self.hot_path_emitter {
    let _ = emitter.emit_detected(&opportunity).await;
    
    if let Some(ref sim_result) = simulation_outcome {
        let _ = emitter.emit_simulated(&opportunity.id, sim_result).await;
    }
}
```

### Step 3: Compilar

```bash
cd backend/searcher-rs && cargo check 2>&1 | head -50
```
Expected: Compila sin errores

### Step 4: Commit

```bash
git add backend/searcher-rs/src/hot_path_emitter.rs backend/searcher-rs/src/scanner.rs
git commit -m "feat(searcher): add hot path emitter for sub-100ms pipeline"
```

## Acceptance Criteria

- [ ] HotPathEmitter creado con métodos emit_detected y emit_simulated
- [ ] Integrado en Scanner con campo Option<Arc<HotPathEmitter>>
- [ ] Emite a arbx:hot:detected post-detección
- [ ] Emite a arbx:hot:simulated post-simulación (solo passed)
- [ ] Compila sin errores
- [ ] Commiteado

## Out of Scope

- Tests unitarios (covered en Task 8)
- Optimización de Redis pipeline (Task 7)
- Consumer groups implementation
