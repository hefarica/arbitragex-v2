#!/usr/bin/env python3
import os
import json
import argparse
from pathlib import Path
from collections import defaultdict

def analyze_frontend(project_path, frontend_dir="frontend"):
    base_path = Path(project_path)
    frontend_path = base_path / frontend_dir
    
    if not frontend_path.exists():
        print(f"Error: Directory {frontend_path} not found.")
        return False
        
    print(f"Analyzing frontend architecture in {frontend_path}...")
    
    pages = []
    hooks = []
    schemas = []
    components = defaultdict(list)
    
    # Find all pages (Next.js App Router)
    for page_file in frontend_path.glob("app/**/page.tsx"):
        rel_path = page_file.relative_to(frontend_path)
        route = "/" + str(rel_path.parent).replace("app", "").lstrip("/")
        if route == "/": route = "/"
        pages.append({"file": str(rel_path), "route": route})
        
    # Find hooks
    for hook_file in frontend_path.glob("lib/**/use*.ts"):
        hooks.append(str(hook_file.relative_to(frontend_path)))
        
    # Find schemas
    for schema_file in frontend_path.glob("lib/**/schema*.ts"):
        schemas.append(str(schema_file.relative_to(frontend_path)))
        
    # Find components
    for comp_file in frontend_path.glob("lib/**/*Client.tsx"):
        components["Client Components"].append(str(comp_file.relative_to(frontend_path)))
        
    for comp_file in frontend_path.glob("lib/**/*Card.tsx"):
        components["Card Components"].append(str(comp_file.relative_to(frontend_path)))

    # Output results
    print("\n=== FRONTEND OMNI-SSOT ANALYSIS ===")
    print(f"\n📊 TOTAL PAGES: {len(pages)}")
    for p in sorted(pages, key=lambda x: x["route"])[:15]:
        print(f"  - {p['route']} ({p['file']})")
    if len(pages) > 15:
        print(f"  ... and {len(pages)-15} more")
        
    print(f"\n🎣 CUSTOM HOOKS: {len(hooks)}")
    for h in sorted(hooks)[:10]:
        print(f"  - {h}")
    if len(hooks) > 10:
        print(f"  ... and {len(hooks)-10} more")
        
    print(f"\n🧩 SCHEMAS: {len(schemas)}")
    for s in sorted(schemas):
        print(f"  - {s}")
        
    print("\n📦 COMPONENTS BY CATEGORY:")
    for cat, comps in components.items():
        print(f"  {cat}: {len(comps)}")
        for c in comps[:5]:
            print(f"    - {c}")
        if len(comps) > 5:
            print(f"    ... and {len(comps)-5} more")
            
    print("\n✅ Analysis complete. Use these insights to build FRONTEND_OMNI_SSOT_MAP.md")
    return True

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Analyze frontend architecture for SSOT mapping")
    parser.add_argument("project_path", help="Path to the project root")
    parser.add_argument("--dir", default="frontend", help="Frontend directory name (default: frontend)")
    
    args = parser.parse_args()
    analyze_frontend(args.project_path, args.dir)
