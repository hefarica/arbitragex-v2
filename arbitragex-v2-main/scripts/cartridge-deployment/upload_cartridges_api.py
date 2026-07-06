#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
ArbitrageX-v2: Script de Ensamblaje de Cartuchos via API
═══════════════════════════════════════════════════════════════════════════════

Script para subir cartuchos Rhai a ArbitrageX-v2 usando la API REST.

MODO DE USO:
1. Asegurarse de que ArbitrageX-v2 está corriendo (docker-compose up)
2. Configurar variables de entorno:
   - ARBITRAGEX_API_URL: URL base de la API (default: http://localhost:8080)
   - ARBITRAGEX_ADMIN_TOKEN: Token de administrador
3. Ejecutar: python3 upload_cartridges_api.py

RESULTADO:
- Cartuchos subidos a la base de datos
- Registrados en cartridge_registry
- Listos para ser ejecutados por el Searcher Engine
"""

import os
import sys
import json
import requests
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional

# ═══════════════════════════════════════════════════════════════════════════════
# CONFIGURACIÓN
# ═══════════════════════════════════════════════════════════════════════════════

ARBITRAGEX_API_URL = os.environ.get("ARBITRAGEX_API_URL", "http://localhost:8080")
ARBITRAGEX_ADMIN_TOKEN = os.environ.get("ARBITRAGEX_ADMIN_TOKEN", "")

if not ARBITRAGEX_ADMIN_TOKEN:
    print("❌ Error: ARBITRAGEX_ADMIN_TOKEN no configurada")
    print("   Establece: export ARBITRAGEX_ADMIN_TOKEN=tu_token_admin")
    sys.exit(1)

CARTRIDGES_DIR = Path("/home/ubuntu/cartridges")
CARTRIDGES_DIR.mkdir(parents=True, exist_ok=True)

# ═══════════════════════════════════════════════════════════════════════════════
# FUNCIONES
# ═══════════════════════════════════════════════════════════════════════════════

def read_cartridge_file(filepath: Path) -> str:
    """Lee contenido de archivo Rhai"""
    try:
        with open(filepath, 'r') as f:
            return f.read()
    except Exception as e:
        print(f"❌ Error leyendo {filepath}: {e}")
        return ""


def extract_cartridge_metadata(content: str) -> Dict:
    """Extrae metadata del cartucho del código Rhai"""
    # Buscar la función init_strategy() para extraer metadata
    try:
        # Buscar línea con "name:"
        for line in content.split('\n'):
            if 'name:' in line and '"' in line:
                name = line.split('"')[1]
                break
        else:
            name = "Unknown"
        
        # Buscar versión
        for line in content.split('\n'):
            if 'version:' in line and '"' in line:
                version = line.split('"')[1]
                break
        else:
            version = "1.0.0"
        
        # Buscar descripción
        for line in content.split('\n'):
            if 'description:' in line and '"' in line:
                description = line.split('"')[1]
                break
        else:
            description = "No description"
        
        return {
            "name": name,
            "version": version,
            "description": description,
        }
    except Exception as e:
        print(f"⚠️  Error extrayendo metadata: {e}")
        return {
            "name": "Unknown",
            "version": "1.0.0",
            "description": "No description",
        }


def upload_cartridge_api(filepath: Path) -> bool:
    """Sube un cartucho via API REST"""
    
    print(f"\n📤 Subiendo: {filepath.name}")
    
    # Leer contenido
    content = read_cartridge_file(filepath)
    if not content:
        return False
    
    # Extraer metadata
    metadata = extract_cartridge_metadata(content)
    
    # Preparar payload
    payload = {
        "name": metadata["name"],
        "version": metadata["version"],
        "description": metadata["description"],
        "code": content,
        "status": "active",
        "created_at": datetime.now().isoformat(),
    }
    
    # Preparar headers
    headers = {
        "Authorization": f"Bearer {ARBITRAGEX_ADMIN_TOKEN}",
        "Content-Type": "application/json",
    }
    
    # Enviar request
    try:
        url = f"{ARBITRAGEX_API_URL}/api/v1/cartridges"
        print(f"   POST {url}")
        print(f"   Payload: {json.dumps(payload, indent=2)[:200]}...")
        
        response = requests.post(url, json=payload, headers=headers, timeout=10)
        
        if response.status_code in [200, 201]:
            result = response.json()
            print(f"   ✅ Éxito: {metadata['name']} v{metadata['version']}")
            print(f"   ID: {result.get('id', 'N/A')}")
            return True
        else:
            print(f"   ❌ Error HTTP {response.status_code}")
            print(f"   Response: {response.text[:200]}")
            return False
    
    except requests.exceptions.ConnectionError:
        print(f"   ❌ No se pudo conectar a {ARBITRAGEX_API_URL}")
        print(f"   ¿Está corriendo ArbitrageX-v2? (docker-compose up)")
        return False
    except Exception as e:
        print(f"   ❌ Error: {e}")
        return False


def upload_all_cartridges() -> Dict[str, int]:
    """Sube todos los cartuchos en el directorio"""
    
    print("=" * 80)
    print("ArbitrageX-v2: Ensamblaje de Cartuchos via API")
    print("=" * 80)
    print(f"API URL: {ARBITRAGEX_API_URL}")
    print(f"Directorio: {CARTRIDGES_DIR}")
    print()
    
    # Encontrar archivos .rhai
    rhai_files = list(CARTRIDGES_DIR.glob("*.rhai"))
    
    if not rhai_files:
        print("⚠️  No se encontraron archivos .rhai en", CARTRIDGES_DIR)
        return {"success": 0, "failed": 0}
    
    print(f"📁 Encontrados {len(rhai_files)} cartuchos:")
    for f in rhai_files:
        print(f"   - {f.name}")
    print()
    
    # Subir cada cartucho
    results = {"success": 0, "failed": 0}
    
    for filepath in rhai_files:
        if upload_cartridge_api(filepath):
            results["success"] += 1
        else:
            results["failed"] += 1
    
    return results


def verify_uploaded_cartridges() -> List[Dict]:
    """Verifica los cartuchos subidos"""
    
    print("\n" + "=" * 80)
    print("Verificación de Cartuchos Subidos")
    print("=" * 80)
    
    headers = {
        "Authorization": f"Bearer {ARBITRAGEX_ADMIN_TOKEN}",
    }
    
    try:
        url = f"{ARBITRAGEX_API_URL}/api/v1/cartridges"
        print(f"GET {url}\n")
        
        response = requests.get(url, headers=headers, timeout=10)
        
        if response.status_code == 200:
            cartridges = response.json()
            print(f"✅ {len(cartridges)} cartuchos registrados:\n")
            
            for cart in cartridges:
                print(f"  📦 {cart.get('name', 'Unknown')}")
                print(f"     Version: {cart.get('version', 'N/A')}")
                print(f"     Status: {cart.get('status', 'N/A')}")
                print(f"     Creado: {cart.get('created_at', 'N/A')}")
                print()
            
            return cartridges
        else:
            print(f"❌ Error HTTP {response.status_code}")
            print(f"Response: {response.text}")
            return []
    
    except Exception as e:
        print(f"❌ Error verificando cartuchos: {e}")
        return []


# ═══════════════════════════════════════════════════════════════════════════════
# MAIN
# ═══════════════════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    # Subir cartuchos
    results = upload_all_cartridges()
    
    # Resumen
    print("\n" + "=" * 80)
    print("RESUMEN")
    print("=" * 80)
    print(f"✅ Éxito: {results['success']}")
    print(f"❌ Fallos: {results['failed']}")
    print()
    
    # Verificar
    if results['success'] > 0:
        verify_uploaded_cartridges()
    
    print("=" * 80)
    print("PRÓXIMOS PASOS:")
    print("=" * 80)
    print("1. Los cartuchos están registrados en la base de datos")
    print("2. El Searcher Engine los ejecutará automáticamente")
    print("3. Monitorear oportunidades en: http://localhost:3000/opportunities")
    print("4. Ver ejecuciones en: http://localhost:3000/executions")
    print()
