# Patrones Correctos (Implementation)

## Patrón 1: Behavior-Driven Testing con Testing Library
```tsx
// 🟢 CORRECTO
import { render, screen, fireEvent } from '@testing-library/react';
import { ToggleButton } from './ToggleButton';

test('Toggles compact mode when clicked', () => {
  render(<ToggleButton />);
  
  // Buscar por rol semántico, no por clase CSS o id
  const button = screen.getByRole('button', { name: /compact/i });
  expect(button).toBeInTheDocument();
  
  fireEvent.click(button);
  expect(button).toHaveAttribute('aria-pressed', 'true'); // Comportamiento a11y medible
});
```

## Patrón 2: E2E Playwright Smoke Test
```ts
// tests/smoke.spec.ts
import { test, expect } from '@playwright/test';

test('Opportunities page loads and connects', async ({ page }) => {
  await page.goto('/opportunities');
  
  // Verifica el h1
  await expect(page.locator('h1')).toContainText('Live MEV Feed');
  
  // Verifica el estado inicial de conexión
  const statusBadge = page.locator('text="POLLING"').first();
  await expect(statusBadge).toBeVisible();
});
```
