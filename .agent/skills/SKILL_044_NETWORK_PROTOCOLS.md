# SKILL: Network Protocols: FIX, WebSocket, gRPC & Binary
**Level:** PhD Network Engineering | Protocol Optimization Expert
**Specialty:** Low-Latency Communication & Binary Serialization

## AGENT DIRECTIVE
El protocolo es tu tubería. Cada byte cuenta. Usa **protocolos binarios**, rechaza JSON.

## CORE KNOWLEDGE
- **FIX:** Financial Information eXchange
- **FAST:** FIX comprimido para alta frecuencia
- **ITCH/OUCH:** NASDAQ binary protocols
- **SBE:** Simple Binary Encoding
- **gRPC:** HTTP/2 + Protocol Buffers

## FIX OPTIMIZATION
```
FIX Binary (SBE): Reduce size 50-70%
FAST: Template-based compression
Session-level: Persist sequence numbers
```

## BINARY PROTOCOLS (HFT)
```cpp
// NASDAQ ITCH: 36 bytes per message
struct OrderAdd {
    uint16_t message_type;
    uint16_t stock_locate;
    uint64_t timestamp;
    uint64_t order_reference_number;
    char buy_sell_indicator;
    uint32_t shares;
    char stock[8];
    uint32_t price;
} __attribute__((packed));
```

## ZERO COPY (DPDK)
```cpp
// Packet llega a user-space sin kernel
struct rte_mbuf *pkts[BURST_SIZE];
uint16_t nb_rx = rte_eth_rx_burst(port_id, queue_id, pkts, BURST_SIZE);
// Latency: <1μs desde NIC hasta application
```
