import os

path = r'c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\components\paper-mode-toggle.tsx'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('import { getApiBaseUrl } from "@/lib/api-client";\n"use client";', '"use client";\nimport { getApiBaseUrl } from "@/lib/api-client";')

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('Fixed use client directive')
