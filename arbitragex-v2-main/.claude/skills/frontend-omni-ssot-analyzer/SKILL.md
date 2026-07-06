---
name: frontend-omni-ssot-analyzer
description: Analyze frontend architectures and generate comprehensive SSOT (Single Source of Truth) mapping documents for React/Next.js projects. Use for auditing frontend codebases, identifying redundant API calls, planning state management refactoring, and designing efficient data flows.
---

# Frontend Omni-SSOT Analyzer

This skill provides a systematic approach to analyzing React/Next.js frontend architectures and generating a comprehensive Single Source of Truth (SSOT) mapping document. It enforces the "Eficiencia Absoluta" doctrine: reusing existing presentation code, centralizing state in Zustand, and eliminating redundant data fetching.

## When to use this skill

- When asked to audit a frontend codebase for performance or state management issues
- When tasked with designing a refactoring plan for a React/Next.js application
- When you need to map how data flows from backend to multiple UI components
- When requested to generate an architecture document for frontend state

## Step 1: Analyze the Frontend Codebase

First, use the provided Python script to automatically scan the project structure and identify pages, hooks, schemas, and components.

```bash
python /home/ubuntu/skills/frontend-omni-ssot-analyzer/scripts/analyze_frontend.py <path-to-project-root>
```

This script will output:
- Total number of pages and their routes
- Custom hooks (potential SSOT selectors)
- Zod/TypeScript schemas (canonical entities)
- Components categorized by type

Save the output to a text file for reference during the documentation phase.

## Step 2: Identify Dynamic Entities

Review the schemas and API calls in the codebase to identify the core dynamic entities that should live in the SSOT store. 

For each entity, determine:
1. Its canonical structure (TypeScript interface)
2. Which pages consume it
3. How it should be selected from the store (the SSOT pattern)

## Step 3: Study SSOT Architecture Patterns

Before designing the solutions, read the reference document on SSOT patterns:

```bash
cat /home/ubuntu/skills/frontend-omni-ssot-analyzer/references/ssot_patterns.md
```

This document covers crucial patterns for:
- Derived state via memoized selectors
- High-volume data rendering (virtualized lists)
- Cascading selectors without refetching
- Dynamic WebSocket injection

## Step 4: Generate the OMNI-SSOT Map Document

Create the final architecture document using the provided template. 

1. Copy the template to the project's documentation folder:
```bash
cp /home/ubuntu/skills/frontend-omni-ssot-analyzer/templates/omni_ssot_map_template.md <path-to-project>/docs/architecture/FRONTEND_OMNI_SSOT_MAP.md
```

2. Fill in the template using the insights gathered in Steps 1 and 2, applying the patterns from Step 3.

### Required Sections in the Output Document:

- **Resumen Ejecutivo**: Overview of the SSOT strategy and the "Eficiencia Absoluta" rule.
- **Inventario de Entidades Dinámicas**: Detailed mapping of each core entity, the pages that consume it, and the specific code pattern to use.
- **Omni-Diagrama Mermaid End-to-End**: A comprehensive Mermaid diagram showing the flow from Backend -> SSOT Store -> Hooks -> Pages.
- **SOP de Interconexión Avanzada**: Specific code solutions for complex data interactions (e.g., filtering large arrays, WebSocket subscriptions).
- **Plan de Despliegue**: A phased approach to implementing the refactor (e.g., Phase 1: Read-only pages, Phase 2: Mutations, Phase 3: WebSockets).
- **Mapeo Página-por-Página**: A comprehensive table listing every page, its purpose, required SSOT hooks, mutations, and WebSocket streams.

## Step 5: Deliver the Result

Commit the generated document to the repository and notify the user that the analysis is complete. Ensure the commit message clearly summarizes the scope of the analysis and the key architectural decisions.
