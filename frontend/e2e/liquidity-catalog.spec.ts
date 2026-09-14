import { test, expect, type Page } from '@playwright/test';

test.beforeEach(() => {
  const target = new URL(process.env.E2E_BASE_URL ?? 'http://localhost:3000');
  expect(['localhost', '127.0.0.1', '[::1]']).toContain(target.hostname);
});

// Explicit UI contract fixtures. These tests neither contact an RPC nor broadcast.
const dexId = '00000000-0000-4000-8000-000000000001';
const poolId = '00000000-0000-4000-8000-000000000002';
const poolAddress = '0x0000000000000000000000000000000000000002';
const rootRow = { id:'1', chain_id:'1', label:'Catalog fixture chain', active:true,
  registered:true, dex_count:'1', factory_count:'1', pool_count:'1' };
function envelope(url: URL, items: object[], next: string | null = null) {
  return { schema_version:1, source:'postgresql-registry', level:url.searchParams.get('level'),
    observed_at:'2026-09-12T00:00:00.000Z', execution_verified:false, counts_include_inactive:true,
    scope:{chain_id:url.searchParams.get('chain_id'),dex_id:url.searchParams.get('dex_id'),q:url.searchParams.get('q') ?? ''},
    count:items.length,limit:25,items,next_after:next };
}
async function route(page: Page, reply: (url: URL) => object | {status:number}) {
  await page.route('**/api/v1/pools?*', async request => {
    const url = new URL(request.request().url());
    if(url.searchParams.get('view') !== 'liquidity_catalog') { await request.continue(); return; }
    expect(request.request().method()).toBe('GET');
    const body = reply(url);
    if('status' in body && typeof body.status === 'number') await request.fulfill({status:body.status,json:{error:'catalog_unavailable'}});
    else await request.fulfill({json:body});
  });
}
test('drill down chain to DEX to pool, preserve administration links, return to root', async ({page}, info) => {
  await route(page, url => {
    const level=url.searchParams.get('level');
    if(level==='chains')return envelope(url,[rootRow]);
    if(level==='dexes')return envelope(url,[{id:dexId,chain_id:'1',label:'Catalog fixture DEX',active:true,protocol_type:'UNISWAP_V2',factory_count:'1',pool_count:'1'}]);
    return envelope(url,[{id:poolId,chain_id:'1',label:poolAddress,active:true,dex_id:dexId,dex_name:'Catalog fixture DEX',dex_active:true,
      protocol_type:'UNISWAP_V2',factory_address:'0x0000000000000000000000000000000000000001',pool_address:poolAddress,
      fee_tier:'3000',token0_address:null,token0_symbol:null,token1_address:null,token1_symbol:null}]);
  });
  await page.goto('/dex-registry');
  const catalog=page.getByTestId('liquidity-catalog');
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
  await expect(catalog.getByRole('link',{name:'Agregar o configurar blockchain'})).toHaveAttribute('href','/admin/chains');
  await expect(catalog.getByRole('link',{name:'Verificación de ejecución LIVE'})).toHaveAttribute('href','/live-readiness');
  await catalog.getByRole('button',{name:'Ver DEX',exact:true}).click();
  await expect(catalog.getByText('Catalog fixture DEX',{exact:true})).toBeVisible();
  await catalog.getByRole('button',{name:'Ver pools',exact:true}).click();
  await expect(catalog.getByText(poolAddress,{exact:true})).toBeVisible();
  await catalog.getByText('Direcciones completas',{exact:true}).click();
  await expect(catalog.getByText(`Pool ID: ${poolId}`)).toBeVisible();
  await info.attach('liquidity-catalog-pool',{body:await catalog.screenshot(),contentType:'image/png'});
  await catalog.getByRole('button',{name:'Blockchains',exact:true}).click();
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
  // Navigating to the already selected root must not stick in a loading state.
  await catalog.getByRole('button',{name:'Blockchains',exact:true}).click();
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
});
test('keyset navigation and search reset the cursor',async({page})=>{
  await route(page,url=>{
    if(url.searchParams.get('q'))return envelope(url,[]);
    return url.searchParams.has('after')
      ? envelope(url,[{...rootRow,id:'11155111',chain_id:'11155111',label:'Catalog testnet fixture'}])
      : envelope(url,[rootRow],'1');
  });
  await page.goto('/dex-registry');const catalog=page.getByTestId('liquidity-catalog');
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
  await catalog.getByRole('button',{name:'Siguiente',exact:true}).click();
  await expect(catalog.getByText('Catalog testnet fixture',{exact:true})).toBeVisible();
  await expect(catalog.getByRole('button',{name:'Siguiente',exact:true})).toBeDisabled();
  await catalog.getByRole('button',{name:'Anterior',exact:true}).click();
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
  await catalog.getByLabel('Buscar en el nivel actual').fill('not found');
  await catalog.getByRole('button',{name:'Buscar',exact:true}).click();
  await expect(catalog.getByText('No hay registros que coincidan con esta consulta.')).toBeVisible();
  await expect(catalog.getByRole('button',{name:'Anterior',exact:true})).toBeDisabled();
});
test('503 clears old inventory and retry restores it',async({page})=>{
  let failing=false;await route(page,url=>failing?{status:503}:envelope(url,[rootRow]));
  await page.goto('/dex-registry');const catalog=page.getByTestId('liquidity-catalog');
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
  failing=true;await catalog.getByRole('button',{name:'Actualizar catálogo'}).click();
  await expect(catalog.getByRole('alert')).toContainText('HTTP 503');
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toHaveCount(0);
  failing=false;await catalog.getByRole('button',{name:'Actualizar catálogo'}).click();
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
});
test('legacy pool responses are incompatible, not an empty successful catalog',async({page})=>{
  await route(page,()=>({chain_id:1,count:0,items:[]}));
  await page.goto('/dex-registry');const catalog=page.getByTestId('liquidity-catalog');
  await expect(catalog.getByRole('alert')).toBeVisible();
  await expect(catalog.getByText('No hay registros que coincidan con esta consulta.')).toHaveCount(0);
});

// No malformed or stale wire response may become a successful empty inventory.
test('wrong-chain response is quarantined, never displayed',async({page})=>{
  await route(page,url=>url.searchParams.get('level')==='chains'?envelope(url,[rootRow]):envelope(url,[
    {id:dexId,chain_id:'11155111',label:'Wrong chain fixture',active:true,protocol_type:'UNISWAP_V2',factory_count:'1',pool_count:'1'}
  ]));
  await page.goto('/dex-registry');const catalog=page.getByTestId('liquidity-catalog');
  await catalog.getByRole('button',{name:'Ver DEX',exact:true}).click();
  await expect(catalog.getByRole('alert')).toBeVisible();
  await expect(catalog.getByText('Wrong chain fixture',{exact:true})).toHaveCount(0);
});
test('a repeated cursor fails rather than showing the same page as next',async({page})=>{
  await route(page,url=>envelope(url,[rootRow],'1'));
  await page.goto('/dex-registry');const catalog=page.getByTestId('liquidity-catalog');
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toBeVisible();
  await catalog.getByRole('button',{name:'Siguiente',exact:true}).click();
  await expect(catalog.getByRole('alert')).toBeVisible();
  await expect(catalog.getByText('Catalog fixture chain',{exact:true})).toHaveCount(0);
});
