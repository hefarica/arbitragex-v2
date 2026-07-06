import os
import re

path = r'c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\lib\api-client.ts'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

new_logic = """const isBrowser = typeof window !== "undefined";

export function getApiBaseUrl(): string {
  const envUrl = isBrowser ? process.env.NEXT_PUBLIC_EDGE_URL : process.env.INTERNAL_EDGE_URL;
  
  const isProd = process.env.NODE_ENV === "production";

  if (isProd && envUrl && /localhost|127\\.0\\.0\\.1|0\\.0\\.0\\.0/.test(envUrl)) {
    throw new Error("Production API base URL cannot point to localhost");
  }

  if (envUrl && envUrl.trim().length > 0) {
    return envUrl.replace(/\\/$/, "");
  }

  if (isBrowser) {
    return window.location.origin;
  }

  return "";
}

export function getWsBaseUrl(): string {
  const envUrl = process.env.NEXT_PUBLIC_WS_URL;
  const isProd = process.env.NODE_ENV === "production";

  if (isProd && envUrl && /localhost|127\\.0\\.0\\.1|0\\.0\\.0\\.0/.test(envUrl)) {
    throw new Error("Production WS base URL cannot point to localhost");
  }

  if (envUrl && envUrl.trim().length > 0) {
    return envUrl.replace(/\\/$/, "");
  }

  if (isBrowser) {
    const loc = window.location;
    const protocol = loc.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${loc.host}`;
  }

  return "";
}

const DEFAULT_TIMEOUT_MS = 5000;
"""

content = re.sub(r'const EDGE_URL =.*?const DEFAULT_TIMEOUT_MS = 5000;', new_logic, content, flags=re.DOTALL)
content = content.replace('const url = `${EDGE_URL}${path}`;', 'const url = `${getApiBaseUrl()}${path}`;')
content = content.replace('const url = new URL(`${EDGE_URL}/admin/audit`);', 'const url = new URL(`${getApiBaseUrl()}/admin/audit`);')
content = content.replace('const url = `${EDGE_URL.replace(/\\/$/, "")}${path}`;', 'const url = `${getApiBaseUrl()}${path}`;')

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('api-client.ts updated')
