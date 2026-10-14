# ============================================================================
# check-feature-producers.ps1 -- MARKET-FEATURES-01 / tarea F4
#
# GATE ANTI-FABRICACION sobre `MarketState.features`.
#
# Recorre el codigo de PRODUCCION (excluye `tests/`, `*_test.rs`, `*tests.rs` y
# todo lo que cuelga de un `#[cfg(test)] mod`) y verifica que el mapa de
# features nunca degrade una AUSENCIA a un valor fabricado.
#
# REGLAS
#   R1  `features.get("<k>")` en produccion cuya expresion se completa con
#       `unwrap_or(<literal numerico>)` o `unwrap_or_default()`.
#       -> la ausencia se vuelve un numero silencioso.
#   R2  `insert(...)` en produccion sobre una clave del universo de features
#       cuyo VALOR es un literal numerico.
#       -> un productor que fabrica una constante en lugar de medir.
#   R3  Una clave leida en produccion y NO declarada en el manifiesto.
#       -> obliga a declarar el contrato antes de leer, no despues.
#   R4  Una clave declarada SIN PRODUCTOR (ausencia honesta) para la que aparece
#       un insert en produccion.
#   R5  `arbitrage_gap` con un lector real pese a estar declarada NO_CONSUMER.
#
# RATCHET (por que el gate puede estar verde con deuda declarada)
#   Cada violacion existente esta clavada en $Ledger por `archivo:linea` EXACTO,
#   no por clave. Consecuencias, todas verificadas por el propio gate:
#     * una violacion NUEVA o MOVIDA  -> BLOCKING (falla)
#     * una violacion del ledger      -> DEBT (se reporta, no falla) si su
#                                        `blocking=false`, o BLOCKING si es true
#     * una entrada del ledger que ya no ocurre -> STALE (hay que borrarla)
#   El ledger solo puede ENCOGER: no hay amnistia por clave.
#
# USO
#   pwsh -File check-feature-producers.ps1 -Root <raiz-del-repo>
#   pwsh -File check-feature-producers.ps1 -Root <raiz> -Json <ruta.json>
#   pwsh -File check-feature-producers.ps1 -SelfTest    # prueba de mutacion
#
# EXIT  0 = PASS (sin violaciones blocking) · 1 = FAIL · 2 = error de uso
# ============================================================================
[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
    [string]$Json = '',
    [switch]$SelfTest
)

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# MANIFIESTO -- la tabla de verdad como DATOS, medida sobre el arbol.
#   PRODUCED               productor escrito y cableado al camino vivo
#   PRODUCED_PENDING_WIRE  productor escrito, conexion pendiente (zona caliente)
#   ABSENT                 hay lectores y NO hay productor: la clave queda fuera
#   NO_CONSUMER            nadie la lee: producirla seria cobertura aparente
# `producer` = archivo:linea del unico escritor autorizado, o '' si no hay.
# ---------------------------------------------------------------------------
$Manifest = @(
    @{ key='parity_deviation'; state='PRODUCED'; producer='backend/searcher-rs/src/math_evidence.rs:250';
       note='Redis arbx:token_prices:<chain>; FEATURES-01a (ya en main)' }
    @{ key='health_factor'; state='PRODUCED'; producer='backend/searcher-rs/src/orchestrator.rs:709';
       note='indexer de liquidacion cacheado; FEATURES-02 (ya en main)' }
    @{ key='volatility'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:158';
       note='clave via const VOLATILITY_KEY; productor NO cableado (zona caliente)' }
    @{ key='oracle_price'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:170';
       note='clave via const ORACLE_PRICE_KEY; ancla Chainlink del PriceBus' }
    @{ key='onchain_price'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:171';
       note='clave via const ONCHAIN_PRICE_KEY; par atomico con oracle_price' }
    @{ key='pool_fee'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:223';
       note='F7: par REAL (fee_units, fee_denominator) del edge — la MISMA lectura que snapshot_services.rs:345 ya consume; cableado pendiente (zona caliente)' }
    @{ key='flash_premium'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:228';
       note='F7: lectura ON-CHAIN del proveedor (Pool.FLASHLOAN_PREMIUM_TOTAL()); si falta, la clave se OMITE y se publica el requisito BLOCKED_EXTERNAL. Nunca 0.0 sin acreditacion' }
    @{ key='bayes_wins'; state='ABSENT'; producer=''; note='op_11 la lee' }
    @{ key='bayes_losses'; state='ABSENT'; producer=''; note='op_11 la lee' }
    @{ key='bayes_prior_alpha'; state='ABSENT'; producer=''; note='op_11 la lee' }
    @{ key='bayes_prior_beta'; state='ABSENT'; producer=''; note='op_11 la lee' }
    @{ key='fee_bps'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:224';
       note='F7: el MISMO par del fee en la unidad de op_15/op_21/op_32 (bps, denominador 10_000 de cost_producers::funding); par atomico con pool_fee' }
    @{ key='eth_price_usd'; state='ABSENT'; producer=''; note='canonical_strategy la lee para valorar el gas en USD; sin productor' }
    @{ key='break_even_target'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:236';
       note='F7: min_profit_usd configurado -> unidades minimas del numerario ([token0 numerary], op_21:123)' }
    @{ key='token0_per_eth'; state='ABSENT'; producer=''; note='op_26 la lee para convertir unidades' }
    @{ key='gas_units'; state='ABSENT'; producer=''; note='op_15/op_21/op_26 la leen' }
    @{ key='decoherencia'; state='ABSENT'; producer=''; note='canonical_strategy la lee (penalizacion nunca dispara)' }
    @{ key='jit_decay_rate'; state='ABSENT'; producer=''; note='op_28 la lee' }
    @{ key='min_liquidity'; state='ABSENT'; producer=''; note='op_28 la lee' }
    @{ key='max_capital'; state='PRODUCED_PENDING_WIRE'; producer='backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:232';
       note='F7: capital_usd configurado -> unidades minimas del numerario (misma familia que b[1+j]=liquidity_reserves[j].0, op_19:188)' }
    @{ key='mo_weight_yield'; state='ABSENT'; producer=''; note='op_32 la lee' }
    @{ key='mo_weight_risk'; state='ABSENT'; producer=''; note='op_32 la lee' }
    @{ key='mo_weight_latency'; state='ABSENT'; producer=''; note='op_32 la lee' }
    @{ key='mo_per_leg_latency_ms'; state='ABSENT'; producer=''; note='op_32 la lee' }
    @{ key='nsga2.count'; state='ABSENT'; producer=''; note='op_32_nsga2 la lee' }
    @{ key='nsga2.population_size'; state='ABSENT'; producer=''; note='op_32_nsga2 la lee' }
    @{ key='arbitrage_gap'; state='NO_CONSUMER'; producer='';
       note='RegimeRouter la computa internamente desde pair_keys; sin lector en features' }
)
$ManifestKeys = $Manifest | ForEach-Object { $_.key }
$AbsentKeys   = $Manifest | Where-Object { $_.state -eq 'ABSENT' } | ForEach-Object { $_.key }

