# Validación y Auditoría

## 1. Criterios de Validación
- Modificar una variable en `globals.css` (ej. `--destructive`). El cambio debe reflejarse automáticamente en botones de borrado, alertas de error y badges críticos en TODA la aplicación.
- Ejecutar un script para buscar regex: `\[#[0-9a-fA-F]{3,6}\]`. El objetivo es que las ocurrencias en los archivos `.tsx` sean cero absoluto (excluyendo casos justificados de marcas externas).

## 2. Cómo Auditar en ARBITRAGEX
- Auditar la consistencia del color verde esmeralda y cian usado en el dashboard principal. Debería estar mapeado a `--primary` o `--accent` para asegurar fácil tematización si ArbitrageX lanza un modo claro o requiere rebranding visual.
