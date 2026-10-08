CRITERIO DECLARADO ANTES DE CONTAR — t177 UNIVERSE-SWEEP-01, intento 2
Intento 91908449-fd7c-49ca-a799-21ab4a414047.

=== QUE FALLO EN EL INTENTO 1, Y QUE SE CORRIGE ACA ===
1. MI TRUNCADO: el script guardo `lista_0_35[:50]`. Con 245 combos en la lista, 195 quedaron
   fuera del JSON y NO se conservaron. La banda [0,35 % - 35,87 %] —la que decide— se perdio.
   => EN ESTE INTENTO SE ESCRIBE LA LISTA COMPLETA Y EL HISTOGRAMA POR BANDAS. Sin truncar.
2. NO CORRI LA VALIDACION POR PRECIO MARGINAL (4/4 = 1,000000), que el contrato exige.
   => SE CORRE, y su resultado se reporta aunque falle.
3. El primer barrido dio `0 de 1.336` porque `localhost:8545` NO esta publicado al host
   (http=000 exit_curl=7). El endpoint correcto es la IP del contenedor.
   => YA CORREGIDO; se re-verifica antes de barrer.

=== EL PROBLEMA QUE HAY QUE RESOLVER, DICHO ANTES DE MIRAR ===
En el intento 1 los 50 mayores spreads fueron TODOS >= 35,87 %, con maximo 4,05e31 %, y
36 de 50 por encima del 100 %. Eso NO son spreads de mercado: son pools DEGENERADOS
(reservas dust, token falso, o decimal distinto del declarado).
=> Los conteos crudos (245 / 206) estan contaminados y NO responden la pregunta.
=> Hace falta un criterio que separe "mercado" de "pool roto" ANTES de contar.

=== EL CRITERIO, DECLARADO ANTES DE VER CUANTAS FILAS REMUEVE ===
CRITERIO DE CONSENSO POR PAR (no un umbral de tamano, y no un umbral elegido a ojo):
  Para cada par de tokens con >= 2 pools, se calcula la MEDIANA de los precios de todos
  los pools de ese par. Un pool es DEGENERADO si su precio se aparta de esa mediana por
  mas de un factor 10 (en cualquiera de los dos sentidos).
  Un combo (par de pools del mismo par) es CANDIDATO VALIDO solo si NINGUNO de los dos
  pools es degenerado.

POR QUE ESTE CRITERIO Y NO OTRO:
  - No elige un umbral de spread. Elige un criterio de COHERENCIA: si todos los venues de
    un par dicen que el precio es ~X y uno dice 1e-30*X, el que discrepa esta roto.
  - El factor 10 es deliberadamente GENEROSO: un pool puede estar 10x fuera de la mediana
    por un movimiento real de mercado y seguir contando. Solo se descarta lo imposible.
  - Es SIMETRICO y no depende de cual pool se mira primero.
  - NO se ajusta despues. Si el factor 10 resulta malo, se declara y se re-corre el barrido
    ENTERO con el nuevo valor, no se retoca el resultado.

Y ADEMAS, declarado por separado para que se vea CADA paso:
  (a) se reportan los conteos CRUDOS (sin filtro) tal como salen;
  (b) se reportan los conteos tras el filtro de degeneracion;
  (c) se declara cuantos combos removio el filtro y de que banda venian.
  Los tres numeros juntos. No se elige el que conviene.

=== LOS UMBRALES DE LA PREGUNTA (de t175, NO se inventan) ===
  > 0,35 %  para vencer el fee del round-trip
  > 0,68 %  para vencer gas a tamano pequeno

=== LO QUE NO VOY A HACER ===
1. NO voy a presentar el conteo crudo como respuesta (el intento 1 demostro que esta
   contaminado).
2. NO voy a presentar un cero que no medi.
3. NO voy a estimar "cuanto se podria ganar": no es computable con lo medido.
4. NO voy a citar el blob c6b99de1 de PR #902: no accedi a el en el intento 1 y si no lo
   verifico, no lo cito.
5. NO voy a extrapolar: la cobertura se declara en la misma linea que cada conclusion.
