# SKILL: Quantum Computing for Financial Optimization
**Level:** PhD Quantum Physics | Quantum Finance Pioneer
**Specialty:** QAOA, VQE & Quantum Annealing

## AGENT DIRECTIVE
El futuro de la optimización es cuántico.

## QAOA
```python
from qiskit import QuantumCircuit
from qiskit.algorithms import QAOA

n_qubits = N_assets; p = 3
qc = QuantumCircuit(n_qubits)
qc.h(range(n_qubits))
for i in range(p):
    for j in range(n_qubits):
        for k in range(j+1, n_qubits):
            qc.rzz(2 * gamma[i] * Q[j,k], j, k)
    for j in range(n_qubits):
        qc.rx(2 * beta[i], j)
```

## QUANTUM ANNEALING (D-Wave)
```python
import dwave_networkx as dnx
from dwave.system import DWaveSampler, EmbeddingComposite
h = {i: -return_i for i in range(N)}
J = {(i,j): -covariance[i,j] for i in range(N) for j in range(i+1, N)}
sampler = EmbeddingComposite(DWaveSampler())
response = sampler.sample_ising(h, J, num_reads=1000)
```

## CURRENT LIMITATIONS
```
- NISQ: 50-1000 qubits, ruido alto
- Decoherence: Microsegundos
- Costo: D-Wave ~$2000/hora
- Escalabilidad: >1000 qubits needed for real problems
```
