# Checklist Operativo: Runtime Optimization

- [ ] ¿Están todas las suscripciones de los WebSockets debidamente canceladas al desmontar el componente o al cambiar la dependencia del Effect?
- [ ] ¿Existen bucles anidados `O(N^2)` procesando payloads del servidor que podrían pre-calcularse mediante mapas hash `O(1)`?
- [ ] En contextos Reactivos: ¿Se declaran objetos u arrays pesados fuera del cuerpo del componente o estáticos en lugar de recrearlos en cada frame?
- [ ] ¿Hay un control explícito del tamaño histórico de datos vivos (truncando arrays antiguos para evitar memoria infinita)?
