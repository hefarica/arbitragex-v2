# Antipatrones Prohibidos

## Antipatrón 1: Consola Ciega en Prod
```tsx
// 🔴 PROHIBIDO
try {
  await doDangerousThing();
} catch (e) {
  console.log("Error", e); // Nadie verá esto nunca en producción. El operador cerrará la pestaña quejándose.
  showToast("Ocurrió un error");
}
```
