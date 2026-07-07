# Checklist Operativo: Accessibility & Semantics

- [ ] ¿Hay botones implementados con `div` o `span`? (Si tienen `onClick`, deben ser `<button>`).
- [ ] ¿Los botones de sólo icono (Ej. `<Zap size={18} />`) poseen `aria-label` o título accesible?
- [ ] ¿Las secciones maestras usan `main`, `aside`, `nav`, `header`, `footer` en lugar de una sopa gigante de `<div>`?
- [ ] ¿No existen anidamientos ilegales de HTML (Párrafos conteniendo Divs, Listas `ul` conteniendo algo distinto a `li`)?
