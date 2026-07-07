# MEV-Share Backrun Searching

## Propósito
Detectar y explotar eventos en redes de orderflow privado (Flashbots MEV-Share) usando hints ofuscados.

## Conocimiento esencial
A diferencia del mempool público, MEV-Share emite eventos SSE (Server-Sent Events) con información parcial (hints como el pair address, o solo un log). El Searcher debe "adivinar" ciegamente la dirección y tamaño de la transacción del usuario para backrunnearla rentablemente, optimizando el bid a ciegas.
