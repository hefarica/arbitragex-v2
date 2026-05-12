# Checklist Operativo: Server Components

- [ ] ¿El componente raíz de la página (page.tsx) es un Server Component?
- [ ] ¿Se declararon directivas `use client` únicamente en el nivel más profundo posible del árbol (Leaves)?
- [ ] Si un Client Component necesita mostrar un componente que requiere acceso a base de datos, ¿se le está pasando ese componente del servidor como una prop (habitualmente `children`)?
- [ ] ¿Se evita pasar funciones no serializables o clases custom desde Server a Client a través de las props?
- [ ] ¿Se verifica que ninguna variable de entorno sensitiva sea fugada a través de las props enviadas al Client component?
