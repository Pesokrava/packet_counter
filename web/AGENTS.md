# AGENTS.md — web (React frontend)

## Purpose

React SPA dashboard that polls the Axum backend (`GET /api/stats`) and displays
per-port packet counts as a bar chart and sortable table. Pure client-side app
with no server-side rendering.

## Tech Stack

- **React 18.3** with functional components and hooks
- **TypeScript 5.5** in strict mode
- **Vite 5.4** for bundling and dev server
- **No CSS framework** — plain CSS with custom dark theme (`App.css`)
- **No router** — single-page, no client-side routing
- **No state management library** — React `useState` + `useMemo`
- **No test framework** — not yet configured

## Build & Dev

```sh
make build-web      # cd web && npm install && npm run build → web/dist/
make dev-frontend   # npm --prefix web run dev (Vite dev server on :5173)
make build-all      # frontend then Rust binary

# Inside web/:
npm run dev         # Vite dev server with HMR
npm run build       # tsc && vite build → dist/
npm run preview     # Preview production build
```

The Vite dev server proxies `/api` requests to `http://localhost:3001` (the Rust
backend). See `vite.config.ts`.

## File Structure

```
src/
  main.tsx         — React root render (StrictMode + createRoot)
  App.tsx          — Main component + helper components (~182 lines)
  App.css          — Dark-theme dashboard styles
  useStats.ts      — Custom hook: polls /api/stats with AbortController
  types.ts         — Shared TypeScript types (StatEntry, SortKey, SortDir)
  components/      — (empty, reserved for future extraction)
```

## TypeScript Configuration (`tsconfig.json`)

- `strict: true` — all strict checks enabled
- `noUnusedLocals: true` — no unused variables
- `noUnusedParameters: true` — no unused function parameters
- `noFallthroughCasesInSwitch: true`
- Target: ES2020, module: ESNext, JSX: react-jsx

## Code Style

### Formatting

- **No semicolons** — the entire codebase omits them
- **Single quotes** for all string literals
- No ESLint or Prettier configured — follow the existing convention manually

### Imports

Ordered in four groups:
1. React / framework imports (`react`, `react-dom/client`)
2. Local imports — hooks, components (`./useStats`)
3. Type-only imports (`import type { SortKey, SortDir, StatEntry } from './types'`)
4. CSS imports (`./App.css`)

```typescript
import { useState, useMemo } from 'react'
import { useStats } from './useStats'
import type { SortKey, SortDir, StatEntry } from './types'
import './App.css'
```

Always use `import type` for type-only imports to keep them separate from runtime imports.

### Naming Conventions

| Kind | Convention | Examples |
|---|---|---|
| Components | `PascalCase` functions | `App`, `BarChart`, `StatsTable`, `StatusBadge` |
| Hooks | `camelCase` with `use` prefix | `useStats` |
| Variables / functions | `camelCase` | `sortKey`, `handleSort`, `fetchStats` |
| Interfaces | `PascalCase` | `BarChartProps`, `UseStatsOptions`, `UseStatsResult` |
| Type aliases | `PascalCase` | `SortKey`, `SortDir`, `ConnectionStatus` |
| CSS classes | `kebab-case` | `bar-chart`, `proto-badge`, `status-badge` |
| Files | PascalCase for components (`App.tsx`), camelCase for hooks (`useStats.ts`), lowercase for modules (`types.ts`) |

### Types

- **`interface`** for component props and object shapes: `BarChartProps`, `UseStatsResult`
- **`type`** for unions and aliases: `type SortKey = 'port' | 'protocol' | 'count'`
- **`export type`** / **`import type`** to separate type-only from runtime exports
- **Never use `any`**. Cast with `as KnownType` when narrowing (e.g., `as Error`)
- Union with known values + catch-all: `protocol: 'tcp' | 'udp' | string`
- Non-null assertion (`!`) only at the app entry point: `document.getElementById('root')!`

### Exports

- **Default export** for the main component: `export default function App()`
- **Named exports** for hooks and types: `export function useStats`, `export type SortKey`
- **Helper components are file-private** (no `export`): `BarChart`, `StatsTable`,
  `SortIndicator`, `StatusBadge` are all defined in `App.tsx` without export

### Component Organization in `App.tsx`

Helper components appear **before** the main component that uses them:
1. `BarChart` (with `BarChartProps` interface above it)
2. `StatsTable` + `SortIndicator` (with `StatsTableProps` interface above)
3. `StatusBadge`
4. `App` (default export, appears last)

Each section is separated by the `// ----------- Section Name -----------` pattern.

### Async / Data Fetching

- `async`/`await` inside `useCallback`. No `.then()` chains.
- `AbortController` for request cancellation — previous request aborted before each new one.
- `useRef<AbortController | null>` to hold the current controller across renders.
- `setInterval` for polling; clean up in `useEffect` return with `clearInterval` + `abort()`.

### Error Handling

- `try`/`catch` with `as Error` for type narrowing
- Silently swallow `AbortError` (expected during cleanup)
- Surface real errors via UI state: `setStatus('error')` → `StatusBadge` shows "Disconnected"
- Check `res.ok` before parsing: `if (!res.ok) throw new Error(\`HTTP ${res.status}\`)`

### API Contract

The hook `useStats` fetches `GET /api/stats` and expects:
```json
[
  { "protocol": "tcp", "port": 443, "count": 1024 },
  { "protocol": "udp", "port": 53,  "count": 300 }
]
```

The `StatEntry` interface in `types.ts` must stay in sync with the Rust `StatEntry`
struct in `packet-counter/src/main.rs`. Fields: `protocol` (string), `port` (number),
`count` (number).
