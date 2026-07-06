import os

BASE_DIR = r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\.agents\skills"

SKILLS = [
    {
        "dir": "11-component-api-design-expert",
        "skill": r"""# Skill 11: Component API Design Expert

## 1. Propósito
Diseñar APIs de componentes React (Props) que sean predecibles, escalables, autodocumentadas y resistentes a cambios futuros. Evitar el antipatrón de componentes monolíticos con 50 props ("Prop Drilling Hell") favoreciendo la composición, la inversión de control (IoC) y el patrón Polymorphic Components.

## 2. Aplicación directa en ARBITRAGEX
La construcción de componentes reutilizables como `Card`, `Badge`, `DataTable`, `Modal` y `MetricBox` utilizados a lo largo del dashboard, configuración, y paneles de logs. Si el componente de tabla de oportunidades (`OppTable`) requiere múltiples variantes (Ej: modo compacto, modo extendido), en lugar de agregar banderas booleanas como `isCompact={true}`, se utiliza composición.

## 3. Problemas que resuelve
- Componentes Frankenstein: Un componente `Button` que recibe 30 props booleanas (`isRed`, `isLarge`, `isLoading`, `hasIconLeft`, `hasIconRight`).
- Dificultad para testear componentes debido al acoplamiento duro de sus partes internas.
- Rigidez visual: Imposibilidad de alterar una parte pequeña del componente sin añadir otra prop más.

## 4. Reglas Inmutables
- **Regla del Máximo de Props:** Si un componente de presentación (UI) tiene más de 7 props exclusivas de diseño, probablemente necesite separarse usando el patrón de Composición (pasar `children` o `slots`).
- **Inversión de Control (IoC):** En vez de pasar `data` a un `Card` y que el `Card` decida cómo renderizar el título y el cuerpo, pasa el título y el cuerpo pre-renderizados como `children`.
- Uso de `clsx` o `tailwind-merge` (`cn`) para la combinación dinámica de clases en lugar de condicionales con literales es obligatorio.
- Los componentes interactivos (Botones, Inputs) deben propagar atributos HTML nativos mediante `...props` (Rest parameters) extendiendo interfaces como `React.ButtonHTMLAttributes<HTMLButtonElement>`.

## 5. Nivel de Madurez
Senior - Diseño de SDKs de UI mantenibles a largo plazo.
""",
        "checklist": r"""# Checklist Operativo: Component API Design

- [ ] ¿El componente de UI delega las clases externas vía `className` usando la utilidad `cn` (tailwind-merge)?
- [ ] ¿Extiende los atributos HTML estándar correspondientes a su elemento raíz (`HTMLAttributes`) para no bloquear `aria-labels` o `data-testids`?
- [ ] ¿Si el componente tiene variantes visuales (primary, secondary, danger), usa una utilidad como `cva` (Class Variance Authority) en lugar de ternarios encadenados infinitos?
- [ ] ¿Se prefiere la inyección de dependencias visuales (`children`) sobre el paso de JSONs gigantes (`data={enormousObject}`)?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Composición y Slotting (Inversión de Control)
Permite máxima flexibilidad sin ensuciar la API del contenedor.

```tsx
// 🟢 CORRECTO
export function OpportunityCard({ header, body, footer }: { header: ReactNode, body: ReactNode, footer?: ReactNode }) {
  return (
    <div className="border border-slate-800 rounded-xl bg-slate-900/50">
      <div className="border-b border-slate-800 p-4">{header}</div>
      <div className="p-4">{body}</div>
      {footer && <div className="p-4 bg-slate-950/30">{footer}</div>}
    </div>
  );
}

// Uso:
<OpportunityCard 
  header={<h3 className="text-emerald-400">ETH/USDC</h3>}
  body={<Sparkline chartData={data} />}
/>
```

## Patrón 2: Class Variance Authority (CVA) + Tailwind Merge
```tsx
// 🟢 CORRECTO
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold",
  {
    variants: {
      variant: {
        default: "border-transparent bg-slate-100 text-slate-900",
        destructive: "border-transparent bg-rose-500 text-slate-50",
        outline: "text-slate-950",
      },
    },
    defaultVariants: { variant: "default" },
  }
)

export interface BadgeProps extends React.HTMLAttributes<HTMLDivElement>, VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />
}
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: El Componente Frankenstein (Prop drilling infernal)
```tsx
// 🔴 PROHIBIDO
// Añadir props booleanas por cada caso de uso visual mata la escalabilidad.
<Button 
  isRed={true} 
  isLarge={false} 
  hasIconLeft={true} 
  iconLeft={<RefreshCw />} 
  isLoading={false}
  noBorder={true}
>
  Refresh
</Button>
```

## Antipatrón 2: Bloqueo de HTML Props
```tsx
// 🔴 PROHIBIDO
interface CardProps { children: React.ReactNode, cssClass: string }
export function Card({ children, cssClass }: CardProps) {
  // Ignora onClick, aria-labels, IDs...
  return <div className={cssClass}>{children}</div> 
}
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Seleccionar un componente de UI base (Ej. `Button.tsx`). Debe permitir pasarle un `data-testid="test-btn"`, y dicho atributo debe aparecer en el DOM renderizado (Prueba de propagación de props correcta).
- La inserción de una clase custom `<Button className="mt-4" />` no debe sobreescribir las clases base del botón, sino fusionarse armónicamente gracias a `tailwind-merge`.

## 2. Cómo Auditar en ARBITRAGEX
- Buscar la carpeta `/components/ui`. Analizar la definición de las interfaces de TypeScript. Si dicen `interface Props { className?: string }` pero omiten `extends React.HTMLAttributes<HTMLElement>`, marcarlo como fallo de escalabilidad.
""",
        "agent-prompt": r"""# Prompt de Agente: Component API Design Expert

```text
Actúa como Experto en Component API Design.
Audita el siguiente componente React.
Tu objetivo:
1. Eliminar los "boolean flags" innecesarios (ej. `isPrimary`, `isDanger`) y consolidarlos en un sistema de variantes usando `cva`.
2. Asegurar que las clases de Tailwind pasadas desde el exterior no colisionen con las internas, usando la utilidad `cn` (clsx + tailwind-merge).
3. Asegurar que el componente extienda los `HTMLAttributes` nativos y use destructuring con `...props` para permitir que quien lo consuma le inyecte atributos `aria`, `data-`, o eventos estándar sin tener que definirlos uno a uno en la interfaz.
```
"""
    },
    {
        "dir": "12-design-system-engineer",
        "skill": r"""# Skill 12: Design System Engineer

## 1. Propósito
Establecer, documentar y defender una fuente única de verdad para la capa visual (Design Tokens, Typography, Espaciados, Colores) utilizando Tailwind CSS, CSS Variables y shadcn/ui. Asegurar coherencia visual absoluta en todas las pantallas del producto, promoviendo el rehuso y castigando fuertemente el "Magic Value CSS" (uso de colores hexadecimales o márgenes duros en el código).

## 2. Aplicación directa en ARBITRAGEX
El dashboard operativo de ArbitrageX maneja alta densidad de datos. Debe existir una paleta estricta de severidad: `emerald` para Profit/OK, `rose` para Fallos/Pérdidas, `amber` para Alertas/Slippage. Todos los fondos deben obedecer a escalas semánticas (`bg-background`, `bg-muted`, `bg-card`) permitiendo el soporte multi-tema (Dark/Industrial).

## 3. Problemas que resuelve
- Inconsistencia visual (Ej: 5 tonos de verde diferentes en la misma tabla).
- Mantenimiento imposible de temas (Imposibilidad de cambiar el branding sin hacer Buscar/Reemplazar 1000 veces).
- UI Infantil o falta de jerarquía que rompe la inmersión de un panel "HFT / Profesional".
- Tailwind CSS bloating (archivos gigantes llenos de clases utilitarias arbitrarias como `bg-[#1a1b1e]`).

## 4. Reglas Inmutables
- **Cero Hexadecimales Locales:** Prohibido usar clases arbitrarias como `text-[#ff0000]` dentro de los componentes. Todos los colores deben venir del `tailwind.config.js` (`text-rose-500` o `text-destructive`).
- **Escala de Espaciado Estricta:** Usar los rems estándar de Tailwind (`p-4`, `m-2`, `gap-8`). Prohibido usar píxeles fijos como `mt-[17px]`.
- Variables CSS para colores semánticos (`--background`, `--foreground`, `--destructive`) en `globals.css` inyectadas en la configuración base.
- Si un bloque de Tailwind supera las 6-7 clases y no es un componente base primitivo, evaluar si la lógica pertenece a un archivo de variantes (`cva`).

## 5. Nivel de Madurez
Senior - Crea la gramática visual de la empresa.
""",
        "checklist": r"""# Checklist Operativo: Design System

- [ ] ¿El archivo `globals.css` y `tailwind.config.ts` contienen la definición de colores semánticos basados en variables CSS (`hsl`)?
- [ ] ¿Se purgaron todos los colores arbitrarios (`bg-[#xxxxxx]`) en las páginas (`page.tsx`) en favor de los design tokens del sistema?
- [ ] ¿Existe una semántica de severidad? (`destructive`, `warning`, `success`, `muted`, `accent`).
- [ ] ¿El panel se diseñó "Dark Mode First" o con soporte a alternancia dinámica a través de `next-themes`?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: CSS Variables Semánticas (globals.css)
```css
@layer base {
  :root {
    --background: 222.2 84% 4.9%;
    --foreground: 210 40% 98%;
    --card: 222.2 84% 4.9%;
    --card-foreground: 210 40% 98%;
    --primary: 210 40% 98%;
    --primary-foreground: 222.2 47.4% 11.2%;
    --destructive: 0 62.8% 30.6%;
    --destructive-foreground: 210 40% 98%;
  }
}
```

## Patrón 2: Consumo Semántico de Tailwind
```tsx
// 🟢 CORRECTO: Uso de semántica.
export function ErrorAlert({ message }) {
  return (
    <div className="bg-destructive/20 border border-destructive text-destructive-foreground p-4 rounded-md">
      {message}
    </div>
  );
}
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Magic Values & Hardcoded Colors
Rompe el Design System, hace imposible soportar temas claros/oscuros o rediseños de marca.

```tsx
// 🔴 PROHIBIDO
export function ErrorAlert({ message }) {
  // Si mañana el cliente decide que el rojo de la empresa es "#ff3333", hay que buscar en todos los archivos.
  return (
    <div className="bg-[#ff0000] text-[#ffffff] p-[15px] border-[#cc0000]">
      {message}
    </div>
  );
}
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Modificar una variable en `globals.css` (ej. `--destructive`). El cambio debe reflejarse automáticamente en botones de borrado, alertas de error y badges críticos en TODA la aplicación.
- Ejecutar un script para buscar regex: `\[#[0-9a-fA-F]{3,6}\]`. El objetivo es que las ocurrencias en los archivos `.tsx` sean cero absoluto (excluyendo casos justificados de marcas externas).

## 2. Cómo Auditar en ARBITRAGEX
- Auditar la consistencia del color verde esmeralda y cian usado en el dashboard principal. Debería estar mapeado a `--primary` o `--accent` para asegurar fácil tematización si ArbitrageX lanza un modo claro o requiere rebranding visual.
""",
        "agent-prompt": r"""# Prompt de Agente: Design System Engineer

```text
Actúa como Ingeniero de Design Systems.
Inspecciona el JSX provisto y purga los siguientes pecados capitales de Tailwind:
1. Valores arbitrarios de color (`text-[#ff0033]`). Reemplázalos con colores semánticos del sistema (ej. `text-destructive`, `text-rose-500`).
2. Valores arbitrarios de espaciado (`mt-[14px]`). Reemplázalos por escalas de rem estándar (ej. `mt-3`, `mt-4`).
3. Sombras o bordes hardcodeados. Usa los tokens de `shadow-sm`, `border-border`.
El objetivo es que el componente sea 100% resiliente a un cambio de tema global controlado por CSS variables.
```
"""
    },
    {
        "dir": "13-accessibility-semantic-html-doctor",
        "skill": r"""# Skill 13: Accessibility & Semantic HTML Doctor

## 1. Propósito
Garantizar que el HTML emitido por la aplicación React/Next sea sintácticamente correcto, semánticamente rico, y accesible (a11y) tanto para Screen Readers (Lectores de pantalla), indexadores (SEO/Bots) y navegadores de teclado. Prevenir fallos de hidratación causados por nido ilegal de etiquetas.

## 2. Aplicación directa en ARBITRAGEX
El uso correcto de roles de tabla en la tabla de Oportunidades, botones interactivos bien etiquetados (`aria-label` en botones con solo íconos), manejo correcto del Focus en los modales de configuración, y estructura limpia (`<main>`, `<aside>`, `<header>`) en el Layout para que sea navegable rápidamente sin ratón.

## 3. Problemas que resuelve
- Errores graves de hidratación en React por meter `<div>` dentro de `<p>`. (El navegador corrige el HTML, React detecta mismatch y estalla).
- Interfaz inutilizable por teclado (Tab no enfoca, Enter no activa).
- Modales que no atrapan el Focus (Focus Traps) permitiendo interactuar con el fondo.
- Botones compuestos por `<div>onClick</div>` en lugar de `<button>`.

## 4. Reglas Inmutables
- Nunca envolver elementos block-level (`<div>`, `<ul>`) dentro de elementos inline estrictos (`<p>`, `<span>`). React detesta esto durante la hidratación.
- Un elemento interactivo que realiza una acción en la misma página SIEMPRE debe ser un `<button>`. Si navega a una ruta distinta, SIEMPRE debe ser un `<a href>` (o `<Link>`). No inventar botones hechos con `<div>`.
- Todo componente que solo contenga un Icono (ej. un basurero) DEBE tener un `aria-label` descriptivo o texto de lectura exclusiva de pantalla (`sr-only`).
- Las imágenes DEBEN contener el atributo `alt`.

## 5. Nivel de Madurez
Profesional - Estándar mínimo de calidad web y respeto al usuario y motor del navegador.
""",
        "checklist": r"""# Checklist Operativo: Accessibility & Semantics

- [ ] ¿Hay botones implementados con `div` o `span`? (Si tienen `onClick`, deben ser `<button>`).
- [ ] ¿Los botones de sólo icono (Ej. `<Zap size={18} />`) poseen `aria-label` o título accesible?
- [ ] ¿Las secciones maestras usan `main`, `aside`, `nav`, `header`, `footer` en lugar de una sopa gigante de `<div>`?
- [ ] ¿No existen anidamientos ilegales de HTML (Párrafos conteniendo Divs, Listas `ul` conteniendo algo distinto a `li`)?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Icon Buttons Seguros
```tsx
// 🟢 CORRECTO
import { RefreshCw } from 'lucide-react';

export function RefreshButton({ onRefresh }) {
  return (
    <button 
      onClick={onRefresh} 
      className="p-2 hover:bg-slate-800 rounded-md"
      aria-label="Refresh live feed" // Crítico para A11y y Bots
      title="Refresh"
    >
      <RefreshCw aria-hidden="true" />
    </button>
  );
}
```

## Patrón 2: Estructura Semántica del DOM
```tsx
// 🟢 CORRECTO
export function DashboardLayout({ sidebar, children }) {
  return (
    <div className="flex">
      <header className="sr-only">ArbitrageX Admin</header>
      <aside aria-label="Sidebar Navigation">{sidebar}</aside>
      <main id="main-content" className="flex-1">
        {children}
      </main>
    </div>
  );
}
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Nido Ilegal Causa Hydration Error #425
```tsx
// 🔴 PROHIBIDO: Esto romperá la hidratación en React
export function BadComponent() {
  return (
    <p>
      Bienvenido al sistema.
      <div className="mt-4">
        Tus métricas están cargando...
      </div>
    </p>
  );
}
// El navegador convierte esto en: <p>Bienvenido</p><div...</div><p></p>
// React ve el DOM diferente al SSR y entra en pánico de hidratación.
```

## Antipatrón 2: El Div Interactivo Ciego
```tsx
// 🔴 PROHIBIDO
<div onClick={submitForm} className="btn-primary cursor-pointer hover:bg-blue-600">
  Submit
</div>
// Imposible activar con "Enter" o "Space". No recibe foco natural del "Tab".
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Navegar la aplicación entera usando SOLAMENTE el botón `Tab`, `Enter` y `Espacio` en el teclado. Todos los modales, botones, inputs y filtros deben ser alcanzables y activables.
- Inspeccionar el DOM con una herramienta validadora W3C o Lighthouse (Accessibility Score > 90).

## 2. Cómo Auditar
- Buscar `onClick` asociado a etiquetas `<div>`, `<span>`, `<a>` sin `href`, o `<svg>`. Exigir la transformación a `<button>`.
- Buscar `<p>` en los códigos y verificar visualmente que no encierren `div`, `table` ni `ul`.
""",
        "agent-prompt": r"""# Prompt de Agente: Semantic HTML Doctor

```text
Actúa como Auditor de Accesibilidad y Semántica Web (A11y Doctor).
Revisa rigurosamente este componente React:
1. Detecta y alerta sobre anidamientos de HTML ilegales que puedan provocar React Hydration Errors (ej. Block-levels dentro de Inline-levels).
2. Transforma cualquier `div` o `span` que posea un `onClick` en un `<button type="button">`.
3. Inyecta `aria-label` en cualquier botón que solo contenga un ícono SVG.
4. Asegura que los formularios usen `<form>` nativo y envíos `onSubmit` en lugar de interceptar `onClick` en los botones aislados.
```
"""
    },
    {
        "dir": "14-performance-budget-commander",
        "skill": r"""# Skill 14: Performance Budget Commander

## 1. Propósito
Monitorear, defender y optimizar el peso global del bundle de JavaScript enviado al cliente. Mantener Core Web Vitals (LCP, FID, CLS, INP) en rangos de excelencia. Dominar Dynamic Imports (`next/dynamic`), code-splitting y el control del impacto de dependencias pesadas (Charts, Date libraries, Mapas) sobre el TTI (Time to Interactive).

## 2. Aplicación directa en ARBITRAGEX
El dashboard tiene o tendrá bibliotecas de visualización de datos de la red blockchain y gráficas de profit. Si se importa una librería gigantesca como `echarts` o `d3` en un Client Component raíz, destruiremos el tiempo de carga del usuario. Esas bibliotecas deben empaquetarse separadamente y cargarse asíncronamente (Lazy Loading) solo cuando entran en la pantalla.

## 3. Problemas que resuelve
- First Load / Time to Interactive lentsísimo (White Screen).
- Bundles de JS superiores a 1MB parseados en dispositivos de gama media/baja.
- Impacto severo al Interaction to Next Paint (INP) porque el Main Thread está bloqueado evaluando megabytes de código en el primer frame.

## 4. Reglas Inmutables
- Toda librería pesada de terceros orientada exclusivamente a un widget (Gráficos estadísticos, mapas, editores de texto rico, animaciones lottie masivas) DEBE ser importada dinámicamente (`next/dynamic` o `React.lazy`).
- Prefiere alternativas modernas o funciones nativas (Intl.DateTimeFormat) en lugar de librerías utilitarias masivas y anticuadas como `moment.js` o `lodash` completo.
- Mantener el First Load JS del bundle inicial en Next.js (Visible durante el npm run build) estrictamente inferior a 150KB gzip.

## 5. Nivel de Madurez
Senior - Defiende el rendimiento bajo estrés en redes lentas.
""",
        "checklist": r"""# Checklist Operativo: Performance Budget

- [ ] Durante `npm run build`, ¿Next.js muestra que el bundle de la ruta principal (`/`) está en color verde (pequeño) y no en amarillo/rojo?
- [ ] ¿Los componentes de gráficos (Charts) están importados usando `next/dynamic` con un indicador de carga (`loading: () => <Skeleton />`)?
- [ ] ¿Se auditaron las dependencias de fechas (reemplazar Moment por date-fns o nativo)?
- [ ] ¿Las imágenes estáticas están utilizando el componente nativo `<Image>` de Next.js (`next/image`) con tamaños explícitos para evitar Cumulative Layout Shift (CLS)?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Lazy Loading Heavy Dependencies
```tsx
// 🟢 CORRECTO
import dynamic from 'next/dynamic';

// Este componente gigante no bloqueará la carga de la página principal.
const HeavyChart = dynamic(() => import('@/components/HeavyChart'), {
  ssr: false, // Opcional: Deshabilita el render en servidor si depende de 'window' o Canvas
  loading: () => <div className="h-[400px] bg-slate-900 animate-pulse rounded-xl" />
});

export default function AnalyticsDashboard() {
  return (
    <div>
      <MetricsHeader />
      <HeavyChart />
    </div>
  );
}
```

## Patrón 2: Tree-shakable Imports
```tsx
// 🟢 CORRECTO: Solo importa la utilidad exacta, permitiendo descartar el resto del paquete.
import { formatDistanceToNow } from 'date-fns/formatDistanceToNow';
// o en librerías modernas como lodash-es:
import debounce from 'lodash-es/debounce';
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Sincronía en el Monolito Client-Side
```tsx
// 🔴 PROHIBIDO: Esto arrastrará toda la librería 'recharts' o 'three.js' al bundle principal
'use client';

import { 3DModelViewer } from 'heavy-3d-lib';
import { LineChart, Tooltip, XAxis } from 'recharts';

export function Dashboard() {
  // ...
}
```

## Antipatrón 2: Layout Shift por Falta de Dimensiones
```tsx
// 🔴 PROHIBIDO: Provoca CLS (Salto del layout) al cargar
<img src="/logo.png" />

// 🟢 CORRECTO
import Image from 'next/image';
<Image src="/logo.png" width={200} height={50} alt="Logo" priority />
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Auditar con Lighthouse en Chrome (Modo incógnito, Simulate Mobile). El Score de Performance debe ser mayor a 90.
- Correr `@next/bundle-analyzer` durante el build. Identificar cualquier chunk único de JavaScript que exceda los 200KB minificado.

## 2. Cómo Auditar en ARBITRAGEX
- Revisar `package.json`. Advertir y marcar reemplazos si se descubren dependencias prohibidas u obsoletas.
- Revisar el código fuente buscando componentes de diagramas o librerías pesadas, y exigir que su importación sea envuelta en `dynamic`.
""",
        "agent-prompt": r"""# Prompt de Agente: Performance Budget Commander

```text
Actúa como Comandante del Rendimiento Web.
Examina las importaciones de este archivo `.tsx` o `.ts`.
Tu objetivo es blindar el TTI (Time to Interactive) y el peso de la red:
1. Detecta importaciones de bibliotecas grandes (gráficos, mapas, tablas complejas de terceros).
2. Reescribe esas importaciones estáticas para usar `next/dynamic` con `{ ssr: false }` si acceden al DOM del cliente, incluyendo un fallback `loading: () => <Skeleton />`.
3. Detecta importaciones de librerías globales de utilidades (`lodash`, `moment`) y sugiera o refactorice a Named Imports específicos o APIs nativas del navegador (Intl, Date nativo).
4. Asegura que las imágenes usen `next/image` con tamaños y la propiedad `priority` si se encuentran "Above the Fold".
```
"""
    },
    {
        "dir": "15-bundle-runtime-optimization-expert",
        "skill": r"""# Skill 15: Bundle & Runtime Optimization Expert

## 1. Propósito
Asegurar que el motor de JavaScript, el runtime de Node (Servidor) y el motor V8 (Cliente) corran con la menor fricción posible. Eliminar memory leaks (fugas de memoria) debidas a closures no liberados en React, optimizar algoritmos recursivos o iterativos pesados y garantizar un uso eficiente de Web Workers en casos de cómputo intensivo del lado del cliente.

## 2. Aplicación directa en ARBITRAGEX
ArbitrageX procesa transacciones, cálculos de simulaciones de ganancia, validación de Zod profunda en grandes arreglos de datos cada segundo. Una ineficiencia asintótica `O(N^2)` dentro de la búsqueda de oportunidades colapsaría la pestaña de Chrome u obligaría al servidor Node a entrar en pánico por saturación del Event Loop.

## 3. Problemas que resuelve
- Picos de CPU y cuellos de botella en el Event Loop (Node.js/Edge).
- Pestañas de Chrome que crashean con el mensaje "Out of Memory" o "Aw, Snap!".
- Lentitud progresiva (La aplicación funciona bien el minuto 1, pero al minuto 30 está intocable).
- Tiempos de parseo JSON masivos bloqueando el hilo principal.

## 4. Reglas Inmutables
- Cierre estricto de suscripciones: TODO `setInterval`, `addEventListener`, o conexión de `socket.on` debe tener su limpieza (`clearInterval`, `removeEventListener`, `socket.off`) en el *Cleanup Function* del `useEffect`.
- Si se deben iterar, transformar o filtrar miles de objetos en el front-end frecuentemente, usa un `useMemo` con una key de dependencia exacta. No lo recalcules en el cuerpo de la función.
- Evita mutaciones y clonaciones masivas (`JSON.parse(JSON.stringify(obj))` o `{...giantObject}`) dentro del render loop en secuencias muy rápidas (ej. eventos de scroll o resize).

## 5. Nivel de Madurez
Maestría - Debugging avanzado en Chrome Memory Profiler.
""",
        "checklist": r"""# Checklist Operativo: Runtime Optimization

- [ ] ¿Están todas las suscripciones de los WebSockets debidamente canceladas al desmontar el componente o al cambiar la dependencia del Effect?
- [ ] ¿Existen bucles anidados `O(N^2)` procesando payloads del servidor que podrían pre-calcularse mediante mapas hash `O(1)`?
- [ ] En contextos Reactivos: ¿Se declaran objetos u arrays pesados fuera del cuerpo del componente o estáticos en lugar de recrearlos en cada frame?
- [ ] ¿Hay un control explícito del tamaño histórico de datos vivos (truncando arrays antiguos para evitar memoria infinita)?
""",
        "implementation-patterns": r"""# Patrones Correctos (Implementation)

## Patrón 1: Efectos Libres de Fugas
```tsx
// 🟢 CORRECTO
useEffect(() => {
  const handleScroll = throttle(() => { ... }, 100);
  
  window.addEventListener('scroll', handleScroll);
  
  // Limpieza estricta: Previene que si el componente se desmonta 10 veces,
  // haya 10 oyentes fantasmas robando memoria.
  return () => window.removeEventListener('scroll', handleScroll);
}, []);
```

## Patrón 2: Optimización O(N) Hash Maps
En el frontend de ArbitrageX, al recibir una lista de IDs actualizada desde el backend, cruzarla contra el estado existente de manera rápida.

```tsx
// 🟢 CORRECTO: Búsqueda O(1) usando Set
const highlightIds = useMemo(() => new Set(newUpdates.map(u => u.id)), [newUpdates]);

// O(N) en lugar de O(N^2)
const result = items.map(item => ({
  ...item,
  isUpdated: highlightIds.has(item.id)
}));
```
""",
        "anti-patterns": r"""# Antipatrones Prohibidos

## Antipatrón 1: Array Mutation Infinito (Memory Leak Vivo)
En sistemas de monitoreo vivos, si guardamos todos los logs sin un techo.

```tsx
// 🔴 PROHIBIDO
socket.on('log', (log) => {
   // A las 5 horas, el array "logs" tendrá millones de objetos. Crash garantizado.
   setLogs(prev => [log, ...prev]); 
});

// 🟢 CORRECTO
socket.on('log', (log) => {
   // Garbage collection forzoso por límite (Ej: 1000 logs máx).
   setLogs(prev => [log, ...prev].slice(0, 1000)); 
});
```

## Antipatrón 2: Recreación de constantes u objetos en cada render
```tsx
export function MyComponent() {
  // 🔴 PROHIBIDO: Este objeto y la regex se recrean en memoria cientos de veces por segundo.
  const DEFAULT_CONFIG = { retries: 3, timeout: 5000 }; 
  const regex = /^[a-z0-9]+$/i;
  // ...
}

// 🟢 CORRECTO: Se definen FUERA del componente o dentro de un useMemo.
const DEFAULT_CONFIG = { retries: 3, timeout: 5000 }; 
const REGEX = /^[a-z0-9]+$/i;

export function MyComponent() {
  // ...
}
```
""",
        "validation": r"""# Validación y Auditoría

## 1. Criterios de Validación
- Ejecutar un Test de Estabilidad ("Soak Test"). Dejar la pestaña abierta 4 horas recibiendo WebSockets. La memoria JS en Task Manager de Chrome no debe pasar de un baseline estable (+/- 30MB del inicio).
- Activar la herramienta Memory Allocation Profiler en Chrome. Validar que no haya "Detached DOM Nodes" persistiendo tras navegar entre pestañas (Señal de EventListeners huérfanos).

## 2. Cómo Auditar
- Inspeccionar todas las dependencias devueltas en los `useEffect` de la aplicación (El bloque `return () => {}`). Todo `useEffect` que haga attach de un socket, fetch interval, u observer global DEBE tener un return limpiador. Si no lo tiene, es una fuga de memoria.
""",
        "agent-prompt": r"""# Prompt de Agente: Bundle & Runtime Optimization Expert

```text
Actúa como Arquitecto de Runtime y Garbage Collection de V8.
Inspecciona críticamente el código de este archivo React/TS buscando fugas de memoria y destrucción del Event Loop:
1. Asegura que cada `useEffect` que abra una conexión persistente (Socket, setInterval, Window Event Listener) declare un callback en el `return` desconectando y limpiando el recurso.
2. Identifica arrays reactivos puros de tiempo real (logs, histórico de transacciones) y fórce un truncamiento preventivo usando `.slice(0, MAX_LIMIT)` para evitar Memory Leaks por crecimiento infinito.
3. Detecta cruces de datos en arrays (`.filter`, `.map`, `.find` anidados) que posean complejidad O(N^2) y refactorízalos construyendo índices en mapa (`Map` o `Set`) O(1) antes de iterar, encapsulados en `useMemo`.
4. Saca del cuerpo del componente (hacia el root del archivo) todas las variables constantes y objetos predefinidos para evitar recreación de basura en memoria.
```
"""
    }
]

for item in SKILLS:
    skill_dir = os.path.join(BASE_DIR, item["dir"])
    os.makedirs(skill_dir, exist_ok=True)
    
    with open(os.path.join(skill_dir, "skill.md"), "w", encoding="utf-8") as f:
        f.write(item["skill"])
    with open(os.path.join(skill_dir, "checklist.md"), "w", encoding="utf-8") as f:
        f.write(item["checklist"])
    with open(os.path.join(skill_dir, "implementation-patterns.md"), "w", encoding="utf-8") as f:
        f.write(item["implementation-patterns"])
    with open(os.path.join(skill_dir, "anti-patterns.md"), "w", encoding="utf-8") as f:
        f.write(item["anti-patterns"])
    with open(os.path.join(skill_dir, "validation.md"), "w", encoding="utf-8") as f:
        f.write(item["validation"])
    with open(os.path.join(skill_dir, "agent-prompt.md"), "w", encoding="utf-8") as f:
        f.write(item["agent-prompt"])

print("Batch 3 generated successfully!")
