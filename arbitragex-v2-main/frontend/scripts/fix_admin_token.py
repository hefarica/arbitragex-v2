import os
import re

path = r'c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\lib\admin-token.ts'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('`${EDGE_URL}/admin/session`', '`${getApiBaseUrl()}/admin/session`')
content = content.replace('`${EDGE_URL}/admin/session/logout`', '`${getApiBaseUrl()}/admin/session/logout`')

if 'getApiBaseUrl' in content and 'import { getApiBaseUrl }' not in content:
    content = 'import { getApiBaseUrl } from "@/lib/api-client";\n' + content

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('Fixed admin-token.ts')