# Claves que ESTA mision produce. Una fabricacion en su camino de consumo es
# blocking: anularia el entregable en el consumidor.
$MissionKeys = @('volatility','oracle_price','onchain_price','parity_deviation','health_factor')

# ---------------------------------------------------------------------------
# LEDGER de deuda preexistente, clavada por archivo:linea EXACTO.
# ---------------------------------------------------------------------------
$Ledger = @(
    @{ loc='backend/math-engine/src/strategies/canonical_strategy.rs:131'; rule='R1'; blocking=$true
       reason='consumidor de `volatility` (clave de ESTA mision): sin dato fabrica 0.0 dentro de estimate_decoherencia, que devuelve Some(...) incondicionalmente. El patron correcto esta 53 lineas antes, en el MISMO archivo (:78 usa `if let Some(&vol)` con guarda is_finite). Fix requerido fuera del alcance de F4 (math-engine) -> registro de integracion.' }
    @{ loc='backend/math-engine/src/operators/op_11_bayes.rs:35'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): ausencia -> wins=0.0. Queda enmascarada aguas abajo por `wins+losses<1.0` -> reason_no_history, pero el mecanismo es implicito: cualquier prior inyectado la volveria un computo silencioso.' }
    @{ loc='backend/math-engine/src/operators/op_11_bayes.rs:36'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): idem para losses.' }
    @{ loc='backend/math-engine/src/operators/op_11_bayes.rs:39'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): el PRIOR alpha ausente se vuelve 1.0. Distinto de wins/losses: un prior Beta(1,1) uniforme es una DECISION DE MODELO, no una medicion fabricada — pero es un default silencioso que nadie declara, y si la clave falta porque la configuracion no cargo, 1.0 lo oculta. Debe declararse como prior por defecto explicito.' }
    @{ loc='backend/math-engine/src/operators/op_11_bayes.rs:44'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): idem para el PRIOR beta.' }
    @{ loc='backend/math-engine/src/strategies/canonical_strategy.rs:157'; rule='R1'; blocking=$true
       reason='FABRICACION MONETARIA: `estimate_gas_usd` valora el gas con un precio de ETH INVENTADO (2000.0 USD) cuando `eth_price_usd` esta ausente, y ese coste de gas alimenta la decision de rentabilidad. Viola RULE 00 y la doctrina §4 (f64 para importes). Fijado como BLOCKING: es un proxy inventado para un valor monetario, no un default de modelo.' }
    @{ loc='backend/math-engine/src/operators/op_15_golden_section.rs:46'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `fee_bps` fabrica 0.003 y sigue computando.' }
    @{ loc='backend/math-engine/src/operators/op_21_newton.rs:52'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `fee_bps` fabrica 0.003.' }
    @{ loc='backend/math-engine/src/operators/op_21_newton.rs:118'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `break_even_target` fabrica 0.0 y sigue computando.' }
    @{ loc='backend/math-engine/src/operators/op_32_multi_objective.rs:462'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `fee_bps` fabrica 0.003.' }
    @{ loc='backend/math-engine/src/operators/op_15_golden_section.rs:48'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `pool_fee` fabrica 0.003 (30 bps) y sigue computando.' }
    @{ loc='backend/math-engine/src/operators/op_19_simplex.rs:174'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `max_capital` fabrica 1.0 (un dolar) y sigue computando.' }
    @{ loc='backend/math-engine/src/operators/op_21_newton.rs:54'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `pool_fee` fabrica 0.003.' }
    @{ loc='backend/math-engine/src/operators/op_26_flash_loan.rs:60'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `pool_fee` fabrica 0.003.' }
    @{ loc='backend/math-engine/src/operators/op_26_flash_loan.rs:66'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `flash_premium` fabrica 0.0 (flash gratis).' }
    @{ loc='backend/math-engine/src/operators/op_32_multi_objective.rs:464'; rule='R1'; blocking=$false
       reason='deuda preexistente (math-engine): sin `pool_fee` fabrica 0.003.' }
)

# Receptores aceptados como "el mapa de features", por archivo. `out` es el
# nombre local del mapa en los dos productores reales; restringirlo por archivo
# evita que un `out.insert(...)` ajeno entre al censo.
$MapAliases = @{
    'backend/searcher-rs/src/math_evidence.rs' = @('out')
    'backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs' = @('out')
}
$NumericLiteral = '-?\d+(\.\d+)?([eE][-+]?\d+)?'

function Get-ProductionFiles {
    param([string]$RepoRoot)
    $backend = Join-Path $RepoRoot 'backend'
    if (-not (Test-Path $backend)) { throw "no existe $backend" }
    return (Get-ChildItem -Recurse -Path $backend -Filter '*.rs' -File | Where-Object {
        $rel = $_.FullName.Substring($RepoRoot.Length).Replace('\', '/')
        if ($rel -match '/tests?/') { return $false }
        if ($_.Name -match 'tests?\.rs$') { return $false }
        return $true
    })
}

# Rangos de linea ocupados por codigo de test, como [inicio, fin].
#
# NO se puede cortar "todo lo que sigue al primer #[cfg(test)]": en
# `market_features/mod.rs` la declaracion `#[cfg(test)] mod tests;` esta en la
# linea 81 y la IMPLEMENTACION de produccion sigue despues (158/170/171). Un
# corte ciego excluiria justo los productores que este gate debe vigilar.
# Casos:
#   * `#[cfg(test)] mod tests;`      -> fuera de linea: el rango es la pareja
#                                       de lineas de la declaracion; el archivo
#                                       de tests ya se excluye por nombre.
#   * `#[cfg(test)] mod tests { ... }` -> rango hasta la llave de cierre.
function Get-TestSpans {
    param([string[]]$Lines)
    # OJO: se devuelve con `,` porque PowerShell APLANA un array de arrays al
    # devolverlo por el pipeline (un `@(@(293,504))` llega como `293,504` y el
    # rango se pierde). Con `,$list` el receptor obtiene UN objeto-lista.
    $spans = [System.Collections.Generic.List[object]]::new()
    for ($i = 0; $i -lt $Lines.Count; $i++) {
        if ($Lines[$i] -notmatch '^\s*#\[cfg\(test\)\]\s*$') { continue }
        $k = $i + 1
        while ($k -lt $Lines.Count -and $Lines[$k].Trim() -eq '') { $k++ }
        if ($k -ge $Lines.Count) { $spans.Add([pscustomobject]@{ Start=$i; End=$i }); continue }
        if ($Lines[$k] -match '^\s*(pub\s+)?mod\s+\w+\s*;') {
            $spans.Add([pscustomobject]@{ Start=$i; End=$k }); continue
        }
        $b = $k
        while ($b -lt $Lines.Count -and $Lines[$b] -notmatch '\{') { $b++ }
        if ($b -ge $Lines.Count) { $spans.Add([pscustomobject]@{ Start=$i; End=$k }); continue }
        $depth = 0; $end = $b
        for ($j = $b; $j -lt $Lines.Count; $j++) {
            $depth += ([regex]::Matches($Lines[$j], '\{')).Count
            $depth -= ([regex]::Matches($Lines[$j], '\}')).Count
            if ($depth -le 0) { $end = $j; break }
        }
        $spans.Add([pscustomobject]@{ Start=$i; End=$end })
    }
    return , $spans
}

function Test-InTestZone {
    param($Spans, [int]$LineIndex)
    foreach ($s in $Spans) {
        if ($LineIndex -ge $s.Start -and $LineIndex -le $s.End) { return $true }
    }
    return $false
}

# Resuelve `const X: &str = "clave";` para que un productor que inserta via
# constante (VOLATILITY_KEY, ORACLE_PRICE_KEY, ONCHAIN_PRICE_KEY) no escape a un
# escaneo por literal.
function Get-StringConstMap {
    param($ProdFiles)
    $map = @{}
    foreach ($f in $ProdFiles) {
        foreach ($m in [regex]::Matches((Get-Content -Raw -LiteralPath $f.FullName),
                                        'const\s+(\w+)\s*:\s*&str\s*=\s*"([^"]+)"')) {
            $map[$m.Groups[1].Value] = $m.Groups[2].Value
        }
    }
    return $map
}

function Test-IsFeaturesReceiver {
    param([string]$Rel, [string]$Name)
    if ($Name -eq 'features') { return $true }
    if ($MapAliases.ContainsKey($Rel) -and $MapAliases[$Rel] -contains $Name) { return $true }
    return $false
}

function Invoke-Gate {
    param([string]$RepoRoot)

    $violations = New-Object System.Collections.Generic.List[object]
    $prodFiles  = Get-ProductionFiles -RepoRoot $RepoRoot
    $constMap   = Get-StringConstMap -ProdFiles $prodFiles

    $readSites   = @{}
    $prodInserts = @{}

    foreach ($f in $prodFiles) {
        # Sin '/' inicial: la ruta debe coincidir literalmente con la del ledger.
        $rel   = $f.FullName.Substring($RepoRoot.Length).Replace('\', '/').TrimStart('/')
        $lines = Get-Content -LiteralPath $f.FullName
        $testSpans = Get-TestSpans -Lines $lines

        for ($i = 0; $i -lt $lines.Count; $i++) {
            if (Test-InTestZone -Spans $testSpans -LineIndex $i) { continue }
            $line = $lines[$i]
            $loc  = "$rel`:$($i + 1)"
            if ($line -match '^\s*(//|/\*|\*)') { continue }   # comentarios no son lectores

            # --- lecturas ---------------------------------------------------
            # El receptor puede estar en una linea ANTERIOR: en op_11_bayes.rs la
            # lectura de `bayes_prior_alpha` (:39) es
            #     state
            #         .features
            #         .get("bayes_prior_alpha")
            #         .copied()
            #         .unwrap_or(1.0);
            # Exigir `ident.get(` en la MISMA linea perdia esa lectura (y su
            # `unwrap_or(1.0)`). Se usa una ventana hacia atras acotada.
            foreach ($m in [regex]::Matches($line, '\.\s*get\(\s*("([^"]+)"|([A-Za-z_]\w*))\s*\)')) {
                $key = if ($m.Groups[2].Success) { $m.Groups[2].Value }
                       elseif ($constMap.ContainsKey($m.Groups[3].Value)) { $constMap[$m.Groups[3].Value] }
                       else { $null }
                if (-not $key) { continue }

                $lookback = $line
                $taken = 0; $j2 = $i - 1
                while ($taken -lt 2 -and $j2 -ge 0) {
                    $prev = $lines[$j2]
                    if ($prev.Trim() -ne '' -and $prev -notmatch '^\s*(//|/\*|\*)') {
                        $lookback = $prev + ' ' + $lookback
                        $taken++
                    }
                    $j2--
                }
                if ($lookback -notmatch '\bfeatures\b') { continue }

                if (-not $readSites.ContainsKey($key)) { $readSites[$key] = @() }
                $readSites[$key] += $loc

                $expr = $line
                $j = $i
                while ($expr -notmatch ';' -and $j -lt [Math]::Min($i + 8, $lines.Count - 1)) {
                    $j++; $expr += ' ' + $lines[$j]
                }
                if ($expr -match "unwrap_or\(\s*$NumericLiteral\s*\)") {
                    $violations.Add([pscustomobject]@{ Rule='R1'; Loc=$loc
                        Message="fabrica ausencia: features.get(`"$key`") con unwrap_or literal -> $($Matches[0])" })
                }
                if ($expr -match 'unwrap_or_default\(\s*\)') {
                    $violations.Add([pscustomobject]@{ Rule='R1'; Loc=$loc
                        Message="fabrica ausencia: features.get(`"$key`") con unwrap_or_default()" })
                }
            }

            # --- inserts ----------------------------------------------------
            foreach ($m in [regex]::Matches($line,
                '([A-Za-z_]\w*)\s*\.\s*insert\(\s*("([^"]+)"|([A-Za-z_]\w*))\.to_(owned|string)\(\)\s*,\s*([^)]*)\)')) {
                if (-not (Test-IsFeaturesReceiver -Rel $rel -Name $m.Groups[1].Value)) { continue }
                $key = if ($m.Groups[3].Success) { $m.Groups[3].Value }
                       elseif ($constMap.ContainsKey($m.Groups[4].Value)) { $constMap[$m.Groups[4].Value] }
                       else { $null }
                if (-not $key) { continue }
                if ($ManifestKeys -notcontains $key) { continue }
                if (-not $prodInserts.ContainsKey($key)) { $prodInserts[$key] = @() }
                $prodInserts[$key] += $loc

                $value = $m.Groups[6].Value.Trim()
                if ($value -match "^$NumericLiteral$") {
                    $violations.Add([pscustomobject]@{ Rule='R2'; Loc=$loc
                        Message="productor fabricado: features.insert(`"$key`", $value)" })
                }
            }
        }
    }

    # R3: lectura sin contrato declarado.
    foreach ($key in $readSites.Keys) {
        if ($ManifestKeys -notcontains $key) {
            $violations.Add([pscustomobject]@{ Rule='R3'; Loc=$readSites[$key][0]
                Message="lectura sin contrato declarado: `"$key`" en $($readSites[$key] -join ', ')" })
        }
    }
    # R4: clave declarada ausente que empieza a emitirse.
    foreach ($key in $AbsentKeys) {
        if ($prodInserts.ContainsKey($key)) {
            $violations.Add([pscustomobject]@{ Rule='R4'; Loc=$prodInserts[$key][0]
                Message="`"$key`" esta declarada SIN PRODUCTOR y ahora se inserta en $($prodInserts[$key] -join ', ')" })
        }
    }
    # R5: clave sin consumidor con lector real.
    if ($readSites.ContainsKey('arbitrage_gap')) {
        $violations.Add([pscustomobject]@{ Rule='R5'; Loc=$readSites['arbitrage_gap'][0]
            Message="`"arbitrage_gap`" declarada NO_CONSUMER pero se lee en $($readSites['arbitrage_gap'] -join ', ')" })
    }

    return [pscustomobject]@{
        ReadSites   = $readSites
        ProdInserts = $prodInserts
        Violations  = $violations
        ProdFiles   = $prodFiles.Count
        ConstMap    = $constMap
    }
}

function Resolve-Verdict {
    param($Result, $LedgerEntries)
    $blocking = New-Object System.Collections.Generic.List[object]
    $debt     = New-Object System.Collections.Generic.List[object]
    $matched  = New-Object System.Collections.Generic.HashSet[string]

    foreach ($v in $Result.Violations) {
        $entry = $LedgerEntries | Where-Object { $_.loc -eq $v.Loc -and $_.rule -eq $v.Rule } | Select-Object -First 1
        if ($entry) {
            [void]$matched.Add("$($entry.rule)|$($entry.loc)")
            if ($entry.blocking) { $blocking.Add([pscustomobject]@{ V=$v; Entry=$entry }) }
            else                 { $debt.Add([pscustomobject]@{ V=$v; Entry=$entry }) }
        } else {
            $blocking.Add([pscustomobject]@{ V=$v; Entry=$null })   # NUEVA o MOVIDA
        }
    }
    $stale = @($LedgerEntries | Where-Object { -not $matched.Contains("$($_.rule)|$($_.loc)") })
    return [pscustomobject]@{ Blocking=$blocking; Debt=$debt; Stale=$stale }
}

function Show-Census {
    param($Result)
    Write-Host ''
    Write-Host 'CLAVE                     | ESTADO                | PRODUCTOR (archivo:linea)                                      | LECTORES'
    Write-Host '--------------------------+-----------------------+----------------------------------------------------------------+---------'
    foreach ($e in ($Manifest | Sort-Object key)) {
        $rd = if ($Result.ReadSites.ContainsKey($e.key)) { $Result.ReadSites[$e.key].Count } else { 0 }
        $pd = if ($e.producer) { $e.producer } else { '(ninguno)' }
        Write-Host ('{0,-25} | {1,-21} | {2,-62} | {3}' -f $e.key, $e.state, $pd, $rd)
    }
    Write-Host ''
    Write-Host 'PRODUCTORES DETECTADOS EN PRODUCCION:'
    if ($Result.ProdInserts.Count -eq 0) { Write-Host '  (ninguno)' }
    else { foreach ($k in ($Result.ProdInserts.Keys | Sort-Object)) {
        Write-Host ("  {0,-22} <- {1}" -f $k, ($Result.ProdInserts[$k] -join ', ')) } }
}

# ---------------------------------------------------------------------------
# SELFTEST: mutacion deliberada. El gate DEBE fallar.
# ---------------------------------------------------------------------------
if ($SelfTest) {
    Write-Host '=== SELFTEST (mutacion deliberada: el gate debe FALLAR) ==='
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("featgate-" + [guid]::NewGuid().ToString('N'))
    $dst = Join-Path $tmp 'backend/math-engine/src/operators'
    New-Item -ItemType Directory -Force -Path $dst | Out-Null

    $victim  = Join-Path $Root 'backend/math-engine/src/operators/op_11_bayes.rs'
    $content = Get-Content -Raw -LiteralPath $victim
    # Mutacion A (R1): una ausencia se completa con 0.25 en un sitio NUEVO.
    $mut = $content -replace 'state\.features\.get\("bayes_wins"\)\.copied\(\)\.unwrap_or\(0\.0\)',
                             'state.features.get("bayes_wins").copied().unwrap_or(0.25)'
    # Mutacion B (R2): un productor que fabrica una constante en el mapa de features.
    $mut = $mut -replace '(fn evaluate\(&self, state: &MarketState\) -> OperatorOutput \{)',
        "`$1`n        let mut features = std::collections::HashMap::new(); features.insert(`"bayes_wins`".to_string(), 0.0);"
    Set-Content -LiteralPath (Join-Path $dst 'op_11_bayes.rs') -Value $mut -NoNewline

    $res  = Invoke-Gate -RepoRoot $tmp
    $ver  = Resolve-Verdict -Result $res -LedgerEntries $Ledger
    Write-Host "VIOLACIONES DETECTADAS: $($res.Violations.Count)"
    foreach ($v in $res.Violations) { Write-Host "  - $($v.Rule) $($v.Loc) $($v.Message)" }
    Write-Host "BLOCKING: $($ver.Blocking.Count)"
    Remove-Item -Recurse -Force -LiteralPath $tmp -ErrorAction SilentlyContinue

    if ($ver.Blocking.Count -gt 0) {
        Write-Host 'SELFTEST=PASS (el gate detecta y bloquea la fabricacion)'
        exit 0
    }
    Write-Host 'SELFTEST=FAIL (el gate NO bloqueo la mutacion: no sirve como gate)'
    exit 1
}

# ---------------------------------------------------------------------------
# EJECUCION NORMAL
# ---------------------------------------------------------------------------
Write-Host 'GATE ANTI-FABRICACION MarketState.features'
Write-Host "Root: $Root"
$result = Invoke-Gate -RepoRoot $Root
$verdict = Resolve-Verdict -Result $result -LedgerEntries $Ledger
Write-Host "Archivos de produccion escaneados: $($result.ProdFiles)"

Show-Census -Result $result

Write-Host ''
Write-Host "BLOCKING (falla el gate): $($verdict.Blocking.Count)"
foreach ($b in $verdict.Blocking) {
    $tag = if ($b.Entry) { 'DECLARADA-BLOCKING' } else { 'NUEVA/MOVIDA' }
    Write-Host "  [$tag] $($b.V.Rule) $($b.V.Loc) -- $($b.V.Message)"
    if ($b.Entry) { Write-Host "      razon: $($b.Entry.reason)" }
}
Write-Host ''
Write-Host "DEUDA PREEXISTENTE CLAVADA (no falla, ratchet): $($verdict.Debt.Count)"
foreach ($d in $verdict.Debt) {
    Write-Host "  [DEBT] $($d.V.Rule) $($d.V.Loc) -- $($d.V.Message)"
    Write-Host "      $($d.Entry.reason)"
}
if ($verdict.Stale.Count -gt 0) {
    Write-Host ''
    Write-Host "ENTRADAS STALE del ledger (ya no ocurren; HAY QUE BORRARLAS): $($verdict.Stale.Count)"
    foreach ($s in $verdict.Stale) { Write-Host "  [STALE] $($s.rule) $($s.loc)" }
}

if ($Json) {
    [pscustomobject]@{
        generated_from    = 'check-feature-producers.ps1'
        manifest          = $Manifest
        mission_keys      = $MissionKeys
        producers_in_prod = $result.ProdInserts
        read_sites        = $result.ReadSites
        blocking          = @($verdict.Blocking | ForEach-Object { [pscustomobject]@{ rule=$_.V.Rule; loc=$_.V.Loc; message=$_.V.Message; declared=[bool]$_.Entry } })
        debt              = @($verdict.Debt | ForEach-Object { [pscustomobject]@{ rule=$_.V.Rule; loc=$_.V.Loc; message=$_.V.Message; reason=$_.Entry.reason } })
        stale_ledger      = @($verdict.Stale)
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Json -Encoding utf8
    Write-Host "JSON escrito en: $Json"
}

Write-Host ''
if ($verdict.Blocking.Count -gt 0) {
    Write-Host "GATE=FAIL  blocking=$($verdict.Blocking.Count)  deuda-clavada=$($verdict.Debt.Count)"
    exit 1
}
Write-Host "GATE=PASS  (0 blocking; deuda-clavada=$($verdict.Debt.Count))"
exit 0
