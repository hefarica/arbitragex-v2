# SKILL: Hardware Acceleration: FPGA & GPU for Trading
**Level:** PhD Computer Engineering | Hardware Acceleration Architect
**Specialty:** FPGA Design & GPU Parallel Computing

## AGENT DIRECTIVE
Cuando necesites **nanosegundos**, usa FPGA. Cuando necesites **paralelismo masivo**, usa GPU.

## FPGA FOR HFT
```verilog
// Order book reconstruction en FPGA
// Latencia: 50-200ns vs 5-50μs en CPU
module order_book (
    input wire clk,
    input wire [63:0] message,
    output reg [31:0] best_bid,
    output reg [31:0] best_ask
);
// State machine para parsear ITCH messages
// Binary search tree en hardware
endmodule
```

## GPU FOR ML
```cuda
__global__ void calculate_rsi(float* prices, float* rsi, int n, int period) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < n - period) {
        float gains = 0, losses = 0;
        for (int i = 0; i < period; i++) {
            float change = prices[idx + i + 1] - prices[idx + i];
            if (change > 0) gains += change;
            else losses -= change;
        }
        float rs = (gains/period) / (losses/period);
        rsi[idx] = 100.0f - (100.0f / (1.0f + rs));
    }
}
// Throughput: 10,000+ símbolos en < 1ms
```

## HYBRID ARCHITECTURE
```
FPGA: Network + Order book + Pre-filtering (nanoseconds)
CPU: Strategy + Risk management (microseconds)
GPU: ML inference + Simulation (milliseconds)
Total latency: ~67μs (vs 500μs+ pure software)
```
