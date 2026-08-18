import json

strategies = json.load(open("docs/atlas_strategies.json", encoding="utf8"))
fam = {
    1: ("Spot DEX intra-chain", "route_graph_engine", 36),
    2: ("Curva del AMM", "amm_curve_engine", 17),
    3: ("Disparadas por evento", "state_event_engine", 31),
    4: ("Paridad/Redención", "parity_redemption_engine", 31),
    5: ("CEX-DEX y externos", "cex_external_engine", 14),
    6: ("Cross-chain/domain", "cross_domain_engine", 30),
    7: ("Derivados y volatilidad", "derivatives_engine", 30),
    8: ("Lending/Liquidación", "credit_liquidation_engine", 25),
    9: ("Intents/Solvers/Order flow", "intents_solver_engine", 20),
    10: ("NFT/Juegos", "nft_engine", 18),
    11: ("Mercados de predicción", "prediction_engine", 12),
}
data_js = json.dumps(strategies, ensure_ascii=False)
fam_js = json.dumps({str(k): v for k, v in fam.items()}, ensure_ascii=False)

html = """<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Atlas 264 Estrategias</title>
<style>
:root{
  --ground:#f4f6fa; --card:#ffffff; --card2:#eef1f7; --border:#d4dae6; --text:#141a26;
  --muted:#5f6b80; --accent:#3b6cf6; --success:#0e9f6e; --warning:#b57d0a; --destructive:#dc2626;
  --chipbg:#e7ebf3; --mono:ui-monospace,'Cascadia Code',Consolas,Menlo,monospace; --sans:system-ui,'Segoe UI',Roboto,sans-serif;
}
@media (prefers-color-scheme: dark){ :root:not([data-theme="light"]){
  --ground:#0a0d13; --card:#11151f; --card2:#0d1119; --border:#242c3d; --text:#e2e8f0;
  --muted:#8b95a7; --accent:#6f9bff; --success:#34d399; --warning:#f5b544; --destructive:#f87171; --chipbg:#1a2130;
}}
:root[data-theme="dark"]{
  --ground:#0a0d13; --card:#11151f; --card2:#0d1119; --border:#242c3d; --text:#e2e8f0;
  --muted:#8b95a7; --accent:#6f9bff; --success:#34d399; --warning:#f5b544; --destructive:#f87171; --chipbg:#1a2130;
}
*{box-sizing:border-box} body{margin:0;background:var(--ground);color:var(--text);font-family:var(--sans);line-height:1.45}
main{max-width:1180px;margin:0 auto;padding:28px 20px 80px}
header h1{font-size:26px;margin:0 0 4px;letter-spacing:-.02em}
header p.sub{color:var(--muted);margin:0 0 14px;font-size:14px;max-width:70ch}
.chips{display:flex;gap:8px;flex-wrap:wrap;margin-bottom:18px}
.chip{font-family:var(--mono);font-size:11.5px;padding:4px 10px;border:1px solid var(--border);border-radius:999px;background:var(--chipbg)}
.chip b{font-weight:700}
.controls{display:flex;gap:8px;flex-wrap:wrap;margin-bottom:22px;align-items:center}
.controls input[type=search]{flex:1;min-width:200px;padding:8px 12px;border:1px solid var(--border);border-radius:8px;background:var(--card);color:var(--text);font-family:var(--mono);font-size:12.5px}
select{padding:8px 10px;border:1px solid var(--border);border-radius:8px;background:var(--card);color:var(--text);font-size:12.5px}
h2.fam{font-size:15px;margin:34px 0 4px;display:flex;gap:10px;align-items:baseline;flex-wrap:wrap}
h2.fam .cnt{font-family:var(--mono);font-size:11.5px;color:var(--muted)}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:10px;margin-top:12px}
.sc{border:1px solid var(--border);border-radius:10px;background:var(--card);padding:11px 12px;display:flex;flex-direction:column;gap:7px}
.sc.off{opacity:.45}
.sc .row1{display:flex;justify-content:space-between;align-items:flex-start;gap:8px}
.badge{font-size:11.5px;font-weight:600;padding:3px 8px;border-radius:6px;border:1px solid transparent;line-height:1.2;white-space:nowrap}
.badge.shadow{color:var(--accent);border-color:var(--accent)}
.badge.paper{color:var(--warning);border-color:var(--warning)}
.sc .name{font-size:13.5px;font-weight:650;letter-spacing:-.01em}
.meta{font-family:var(--mono);font-size:10.5px;color:var(--muted);display:flex;gap:8px;flex-wrap:wrap}
.meta .det{color:var(--text)}
.ops{display:flex;gap:4px;flex-wrap:wrap}
.op{font-family:var(--mono);font-size:9.5px;padding:2px 6px;border-radius:4px;background:var(--chipbg);border:1px solid var(--border);color:var(--muted)}
.togrow{display:flex;align-items:center;gap:8px;margin-top:auto;padding-top:6px;border-top:1px dashed var(--border)}
.togrow code{font-family:var(--mono);font-size:10px;color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1}
.sw{position:relative;width:34px;height:19px;flex-shrink:0}
.sw input{opacity:0;position:absolute;inset:0;margin:0;cursor:pointer;z-index:1}
.sw span{position:absolute;inset:0;background:var(--success);border-radius:999px;transition:background .15s}
.sw span::after{content:'';position:absolute;top:2.5px;left:18px;width:14px;height:14px;border-radius:50%;background:#fff;transition:left .15s}
.sw input:not(:checked)+span{background:var(--border)}
.sw input:not(:checked)+span::after{left:2.5px}
.sw input:focus-visible+span{outline:2px solid var(--accent);outline-offset:2px}
section.states{margin:26px 0 6px}
.cards2{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:12px;margin-top:12px}
.demo{border-radius:12px;padding:14px;font-size:12.5px}
.demo.eval{background:var(--card);border:1px solid var(--border)}
.demo.diag{background:var(--card2);border:1.5px dashed var(--border)}
.demo h4{margin:0 0 8px;font-size:12px;text-transform:uppercase;letter-spacing:.08em;color:var(--muted)}
.kv{display:flex;justify-content:space-between;gap:10px;font-family:var(--mono);font-size:12px;padding:2.5px 0}
.kv .v{color:var(--muted);text-align:right} .kv .num{color:var(--success);font-weight:700}
.kv .neg{color:var(--destructive)}
.btn{display:block;text-align:center;margin-top:10px;padding:8px;border-radius:8px;background:var(--accent);color:#fff;font-weight:700;font-size:11px;letter-spacing:.06em}
figure{margin:26px 0 0}
figcaption{font-size:12.5px;color:var(--muted);margin-top:8px;max-width:80ch}
svg{max-width:100%;height:auto;display:block}
footer{margin-top:40px;color:var(--muted);font-size:11.5px;font-family:var(--mono)}
</style>
</head>
<body>
<main>
<header>
<h1>Atlas 264 Estrategias</h1>
<p class="sub">Cómo se ve cada estrategia del catálogo maestro ArbitrageX en su tarjeta: identidad, familia, detector, operadores matemáticos y su toggle de activación. Hoy el pipeline habilita ~5 estrategias; el objetivo es el máximo disponible — 264 — controlado SOLO por estos toggles.</p>
<div class="chips">
<span class="chip">TOTAL <b>264</b></span>
<span class="chip">SHADOW <b>160</b></span>
<span class="chip">PAPER <b>104</b></span>
<span class="chip">habilitadas <b id="cnt">264</b>/264</span>
</div>
</header>
<div class="controls">
<input id="q" type="search" placeholder="buscar estrategia, detector, MEV-ID...">
<select id="fmode"><option value="">todos los modos</option><option>SHADOW</option><option>PAPER</option></select>
<select id="ffam"><option value="">todas las familias</option></select>
</div>
<section class="states">
<h2 class="fam">Los dos estados de tarjeta (SSOT)</h2>
<div class="cards2">
<div class="demo eval"><h4>Evaluada — con economía real + Execute</h4>
<div class="kv"><span>Net yield</span><span class="num">$18.20</span></div>
<div class="kv"><span>Gross · AMM spread</span><span>$42.50</span></div>
<div class="kv"><span>Gas + LP + slippage</span><span class="neg">-24.30</span></div>
<div class="kv"><span>ROI</span><span class="num">1.70%</span></div>
<div class="kv"><span>Ruta</span><span class="v">WETH-USDC-DAI-WETH · 3 legs</span></div>
<div class="kv"><span>Fuente net</span><span class="v">canonical spine</span></div>
<div class="btn">EXECUTE (PAPER SHADOW)</div></div>
<div class="demo diag"><h4>Detección — sin evaluar (diagnóstico)</h4>
<div class="kv"><span>Estado</span><span class="v">DETECCION — sin evaluar</span></div>
<div class="kv"><span>Por qué no pasó</span><span class="v">categoría sin motor de evaluación</span></div>
<div class="kv"><span>Razón máquina</span><span class="v">cartridge_unmapped_...</span></div>
<div class="kv"><span>Economía</span><span class="v">no computada (honesto)</span></div>
<div class="kv"><span>Execute</span><span class="v">no disponible sin evaluar</span></div></div>
</div>
</section>
<figure>
<svg viewBox="0 0 900 190" role="img" aria-label="Cableado del toggle: del Excel maestro a la tarjeta; el toggle trading_config enciende o silencia cada cartucho.">
<defs><marker id="a" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="currentColor"/></marker></defs>
<g fill="currentColor" font-family="ui-monospace,Consolas,monospace" font-size="12">
<rect x="10" y="60" width="130" height="52" rx="8" fill="none" stroke="currentColor"/><text x="75" y="82" text-anchor="middle">Excel maestro</text><text x="75" y="98" text-anchor="middle" opacity=".65">264 x 28</text>
<rect x="180" y="60" width="130" height="52" rx="8" fill="none" stroke="currentColor"/><text x="245" y="82" text-anchor="middle">math_map.json</text><text x="245" y="98" text-anchor="middle" opacity=".65">detector + ops</text>
<rect x="350" y="60" width="130" height="52" rx="8" fill="none" stroke="currentColor"/><text x="415" y="82" text-anchor="middle">cartucho .rhai</text><text x="415" y="98" text-anchor="middle" opacity=".65">evaluate_opportunity</text>
<rect x="520" y="60" width="150" height="52" rx="8" fill="none" stroke="currentColor"/><text x="595" y="82" text-anchor="middle">mapeo cartridge_boot</text><text x="595" y="98" text-anchor="middle" opacity=".65">11 familias a engine</text>
<rect x="710" y="14" width="170" height="52" rx="8" fill="none" stroke="#6f9bff"/><text x="795" y="36" text-anchor="middle">Tarjeta (SSOT)</text><text x="795" y="52" text-anchor="middle" opacity=".65">evaluada | deteccion</text>
<rect x="710" y="120" width="170" height="52" rx="8" fill="none" stroke="#34d399"/><text x="795" y="142" text-anchor="middle">Toggle</text><text x="795" y="158" text-anchor="middle" opacity=".65">strategy.XX.YYY.enabled</text>
<line x1="140" y1="86" x2="178" y2="86" stroke="currentColor" marker-end="url(#a)"/>
<line x1="310" y1="86" x2="348" y2="86" stroke="currentColor" marker-end="url(#a)"/>
<line x1="480" y1="86" x2="518" y2="86" stroke="currentColor" marker-end="url(#a)"/>
<line x1="670" y1="86" x2="708" y2="50" stroke="currentColor" marker-end="url(#a)"/>
<line x1="795" y1="120" x2="795" y2="68" stroke="#34d399" marker-end="url(#a)"/><text x="803" y="100" font-size="10.5" opacity=".8">habilita / silencia</text>
<text x="245" y="160" font-size="10.5" opacity=".6">el toggle vive en trading_config.strategy_configs y enciende el cartucho</text>
</g></svg>
<figcaption>Camino completo: el catálogo define identidad y matemática por estrategia; el toggle (verde) es el ÚNICO interruptor — con todos activos el grid se puebla con el máximo disponible y cada desactivación es decisión del operador.</figcaption>
</figure>
<section id="atlas"></section>
<footer id="foot"></footer>
</main>
<script>
const S=__DATA__; const FAM=__FAM__;
const atlas=document.getElementById('atlas'); const q=document.getElementById('q'), fm=document.getElementById('fmode'), ff=document.getElementById('ffam'), cnt=document.getElementById('cnt');
const famNames={1:'G01 · Spot DEX intra-chain',2:'G02 · Curva del AMM',3:'G03 · Disparadas por evento',4:'G04 · Paridad / Redención',5:'G05 · CEX–DEX y externos',6:'G06 · Cross-chain / domain',7:'G07 · Derivados y volatilidad',8:'G08 · Lending / Liquidación',9:'G09 · Intents / Solvers',10:'G10 · NFT / Juegos',11:'G11 · Predicción'};
Object.keys(FAM).sort((a,b)=>a-b).forEach(g=>{const o=document.createElement('option');o.value=g;o.textContent=famNames[g];ff.appendChild(o)});
const state={}; S.forEach(s=>state[s.id]=true);
function render(){
  const t=q.value.trim().toLowerCase(), m=fm.value, g=ff.value;
  let enabled=0; S.forEach(s=>{if(state[s.id])enabled++}); cnt.textContent=enabled;
  const groups={};
  S.forEach(s=>{
    if(m&&s.mode!==m)return; if(g&&String(s.g)!==g)return;
    if(t&&!(s.name.toLowerCase().includes(t)||s.id.toLowerCase().includes(t)||s.det.toLowerCase().includes(t)))return;
    (groups[s.g]=groups[s.g]||[]).push(s);
  });
  atlas.innerHTML='';
  const keys=Object.keys(groups).sort((a,b)=>a-b);
  if(!keys.length){atlas.innerHTML='<p style="color:var(--muted)">sin resultados</p>';return}
  keys.forEach(gk=>{
    const h=document.createElement('h2');h.className='fam';
    h.innerHTML=famNames[gk]+' <span class="cnt">'+groups[gk].length+' · motor '+FAM[gk][1]+'</span>';atlas.appendChild(h);
    const grid=document.createElement('div');grid.className='grid';
    groups[gk].forEach(s=>{
      const d=document.createElement('div');d.className='sc'+(state[s.id]?'':' off');
      d.innerHTML='<div class="row1"><span class="name">'+s.name+'</span><span class="badge '+s.mode.toLowerCase()+'">'+s.mode+'</span></div>'
        +'<div class="meta"><span>'+s.id+'</span><span class="det">'+s.det+'</span><span>legs '+s.legs+'</span>'+(s.atomic?'<span style="color:var(--success)">atómica</span>':'')+'</div>'
        +'<div class="ops">'+s.ops_p.map(o=>'<span class="op">'+o+'</span>').join('')+'</div>'
        +'<div class="togrow"><label class="sw"><input type="checkbox" '+(state[s.id]?'checked':'')+' aria-label="toggle '+s.tog+'"><span></span></label><code>'+s.tog+'</code></div>';
      grid.appendChild(d);
    });
    atlas.appendChild(grid);
  });
}
atlas.addEventListener('change',e=>{const c=e.target.closest('.sw input');if(!c)return;const card=c.closest('.sc');const code=card.querySelector('code').textContent;const id=S.find(s=>s.tog===code)?.id;if(id){state[id]=c.checked;card.classList.toggle('off',!c.checked);let n=0;S.forEach(s=>{if(state[s.id])n++});cnt.textContent=n}});
q.addEventListener('input',render);fm.addEventListener('change',render);ff.addEventListener('change',render);
render();
document.getElementById('foot').textContent='Fuente: ArbitrageX_264_Cartridge_Math_Architecture · 02_CARTRIDGE_MATH_MAP (READY_FOR_CARTRIDGE_MIGRATION) · modos del Excel = ley';
</script>
</body>
</html>"""
html = html.replace("__DATA__", data_js).replace("__FAM__", fam_js)
open("docs/atlas_264.html", "w", encoding="utf8").write(html)
print("atlas listo:", len(html) // 1024, "KB")
