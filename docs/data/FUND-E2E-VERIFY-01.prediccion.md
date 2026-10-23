PREDICCION FALSABLE — t156 FUND-E2E-VERIFY-01
Escrita ANTES de mirar cualquier dato. Intento 0618815e-7de8-4191-953b-3a4eb5ade29b.

=== LO QUE VOY A MEDIR ===
El arreglo del calldata de `balanceOf` (24 B -> 36 B) aterrizo en main como merge
77b42b3dccc001455fda3e8d4437d973c9e98c4b (PR #879). El blob de
backend/sim-ctl/src/signer_funding.rs en main es 6b5fb633f4cdd7547a8bb9ae31f2386cb4d2d734.

LINEA BASE PRE-ARREGLO (medida por el instrumento sobre 901eb947, el PADRE, SIN el arreglo):
    SAMPLE_REASON_DISTRIBUTION total=25
        sim_failed:sim_signer_funding_slot_unresolved=7
        no_simulation_row=18
Las 7 con fila de simulacion fallan TODAS por sim_signer_funding_slot_unresolved.

=== LA PREDICCION, EN UNA LINEA ===
Si el arreglo funciona, `sim_signer_funding_slot_unresolved` tiene que DROPEAR.

=== COMO SE FALSIFICA ===
- DROPEA a 0 (o baja de forma clara): la prediccion se sostiene, el hilo del fondo
  se cierra con medicion end-to-end.
- NO DROPEA (queda en 7, o sube, o se mueve a otra razon): la prediccion es FALSA.
  Eso significa que hay OTRA causa en el camino del fondeo, y ESE es el hallazgo.
  Se reporta crudo. NO se maquilla con "el instrumento cambio" ni con una ventana
  distinta.

=== LO QUE NO DEBO CONFUNDIR (declarado antes, para no elegir despues) ===
1. `no_simulation_row` = NO MEDIDA, no "fallo". Si sim_failed baja y no_simulation_row
   sube, hay que dar LOS DOS numeros. Prohibido elegir el que conviene.
2. El benchmark NO pasa porque el fondo funcione: su poblacion sigue siendo rechazos.
   `samples_labeled` se reporta AUNQUE SEA 0. Confundir "el fondo funciona" con
   "el benchmark pasa" es el error que este proyecto paga desde F03.
3. Un cambio en la PARTICION es la evidencia del mecanismo. El mismo conteo con
   nombres distintos NO lo es.
4. `write_rejected` esta AUSENTE (el anvil_setStorageAt nunca falla). El arreglo
   cambia la LECTURA, no la escritura.
5. La tabla es viva y el historico domina: hace falta CORTE TEMPORAL antes/despues
   del deploy. Sin corte, un agregado sobre la tabla entera es ilegible.

=== LO QUE ESPERO VER, DICHO ANTES ===
Espero que el deploy haya cerrado en success sobre 77b42b3d y que el runtime sirva
77b42b3d. Si el runtime todavia sirve el padre (901eb947), NO se mide: se declara.

=== ESTADO DEL BENCHMARK, PREDICHO ===
Predigo que el benchmark sigue en FAIL con samples_labeled=0. El arreglo del fondo
no cambia su veredicto. Si sale PASS, eso seria un hallazgo y habria que explicarlo.
