// The interface's only way to the runtime: the protocol over the native side's relay. Everything
// else in the app talks to the runtime through `request` and `onEvent`, never through the relay.

import { Channel, invoke } from '@tauri-apps/api/core';
import type { ClientFrame } from 'protocol/ClientFrame';
import type { Command } from 'protocol/Command';
import type { Event } from 'protocol/Event';
import type { Failure } from 'protocol/Failure';
import type { Reply } from 'protocol/Reply';
import type { RuntimeFrame } from 'protocol/RuntimeFrame';
import { VERSION } from 'protocol/version';

type Waiting<T> = { resolve: (value: T) => void; reject: (reason: Failure | Error) => void };

let greeting: Waiting<number> | undefined;
let closed = false;
const pending = new Map<number, Waiting<Reply>>();
const listeners = new Set<(event: Event) => void>();
let nextId = 0;

/** Connects through the native side and greets the runtime; rejects when it speaks another version. */
export async function connect(): Promise<void> {
  closed = false;
  const hello = new Promise<number>((resolve, reject) => {
    greeting = { resolve, reject };
  });
  await relay('connect', { frames: new Channel<RuntimeFrame | null>(receive) });
  await relay('send', { frame: { frame: 'hello', protocol: VERSION } satisfies ClientFrame });
  const protocol = await hello;
  if (protocol !== VERSION) {
    throw new Error(
      `El runtime habla la versión ${protocol} del protocolo y esta app, la ${VERSION}. Actualiza la que esté atrás.`,
    );
  }
}

/** Sends one command and resolves with its reply; rejects with the `Failure` when it fails. */
export function request(command: Command): Promise<Reply> {
  if (closed) return Promise.reject(closedError());
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    relay('send', { frame: { frame: 'command', id, command } satisfies ClientFrame }).catch(
      (error: Error) => {
        pending.delete(id);
        reject(error);
      },
    );
  });
}

/** Calls `listener` with every event the runtime sends; returns how to stop. */
export function onEvent(listener: (event: Event) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** What the person reads about why a call to the runtime failed. */
export function describe(error: unknown): string {
  return error instanceof Object && 'message' in error ? String(error.message) : String(error);
}

/** The native side rejects with the text of its error. */
function relay(command: 'connect' | 'send', args: Record<string, unknown>): Promise<void> {
  return invoke<void>(command, args).catch((message: unknown) => {
    throw new Error(String(message));
  });
}

function closedError(): Error {
  return new Error('Se cerró la conexión con el runtime.');
}

/** Each frame the runtime sends, and `null` once the connection ends. */
function receive(frame: RuntimeFrame | null): void {
  if (frame === null) {
    closed = true;
    greeting?.reject(closedError());
    greeting = undefined;
    for (const waiting of pending.values()) waiting.reject(closedError());
    pending.clear();
    return;
  }
  switch (frame.frame) {
    case 'hello':
      greeting?.resolve(frame.protocol);
      greeting = undefined;
      return;
    case 'reply': {
      const waiting = pending.get(frame.id);
      pending.delete(frame.id);
      if (frame.outcome.outcome === 'done') waiting?.resolve(frame.outcome.reply);
      else waiting?.reject(frame.outcome.failure);
      return;
    }
    case 'event':
      for (const listener of listeners) listener(frame.event);
  }
}
