#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# ArbitrageX-v2: Script de Ensamblaje de Cartuchos via Interfaz de Archivos
# ═══════════════════════════════════════════════════════════════════════════════
#
# Script para desplegar cartuchos Rhai copiándolos directamente al directorio
# de cartuchos de ArbitrageX-v2.
#
# MODO DE USO:
# 1. Asegurarse de que ArbitrageX-v2 está corriendo
# 2. Ejecutar: bash deploy_cartridges_files.sh
#
# RESULTADO:
# - Cartuchos copiados a /opt/arbitragex-v2/backend/searcher-rs/cartridges/
# - Hot-reload automático via Redis
# - Listos para ser ejecutados
#
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuración
CARTRIDGES_SOURCE="/home/ubuntu/cartridges"
CARTRIDGES_DEST="/opt/arbitragex-v2/backend/searcher-rs/cartridges"
DOCKER_CONTAINER="arbitragex-v2-searcher-rs-1"

echo -e "${BLUE}════════════════════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}ArbitrageX-v2: Ensamblaje de Cartuchos via Interfaz de Archivos${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════════════════════════${NC}"
echo ""

# Verificar que existen los cartuchos fuente
if [ ! -d "$CARTRIDGES_SOURCE" ]; then
    echo -e "${RED}❌ Error: Directorio de cartuchos no encontrado: $CARTRIDGES_SOURCE${NC}"
    exit 1
fi

# Contar cartuchos
CARTRIDGE_COUNT=$(ls -1 "$CARTRIDGES_SOURCE"/*.rhai 2>/dev/null | wc -l)
if [ $CARTRIDGE_COUNT -eq 0 ]; then
    echo -e "${RED}❌ Error: No se encontraron archivos .rhai en $CARTRIDGES_SOURCE${NC}"
    exit 1
fi

echo -e "${GREEN}📁 Encontrados $CARTRIDGE_COUNT cartuchos:${NC}"
for file in "$CARTRIDGES_SOURCE"/*.rhai; do
    echo -e "   ${YELLOW}$(basename "$file")${NC}"
done
echo ""

# Opción 1: Copiar a directorio local (si ArbitrageX-v2 está en local)
echo -e "${BLUE}[1/3] Verificando destino...${NC}"

if [ -d "$CARTRIDGES_DEST" ]; then
    echo -e "${GREEN}✓ Directorio local encontrado: $CARTRIDGES_DEST${NC}"
    DEPLOY_LOCAL=true
else
    echo -e "${YELLOW}⚠ Directorio local no encontrado: $CARTRIDGES_DEST${NC}"
    DEPLOY_LOCAL=false
fi

# Opción 2: Copiar via Docker
echo -e "${BLUE}[2/3] Verificando Docker...${NC}"

if docker ps --filter "name=$DOCKER_CONTAINER" --format '{{.Names}}' | grep -q "$DOCKER_CONTAINER"; then
    echo -e "${GREEN}✓ Contenedor Docker encontrado: $DOCKER_CONTAINER${NC}"
    DEPLOY_DOCKER=true
else
    echo -e "${YELLOW}⚠ Contenedor Docker no encontrado${NC}"
    DEPLOY_DOCKER=false
fi

echo ""

# Desplegar
echo -e "${BLUE}[3/3] Desplegando cartuchos...${NC}"
echo ""

DEPLOYED=0
FAILED=0

# Desplegar via directorio local
if [ "$DEPLOY_LOCAL" = true ]; then
    echo -e "${YELLOW}→ Copiando a directorio local...${NC}"
    for file in "$CARTRIDGES_SOURCE"/*.rhai; do
        filename=$(basename "$file")
        echo -n "  $filename ... "
        
        if cp "$file" "$CARTRIDGES_DEST/$filename"; then
            echo -e "${GREEN}✓${NC}"
            ((DEPLOYED++))
        else
            echo -e "${RED}✗${NC}"
            ((FAILED++))
        fi
    done
    echo ""
fi

# Desplegar via Docker
if [ "$DEPLOY_DOCKER" = true ]; then
    echo -e "${YELLOW}→ Copiando via Docker...${NC}"
    for file in "$CARTRIDGES_SOURCE"/*.rhai; do
        filename=$(basename "$file")
        echo -n "  $filename ... "
        
        if docker cp "$file" "$DOCKER_CONTAINER:/opt/arbitragex-v2/backend/searcher-rs/cartridges/$filename" 2>/dev/null; then
            echo -e "${GREEN}✓${NC}"
            ((DEPLOYED++))
        else
            echo -e "${RED}✗${NC}"
            ((FAILED++))
        fi
    done
    echo ""
fi

# Trigger hot-reload via Redis
echo -e "${BLUE}Activando hot-reload...${NC}"

if command -v redis-cli &> /dev/null; then
    redis-cli PUBLISH cartridge:reload "*" > /dev/null 2>&1
    echo -e "${GREEN}✓ Hot-reload activado${NC}"
else
    echo -e "${YELLOW}⚠ redis-cli no encontrado (hot-reload manual puede ser necesario)${NC}"
fi

echo ""

# Resumen
echo -e "${BLUE}════════════════════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}RESUMEN${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}✓ Desplegados: $DEPLOYED${NC}"
if [ $FAILED -gt 0 ]; then
    echo -e "${RED}✗ Fallos: $FAILED${NC}"
fi
echo ""

# Verificar cartuchos registrados
if [ "$DEPLOY_LOCAL" = true ] || [ "$DEPLOY_DOCKER" = true ]; then
    echo -e "${BLUE}Cartuchos disponibles:${NC}"
    if [ "$DEPLOY_LOCAL" = true ]; then
        ls -lh "$CARTRIDGES_DEST"/*.rhai 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'
    elif [ "$DEPLOY_DOCKER" = true ]; then
        docker exec "$DOCKER_CONTAINER" ls -lh /opt/arbitragex-v2/backend/searcher-rs/cartridges/*.rhai 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'
    fi
fi

echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}PRÓXIMOS PASOS:${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════════════════════════${NC}"
echo "1. Los cartuchos están registrados en el sistema"
echo "2. El Searcher Engine los ejecutará automáticamente"
echo "3. Monitorear oportunidades:"
echo "   - Frontend: http://localhost:3000/opportunities"
echo "   - API: curl http://localhost:8080/api/v1/opportunities"
echo "4. Ver ejecuciones:"
echo "   - Frontend: http://localhost:3000/executions"
echo "   - API: curl http://localhost:8080/api/v1/executions"
echo ""
echo -e "${GREEN}✅ Ensamblaje completado${NC}"
echo ""
