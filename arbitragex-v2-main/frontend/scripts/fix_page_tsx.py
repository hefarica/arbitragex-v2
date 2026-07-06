import os

path = r'c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\page.tsx'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('edgeUrl={edgeUrl}', 'edgeUrl={getApiBaseUrl()}')
content = content.replace('{edgeUrl}', '{getApiBaseUrl()}')

if 'getApiBaseUrl' in content and 'import { getApiBaseUrl }' not in content:
    content = 'import { getApiBaseUrl } from "@/lib/api-client";\n' + content

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('Fixed page.tsx')
