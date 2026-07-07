import os
import re

path = r'c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\frontend\next.config.js'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Delete fallbacks
content = re.sub(r'NEXT_PUBLIC_EDGE_URL:\s*EDGE_URL\s*\|\|\s*\"http://localhost:8787\",', 'NEXT_PUBLIC_EDGE_URL: EDGE_URL,', content)
content = re.sub(r'NEXT_PUBLIC_WS_URL:\s*WS_URL\s*\|\|\s*\"http://localhost:8080\",', 'NEXT_PUBLIC_WS_URL: WS_URL,', content)

# Remove console.warn for missing URLs (it should be an error in prod, but next.config.js runs at build time)
content = re.sub(r'if \(!EDGE_URL\) \{[\s\S]*?\}', '', content)
content = re.sub(r'if \(!WS_URL\) \{[\s\S]*?\}', '', content)

# Fix csp localhost reference
content = re.sub(r'const resolvedEdge = EDGE_URL \|\| \"http://localhost:8787\";', 'const resolvedEdge = EDGE_URL || \"\";', content)
content = re.sub(r'const resolvedWs = WS_URL \|\| \"http://localhost:8080\";', 'const resolvedWs = WS_URL || \"\";', content)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('next.config.js updated')
