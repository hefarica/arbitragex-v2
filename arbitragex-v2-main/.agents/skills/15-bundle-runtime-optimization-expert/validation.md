# Validación y Auditoría

## 1. Criterios de Validación
- Ejecutar un Test de Estabilidad ("Soak Test"). Dejar la pestaña abierta 4 horas recibiendo WebSockets. La memoria JS en Task Manager de Chrome no debe pasar de un baseline estable (+/- 30MB del inicio).
- Activar la herramienta Memory Allocation Profiler en Chrome. Validar que no haya "Detached DOM Nodes" persistiendo tras navegar entre pestañas (Señal de EventListeners huérfanos).

## 2. Cómo Auditar
- Inspeccionar todas las dependencias devueltas en los `useEffect` de la aplicación (El bloque `return () => {}`). Todo `useEffect` que haga attach de un socket, fetch interval, u observer global DEBE tener un return limpiador. Si no lo tiene, es una fuga de memoria.
