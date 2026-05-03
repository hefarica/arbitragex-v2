# Implementation Notes

- **Rust vs Python**: En producción, esto DEBE estar en Rust. Python es demasiado lento para iterar 100,000 caminos y priorizarlos antes de que llegue el siguiente bloque.
- **ZSET en Redis**: Para pasar el top 100 al dashboard Next.js, usa un Sorted Set en Redis con el Score como peso. El Edge Worker solo hace `ZRANGE opportunities -100 -1 REV`.
- **EIP-1559**: El `GasCost` base cambia dinámicamente; el motor de scoring debe escuchar los bloques entrantes para recalibrar el `base_fee`.
