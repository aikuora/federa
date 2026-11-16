import type { ClientFrame } from 'protocol/ClientFrame';
import type { RuntimeFrame } from 'protocol/RuntimeFrame';
import { VERSION } from 'protocol/version';
import { beforeEach, describe, expect, test, vi } from 'vite-plus/test';

// A fake of the native side: it keeps the frames the interface sends and lets each test answer as
// the runtime would, through the channel the interface handed it.
const relay = vi.hoisted(() => ({
  sent: [] as ClientFrame[],
  deliver: (_frame: RuntimeFrame | null) => {},
  refuse: undefined as string | undefined,
  // What the runtime answers to the interface's `Hello`.
  hello: undefined as number | undefined,
}));

vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    constructor(onmessage: (frame: RuntimeFrame | null) => void) {
      relay.deliver = onmessage;
    }
  },
  invoke: async (command: string, args: { frame?: ClientFrame }) => {
    if (relay.refuse !== undefined) throw relay.refuse;
    if (command === 'send' && args.frame) {
      relay.sent.push(args.frame);
      if (args.frame.frame === 'hello' && relay.hello !== undefined) {
        const protocol = relay.hello;
        queueMicrotask(() => relay.deliver({ frame: 'hello', protocol }));
      }
    }
  },
}));

type Runtime = typeof import('./index.ts');

async function connected(): Promise<Runtime> {
  const runtime = await import('./index.ts');
  await runtime.connect();
  return runtime;
}

function idOf(frame: ClientFrame | undefined): number {
  if (frame?.frame !== 'command') throw new Error('not a command');
  return frame.id;
}

beforeEach(() => {
  vi.resetModules();
  relay.sent = [];
  relay.refuse = undefined;
  relay.hello = VERSION;
});

describe('connect', () => {
  test('greets the runtime with this version', async () => {
    await connected();
    expect(relay.sent).toEqual([{ frame: 'hello', protocol: VERSION }]);
  });

  test('rejects a runtime that speaks another version', async () => {
    relay.hello = VERSION + 1;
    const runtime = await import('./index.ts');
    await expect(runtime.connect()).rejects.toThrow(`versión ${VERSION + 1}`);
  });

  test('rejects when the connection ends before the greeting', async () => {
    relay.hello = undefined;
    const runtime = await import('./index.ts');
    const connecting = runtime.connect();
    await vi.waitFor(() => expect(relay.sent).toHaveLength(1));
    relay.deliver(null);
    await expect(connecting).rejects.toThrow('Se cerró la conexión');
  });

  test('rejects with what the native side says', async () => {
    relay.refuse = 'could not start the runtime';
    const runtime = await import('./index.ts');
    await expect(runtime.connect()).rejects.toThrow('could not start the runtime');
  });
});

describe('request', () => {
  test('resolves each command with the reply of its id, in any order', async () => {
    const runtime = await connected();
    const assets = runtime.request({ command: 'list_assets' });
    const turn = runtime.request({ command: 'send_message', asset: 'billing', text: 'Hola' });
    const [, first, second] = relay.sent;

    relay.deliver({
      frame: 'reply',
      id: idOf(second),
      outcome: { outcome: 'done', reply: { reply: 'message_accepted', turn: 3 } },
    });
    relay.deliver({
      frame: 'reply',
      id: idOf(first),
      outcome: { outcome: 'done', reply: { reply: 'assets', assets: [] } },
    });

    await expect(assets).resolves.toEqual({ reply: 'assets', assets: [] });
    await expect(turn).resolves.toEqual({ reply: 'message_accepted', turn: 3 });
    expect(idOf(first)).not.toBe(idOf(second));
  });

  test('rejects with the failure the runtime answers', async () => {
    const runtime = await connected();
    const assets = runtime.request({ command: 'list_assets' });
    const failure = { code: 'no_organization', message: 'Open an organization first.' } as const;

    relay.deliver({
      frame: 'reply',
      id: idOf(relay.sent[1]),
      outcome: { outcome: 'failed', failure },
    });

    await expect(assets).rejects.toEqual(failure);
  });

  test('rejects every waiting command when the connection ends', async () => {
    const runtime = await connected();
    const assets = runtime.request({ command: 'list_assets' });
    const conversation = runtime.request({ command: 'load_conversation', asset: 'billing' });

    relay.deliver(null);

    await expect(assets).rejects.toThrow('Se cerró la conexión');
    await expect(conversation).rejects.toThrow('Se cerró la conexión');
  });

  test('rejects at once once the connection has ended', async () => {
    const runtime = await connected();
    relay.deliver(null);

    await expect(runtime.request({ command: 'list_assets' })).rejects.toThrow(
      'Se cerró la conexión',
    );
    expect(relay.sent).toHaveLength(1);
  });

  test('rejects when the native side cannot send it', async () => {
    const runtime = await connected();
    relay.refuse = 'the connection to the runtime is closed';
    await expect(runtime.request({ command: 'list_assets' })).rejects.toThrow(relay.refuse);
  });
});

describe('onEvent', () => {
  test('hands each event to every listener until it stops', async () => {
    const runtime = await connected();
    const first = vi.fn();
    const second = vi.fn();
    const stop = runtime.onEvent(first);
    runtime.onEvent(second);
    const event = { event: 'peer_done', asset: 'billing', turn: 3 } as const;

    relay.deliver({ frame: 'event', event });
    stop();
    relay.deliver({ frame: 'event', event });

    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(2);
    expect(second).toHaveBeenCalledWith(event);
  });
});
