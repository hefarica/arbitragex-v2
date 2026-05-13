# SKILL: High-Frequency Trading System Architecture
**Level:** PhD Computer Engineering | Low-Latency Systems Architect
**Specialty:** Microsecond-Level System Design & Fault Tolerance

## AGENT DIRECTIVE
Construye sistemas que no fallen. En HFT, un microsegundo de downtime es una eternidad. La arquitectura es tu **fortaleza**. Diseña para la guerra, no para la paz.

## CORE KNOWLEDGE
- **Kernel Bypass:** DPDK, RDMA, FPGA NICs
- **Lock-Free Programming:** Atomics, memory barriers, cache coherence
- **NUMA Awareness:** Non-Uniform Memory Access optimization
- **Disruptor Pattern:** LMAX architecture, ring buffers
- **Fault Tolerance:** Graceful degradation, circuit breakers, bulkheads

## SYSTEM LAYERS
```
Layer 1: Network (Sub-microsecond)
- FPGA NICs (Solarflare/Mellanox)
- DPDK for kernel bypass
- RDMA for zero-copy transfer
- PTP for clock synchronization (<100ns accuracy)

Layer 2: Kernel (Microsecond)
- Real-time Linux (PREEMPT_RT)
- CPU isolation (isolcpus)
- Disable C-states, P-states (determinismo)
- Huge pages (2MB/1GB) para TLB efficiency

Layer 3: Application (5-50 microseconds)
- Lock-free data structures
- Pre-allocated memory pools
- Custom allocators (no malloc/free en hot path)
- Cache-line alignment (64 bytes)

Layer 4: Strategy (50-500 microseconds)
- Deterministic execution time
- Worst-case execution time (WCET) analysis
- Branch prediction friendly code
- SIMD instructions (AVX-512)
```

## FAULT TOLERANCE PATTERNS
```
1. Circuit Breaker:
   - Si error rate > threshold: Open circuit
   - After timeout: Half-open (test limited traffic)
   - Prevents cascade failures

2. Bulkhead:
   - Isolar recursos por estrategia
   - Si una estrategia consume todo CPU, las otras siguen

3. Kill Switch:
   - Hardware/software button para detener TODO trading
   - Auto-activation si P&L < -5% en 1 minuto
```

## MONITORING
```
- Latency p99 > 100μs: WARNING
- Latency p99 > 500μs: CRITICAL
- Error rate > 1%: CRITICAL
- Drawdown > 10%: EMERGENCY (kill switch)
```
