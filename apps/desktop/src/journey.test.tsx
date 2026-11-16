// @vitest-environment jsdom

// The goal's journey through the real interface, against a real runtime that runs the fake
// `claude`. Only the native side is replaced: a bridge in Node reads `runtime.toml` and holds the
// WebSocket, as the native side does, so no window opens. Everything lives in a temporary folder
// that is removed at the end, and the runtime is stopped even when a step fails.

import { type ChildProcess, execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join, resolve } from 'node:path';

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterAll, beforeAll, describe, expect, test, vi } from 'vite-plus/test';

import { App } from './App.tsx';
import { reopen } from './organization-setup/index.tsx';
import { connect } from './runtime/index.ts';

const native = vi.hoisted(() => ({
  /** The runtime's data folder, where the bridge reads `runtime.toml`. */
  data: '',
  /** The folders the person picks, in order. */
  picks: [] as string[],
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: async () => native.picks.shift() ?? null,
}));

vi.mock('@tauri-apps/api/core', async () => {
  // The DOM environment replaces Node's WebSocket and its events with ones that take no headers.
  const { WebSocket } = await import('ws');
  let socket: InstanceType<typeof WebSocket> | undefined;
  return {
    Channel: class {
      onmessage: (frame: unknown) => void;
      constructor(onmessage: (frame: unknown) => void) {
        this.onmessage = onmessage;
      }
    },
    invoke: async (
      command: string,
      args: { frames?: { onmessage: (frame: unknown) => void }; frame?: unknown },
    ) => {
      if (command === 'send') return socket!.send(JSON.stringify(args.frame));
      const published = readFileSync(join(native.data, 'runtime.toml'), 'utf8');
      const port = /^port = (\d+)$/m.exec(published)![1];
      const token = /^token = "(.+)"$/m.exec(published)![1];
      const opening = new WebSocket(`ws://127.0.0.1:${port}/`, {
        headers: { authorization: `Bearer ${token}` },
      });
      await new Promise((resolve, reject) => opening.once('open', resolve).once('error', reject));
      const frames = args.frames!;
      // The runtime sends only text frames, which `ws` hands over as one buffer.
      opening.on('message', (data) => frames.onmessage(JSON.parse((data as Buffer).toString())));
      opening.on('close', () => frames.onmessage(null));
      socket = opening;
    },
  };
});

const KNOWLEDGE = /Billing charges customers each month/;
const root = resolve(import.meta.dirname, '../../..');
const scratch = mkdtempSync(join(tmpdir(), 'journey-'));
// The git hooks run this suite with variables that point git at the project's own repository.
const git = Object.fromEntries(
  Object.entries(process.env).filter(([name]) => !name.startsWith('GIT_')),
);
let runtime: ChildProcess | undefined;

/** Starts the runtime on the scratch data folder and waits until it publishes where it listens. */
async function startRuntime() {
  // Removed first, so a restart waits for the new runtime's file, not the one the last left.
  rmSync(join(native.data, 'runtime.toml'), { force: true });
  runtime = spawn(join(root, 'target/debug/runtime'), {
    env: {
      ...git,
      FEDERA_DATA_DIR: native.data,
      GIT_CONFIG_GLOBAL: join(scratch, 'gitconfig'),
      GIT_CONFIG_NOSYSTEM: '1',
      PATH: [join(root, 'crates/claude/fake'), process.env.PATH].join(delimiter),
    },
    stdio: ['ignore', 'ignore', 'inherit'],
  });
  await vi.waitFor(() => expect(existsSync(join(native.data, 'runtime.toml'))).toBe(true), {
    timeout: 10_000,
  });
}

async function stopRuntime() {
  const stopped = new Promise((resolve) => runtime!.once('exit', resolve));
  runtime!.kill();
  await stopped;
}

/** A git repository holding an asset's `.federa/`, with what the asset knows about itself. */
function assetFolder(): string {
  const folder = join(scratch, 'billing');
  mkdirSync(join(folder, '.federa'), { recursive: true });
  writeFileSync(join(folder, '.federa/asset.toml'), 'schema = 1\nname = "billing"\n');
  writeFileSync(join(folder, '.federa/knowledge.md'), 'Billing charges customers each month.\n');
  execFileSync('git', ['init', '--quiet'], { cwd: folder, env: git });
  return folder;
}

describe.skipIf(process.platform === 'win32')('the journey', () => {
  beforeAll(async () => {
    execFileSync('cargo', ['build', '--quiet', '-p', 'runtime'], { cwd: root, stdio: 'inherit' });
    native.data = join(scratch, 'data');
    writeFileSync(join(scratch, 'gitconfig'), '[user]\n\temail = ada@example.com\n');
    await startRuntime();
  }, 600_000);

  afterAll(() => {
    cleanup();
    runtime?.kill();
    rmSync(scratch, { recursive: true, force: true });
  });

  test('creates an organization, registers an asset, connects Claude and talks with the peer', async () => {
    await connect();
    render(<App reopened={null} />);

    fireEvent.change(screen.getByPlaceholderText('Nombre de la organización'), {
      target: { value: 'Acme' },
    });
    native.picks.push(join(scratch, 'acme'));
    mkdirSync(join(scratch, 'acme'));
    fireEvent.click(screen.getByRole('button', { name: 'Elegir carpeta y crear' }));
    expect(await screen.findByText('Acme')).toBeDefined();

    native.picks.push(assetFolder());
    fireEvent.click(screen.getByRole('button', { name: 'Registrar activo' }));
    expect(await screen.findByRole('heading', { name: 'billing' })).toBeDefined();

    fireEvent.click(await screen.findByRole('button', { name: 'Conectar con Claude' }));
    // The runtime looks at the sign-in again every few seconds.
    expect(await screen.findByText('Claude conectado', {}, { timeout: 10_000 })).toBeDefined();

    // The peer's instructions name the tool that reads the asset's knowledge, so the fake `claude`
    // calls it and answers with its text.
    fireEvent.change(screen.getByPlaceholderText('Pregúntale a tu peer sobre billing'), {
      target: { value: '¿Qué hace este activo?' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Enviar' }));
    // Done once the answer is no longer being written, so the runtime has stored it.
    await vi.waitFor(
      () =>
        expect(screen.getByText(KNOWLEDGE).closest('li')?.getAttribute('aria-busy')).toBe('false'),
      { timeout: 10_000 },
    );

    cleanup();
    await stopRuntime();
    await startRuntime();
    await connect();
    render(<App reopened={await reopen()} />);
    fireEvent.click(await screen.findByRole('button', { name: /billing/ }));
    expect(await screen.findByText('¿Qué hace este activo?')).toBeDefined();
    expect(await screen.findByText(KNOWLEDGE)).toBeDefined();
  }, 60_000);
});
