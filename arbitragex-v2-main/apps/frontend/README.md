# ArbitrageX V2 Frontend

## Overview

The ArbitrageX V2 frontend is a React-based dashboard for monitoring the
trading system. It provides real-time opportunity feeds, execution telemetry,
risk metrics, and administrative controls including the kill-switch.

## Quick Start

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Run unit tests
npm run test

# Run E2E tests
npm run test:e2e

# Build for production
npm run build
```

## Architecture

```
frontend/
  src/
    components/      # Reusable UI components
    pages/           # Route-level page components
    hooks/           # Custom React hooks
    services/        # API and WebSocket clients
    stores/          # Zustand state management
    utils/           # Shared utilities
  tests/
    e2e/             # Playwright end-to-end tests
    unit/            # Vitest unit tests
  .storybook/        # Storybook configuration
  public/            # Static assets
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | React 18 + TypeScript |
| Bundler | Vite |
| Styling | Tailwind CSS |
| State | Zustand |
| Testing | Vitest + Playwright |
| Docs | Storybook |
| Charts | Recharts |

## Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `VITE_API_URL` | REST API base URL | Yes |
| `VITE_WS_URL` | WebSocket base URL | Yes |
| `VITE_ADMIN_TOKEN` | Admin token for privileged ops | Dev only |

## API Client

The frontend uses a typed Axios client generated from `apis/openapi.yaml`.
Regenerate after API changes:

```bash
npm run generate-api
```

## WebSocket Channels

- `opportunities` — Real-time opportunity feed
- `executions` — Execution lifecycle events
- `system` — Guard state and kill-switch events

## Storybook

```bash
npm run storybook
```

Components are documented at `http://localhost:6006`.

## Testing Strategy

| Test Type | Tool | Coverage Target |
|-----------|------|-----------------|
| Unit | Vitest | 80% |
| Integration | React Testing Library | Key user flows |
| E2E | Playwright | Critical paths |

## Contributing

1. Branch from `main` with prefix `feat/`, `fix/`, or `chore/`.
2. Write tests for new features.
3. Update Storybook stories for UI changes.
4. Ensure E2E tests pass before requesting review.

## Deployment

Production builds are deployed via CI/CD pipeline:

```
main branch → Docker build → Staging → Production
```

See `pipelines/_README.md` for pipeline details.