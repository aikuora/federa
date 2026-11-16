// Whether Claude is connected, and starting its official sign-in, which the runtime opens in the
// browser; the status then arrives as it changes.

import type { SignInStatus } from 'protocol/SignInStatus';
import { useEffect, useReducer, useState } from 'react';

import { describe, onEvent, request } from '../runtime/index.ts';
import { Button } from '../ui/button.tsx';

type Shown = { status?: SignInStatus; incomplete: boolean };

// A sign-in that goes back to signed out did not complete, and the person is told so.
function next(shown: Shown, status: SignInStatus): Shown {
  return {
    status,
    incomplete: shown.status?.state === 'signing_in' && status.state === 'signed_out',
  };
}

export function ClaudeConnection() {
  const [{ status, incomplete }, show] = useReducer(next, { incomplete: false });
  const [error, setError] = useState<string>();

  useEffect(() => {
    request({ command: 'sign_in_status' }).then(
      (reply) => reply.reply === 'sign_in' && show(reply.status),
      (failure) => setError(describe(failure)),
    );
    return onEvent((event) => event.event === 'sign_in_changed' && show(event.status));
  }, []);

  async function signIn() {
    setError(undefined);
    try {
      const reply = await request({ command: 'start_sign_in' });
      if (reply.reply === 'sign_in') show(reply.status);
    } catch (failure) {
      setError(describe(failure));
    }
  }

  return (
    <section className="flex flex-col gap-2 px-2 text-sm">
      {status?.state === 'signed_in' && <p>Claude conectado</p>}
      {status?.state === 'signing_in' && (
        <p className="text-muted-foreground">Esperando que termines de entrar en el navegador…</p>
      )}
      {status?.state === 'unavailable' && <p className="text-destructive">{status.reason}</p>}
      {status?.state === 'signed_out' && (
        <>
          {incomplete && <p className="text-muted-foreground">No se completó la conexión.</p>}
          <Button size="sm" onClick={() => void signIn()}>
            {incomplete ? 'Intentar de nuevo' : 'Conectar con Claude'}
          </Button>
        </>
      )}
      {error && <p className="text-destructive">{error}</p>}
    </section>
  );
}
