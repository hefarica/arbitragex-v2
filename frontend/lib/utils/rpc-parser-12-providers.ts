export interface ParsedRpcUrls {
  httpUrls: string;
  wssUrls: string;
  originalCount: number;
  parsedCount: number;
}

export function parseRpcUrls12Providers(rawInput: string): ParsedRpcUrls {
  if (!rawInput || rawInput.trim() === '') {
    return { httpUrls: '', wssUrls: '', originalCount: 0, parsedCount: 0 };
  }

  // Separar por comas, espacios o saltos de línea
  const rawUrls = rawInput.split(/[\s,]+/).filter(url => url.trim() !== '');
  
  const httpMap = new Map<string, string>();
  const wssMap = new Map<string, string>();
  
  let parsedCount = 0;

  for (const raw of rawUrls) {
    try {
      let urlString = raw;
      let explicitName = '';

      // Si tiene formato "nombre=url", extraer ambos
      if (raw.includes('=')) {
        const parts = raw.split('=');
        explicitName = (parts[0] || '').trim().toLowerCase();
        urlString = (parts[1] || '').trim();
      }

      // Validar que sea URL válida
      const url = new URL(urlString);
      
      // Detectar protocolo
      const isWss = url.protocol === 'wss:' || url.protocol === 'ws:';
      const isHttp = url.protocol === 'https:' || url.protocol === 'http:';
      
      if (!isWss && !isHttp) continue; // Ignorar protocolos no soportados

      // Normalizar ws -> wss y http -> https (recomendado para RPCs)
      if (url.protocol === 'ws:') url.protocol = 'wss:';
      
      const host = url.hostname.toLowerCase();
      let providerName = explicitName;
      let chainSuffix = '';

      // Detectar chain desde el host o path
      const fullPath = (host + url.pathname).toLowerCase();
      if (fullPath.includes('mainnet') && !fullPath.includes('bsc') && !fullPath.includes('polygon') && !fullPath.includes('arb') && !fullPath.includes('opt') && !fullPath.includes('base')) chainSuffix = '-mainnet';
      else if (fullPath.includes('sepolia')) chainSuffix = '-sepolia';
      else if (fullPath.includes('goerli')) chainSuffix = '-goerli';
      else if (fullPath.includes('holesky')) chainSuffix = '-holesky';
      else if (fullPath.includes('hoodi')) chainSuffix = '-hoodi';
      else if (fullPath.includes('bsc-mainnet') || fullPath.includes('bsc-dataseed')) chainSuffix = '-bsc';
      else if (fullPath.includes('bsc-testnet')) chainSuffix = '-bsc-testnet';
      else if (fullPath.includes('polygon-mainnet') || fullPath.includes('polygon-rpc')) chainSuffix = '-polygon';
      else if (fullPath.includes('polygon-mumbai')) chainSuffix = '-mumbai';
      else if (fullPath.includes('arb-mainnet') || fullPath.includes('arbitrum-mainnet') || fullPath.includes('arb1')) chainSuffix = '-arbitrum';
      else if (fullPath.includes('arb-sepolia')) chainSuffix = '-arb-sepolia';
      else if (fullPath.includes('opt-mainnet') || fullPath.includes('optimism-mainnet')) chainSuffix = '-optimism';
      else if (fullPath.includes('base-mainnet')) chainSuffix = '-base';

      // Detectar proveedor si no se especificó explícitamente
      if (!providerName) {
        if (host.includes('alchemy')) providerName = 'alchemy';
        else if (host.includes('infura')) providerName = 'infura';
        else if (host.includes('drpc')) providerName = 'drpc';
        else if (host.includes('publicnode')) providerName = 'publicnode';
        else if (host.includes('quicknode') || host.includes('quiknode')) providerName = 'quicknode';
        else if (host.includes('lava')) providerName = 'lava';
        else if (host.includes('llama')) providerName = 'llamarpc';
        else if (host.includes('cloudflare')) providerName = 'cloudflare';
        else if (host.includes('flashbots')) providerName = 'flashbots';
        else if (host.includes('mevblocker') || host.includes('mev-blocker')) providerName = 'mevblocker';
        else if (host.includes('0xrpc')) providerName = '0xrpc';
        else if (host.includes('1rpc')) providerName = '1rpc';
        else if (host.includes('blockpi')) providerName = 'blockpi';
        else if (host.includes('tenderly')) providerName = 'tenderly';
        else if (host.includes('blockreq')) providerName = 'blockreq';
        else if (host.includes('chainstack')) providerName = 'chainstack';
        else if (host.includes('ankr')) providerName = 'ankr';
        else if (host.includes('blast')) providerName = 'blast';
        else {
          const parts = host.split('.');
          providerName = parts.length >= 2 ? (parts[parts.length - 2] || host) : (parts[0] || host);
        }
      }

      providerName = providerName.toLowerCase();
      
      // Combinar proveedor y chain para clave única (ej: alchemy-mainnet)
      // Si el proveedor ya incluye la chain, no la duplicamos
      const uniqueKey = providerName.includes(chainSuffix.replace('-', '')) ? providerName : `${providerName}${chainSuffix}`;

      // Añadir al mapa correspondiente (deduplicación automática por clave única)
      if (isWss) {
        if (!wssMap.has(uniqueKey)) {
          wssMap.set(uniqueKey, url.toString());
          parsedCount++;
        }
      } else if (isHttp) {
        if (!httpMap.has(uniqueKey)) {
          httpMap.set(uniqueKey, url.toString());
          parsedCount++;
        }
      }
    } catch (e) {
      // Ignorar URLs inválidas
      console.warn(`Invalid URL: ${raw}`, e);
    }
  }

  // Ordenamiento inteligente (prioridad)
  const priorityOrder = [
    'alchemy', 'drpc', 'publicnode', 'infura', 'quicknode', 'lava', 
    'llamarpc', 'cloudflare', 'flashbots', 'mevblocker', '0xrpc', 
    '1rpc', 'blockpi', 'tenderly', 'blockreq'
  ];

  const sortByKey = (a: [string, string], b: [string, string]) => {
    const getPriority = (key: string) => {
      const baseProvider = key.split('-')[0];
      const index = priorityOrder.indexOf(baseProvider || '');
      return index === -1 ? 999 : index;
    };
    return getPriority(a[0]) - getPriority(b[0]);
  };

  // Convertir mapas a strings formateados (nombre=url,nombre2=url2)
  const httpUrlsStr = Array.from(httpMap.entries())
    .sort(sortByKey)
    .map(([key, url]) => `${key}=${url}`)
    .join(',');

  const wssUrlsStr = Array.from(wssMap.entries())
    .sort(sortByKey)
    .map(([key, url]) => `${key}=${url}`)
    .join(',');

  return {
    httpUrls: httpUrlsStr,
    wssUrls: wssUrlsStr,
    originalCount: rawUrls.length,
    parsedCount
  };
}
