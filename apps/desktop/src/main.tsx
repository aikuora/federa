import './index.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App.tsx';
import { reopen } from './organization-setup/index.tsx';
import { connect, describe } from './runtime/index.ts';

// Connected once, before the app renders, so every feature can count on the runtime being there.
const root = createRoot(document.getElementById('root')!);
root.render(<p className="p-6 text-sm text-muted-foreground">Conectando con el runtime…</p>);
connect()
  .then(reopen)
  .then(
    (organization) =>
      root.render(
        <StrictMode>
          <App reopened={organization} />
        </StrictMode>,
      ),
    (error) =>
      root.render(
        <p className="p-6 text-sm text-destructive">
          No se pudo conectar con el runtime: {describe(error)}
        </p>,
      ),
  );
