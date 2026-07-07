import os

path = r'c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\components\paper-mode-toggle.tsx'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('EDGE_URL.replace', 'getApiBaseUrl().replace')
if 'getApiBaseUrl' in content and 'import { getApiBaseUrl }' not in content:
    content = 'import { getApiBaseUrl } from "@/lib/api-client";\n' + content

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('Fixed paper-mode-toggle.tsx')
