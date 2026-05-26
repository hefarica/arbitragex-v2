---
name: office-master-phd-excel
description: Automatizar Excel y Microsoft Office con nivel PhD usando scripts de Python, COM, manejo asíncrono de diálogos y generación de reportes integrados.
---
# office-master-phd-excel

## Purpose
Automatizar tareas complejas de Microsoft Excel y Office (VBA macros, manipulación de celdas, formateo de hojas, exportaciones de reportes PDF/Markdown) mediante scripts externos de Python, asegurando ejecución robusta de macros con desestimación asíncrona de ventanas emergentes (diálogos modales y MsgBox).

## When to use
Cuando necesites interactuar con planillas de cálculo Excel, ejecutar macros automatizadas que requieran clics o confirmaciones asíncronas, o poblar y extraer datos de dashboards financieros y operativos sin requerir intervención humana directa.

## Inputs needed
- Ruta absoluta al archivo Excel (`.xlsm`, `.xlsx`).
- Código del macro a ejecutar u hoja y celda a manipular.
- Especificación del reporte a generar (.pdf, .md, .zip).

## Files usually touched
- Archivos `.bas` (VBA modules).
- Archivos `.xlsm` (Workbooks habilitados para macros).
- Scripts de control (`.py`).

## Python Automation Blueprint (PhD Level COM and Dialog Interceptor)
Para interactuar de forma segura con Excel en Windows, utiliza la API COM de `win32com.client` junto con un hilo de interceptación para evitar bloqueos por ventanas MsgBox modales.

```python
import win32com.client
import win32gui
import win32con
import threading
import time

def msgbox_dismiss_thread():
    start_time = time.time()
    while time.time() - start_time < 30: # 30s timeout
        hwnd = win32gui.FindWindow("#32770", "Window Title Here")
        if hwnd:
            # Post command message to press 'Aceptar' (ID 1)
            win32gui.PostMessage(hwnd, win32con.WM_COMMAND, 1, 0)
            break
        time.sleep(0.5)

# Iniciar el interceptor asíncrono
t = threading.Thread(target=msgbox_dismiss_thread, daemon=True)
t.start()

# Iniciar instancia de Excel COM
excel = win32com.client.Dispatch("Excel.Application")
excel.Visible = True
excel.DisplayAlerts = False

wb = excel.Workbooks.Open(r"C:\path\to\file.xlsm")
ws = wb.Sheets("SheetName")

# Escribir o leer celdas
ws.Range("E8").Value = "Valor de prueba"

# Ejecutar macro de VBA
excel.Run("MacroName")

# Guardar y cerrar
wb.Close(SaveChanges=True)
excel.Quit()
```

## Safety rules
1. **Always Kill Stale Excel Instances**: Asegura limpiar el estado de Excel antes de ejecutar automatizaciones corriendo `taskkill /f /im excel.exe`.
2. **Handle Excel UTF-8 Output buffering**: En Windows PowerShell, usa `sys.stdout.reconfigure(encoding='utf-8')` en tus scripts para evitar fallas en la salida unicode.
3. **No Mocks**: No inyectes celdas vacías o con placeholders simulados; si hay error en una llamada LLM, usa un valor real de fallo formal (ej. `"LLM - ERROR DE API"`).

## Verification steps
1. Ejecuta el script de control de Excel.
2. Verifica visualmente o mediante queries COM que las celdas se modifiquen correctamente.
3. Confirma la existencia de los archivos exportados (PDF, MD) en los directorios correspondientes.
