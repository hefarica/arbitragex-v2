import os
import glob

files = glob.glob(r'c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\onboarding\*\page.tsx')
for path in files:
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    content = content.replace('edgeUrl={edgeUrl}', 'edgeUrl={getApiBaseUrl()}')
    if 'getApiBaseUrl' not in content:
        content = 'import { getApiBaseUrl } from "@/lib/api-client";\n' + content
    
    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)
    print(f'Fixed {path}')
