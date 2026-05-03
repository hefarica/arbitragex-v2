import os
import re

files_to_update = [
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\page.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\opportunities\page.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\onboarding\2-connect\page.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\onboarding\3-advanced\page.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\onboarding\4-testing\page.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\app\onboarding\5-production\page.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\components\site-header.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\components\paper-mode-toggle.tsx",
    r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\lib\admin-token.ts"
]

for filepath in files_to_update:
    if os.path.exists(filepath):
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()

        # Reemplazar declaracion de edgeUrl estatico
        content = re.sub(r'const edgeUrl = process\.env\.NEXT_PUBLIC_EDGE_URL \?\? "http://localhost:8787";\s*', '', content)
        content = re.sub(r'const EDGE_URL = process\.env\.NEXT_PUBLIC_EDGE_URL \|\| "http://localhost:8787";\s*', '', content)
        
        # En opportunities page
        content = re.sub(r'const wsUrl = process\.env\.NEXT_PUBLIC_WS_URL \?\? "http://localhost:3000";\s*', '', content)
        content = content.replace('const socket = io(wsUrl, {', 'import { getWsBaseUrl } from "@/lib/api-client";\nconst socket = io(getWsBaseUrl(), {')

        # Reemplazos directos
        content = content.replace('{process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787"}', '{getApiBaseUrl()}')
        content = content.replace('`${edgeUrl}/admin/token/validate`', '`${getApiBaseUrl()}/admin/token/validate`')
        content = content.replace('`${EDGE_URL}/admin/mode`', '`${getApiBaseUrl()}/admin/mode`')
        content = content.replace('`${EDGE_URL}/admin/onboarding/1/complete`', '`${getApiBaseUrl()}/admin/onboarding/1/complete`')

        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"Updated {os.path.basename(filepath)}")
