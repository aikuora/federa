// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ClientFrame } from 'protocol/ClientFrame';
import type { RuntimeFrame } from 'protocol/RuntimeFrame';
import { VERSION } from 'protocol/version';
import { afterEach, expect, test, vi } from 'vite-plus/test';

import { connect } from '../runtime/index.ts';
import { AssetConversation } from './index.tsx';

// A fake of the native side that answers the greeting and an empty conversation on its own, and
// leaves every other frame to the test.
const relay = vi.hoisted(() => ({
  sent: [] as ClientFrame[],
  deliver: (_frame: RuntimeFrame | null) => {},
}));

vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    constructor(onmessage: (frame: RuntimeFrame | null) => void) {
      relay.deliver = onmessage;
    }
  },
  invoke: async (command: string, args: { frame?: ClientFrame }) => {
    const frame = args.frame;
    if (command !== 'send' || !frame) return;
    relay.sent.push(frame);
    if (frame.frame === 'hello') {
      queueMicrotask(() => relay.deliver({ frame: 'hello', protocol: VERSION }));
    } else if (frame.command.command === 'load_conversation') {
      const conversation = { asset: 'billing', messages: [] };
      const reply = { reply: 'conversation', conversation } as const;
      queueMicrotask(() =>
        relay.deliver({ frame: 'reply', id: frame.id, outcome: { outcome: 'done', reply } }),
      );
    }
  },
}));

afterEach(cleanup);

test('shows the answer when its turn arrives in the same task as the first part', async () => {
  await connect();
  render(<AssetConversation asset="billing" />);
  fireEvent.change(screen.getByPlaceholderText('Pregúntale a tu peer sobre billing'), {
    target: { value: '¿Qué hace?' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Enviar' }));
  const sent = await vi.waitFor(() => {
    const last = relay.sent.at(-1);
    if (last?.frame !== 'command' || last.command.command !== 'send_message') throw new Error();
    return last;
  });

  // One task, as when the relay flushes several frames at once: nothing awaiting the reply runs
  // before the parts arrive.
  const reply = { reply: 'message_accepted', turn: 7 } as const;
  relay.deliver({ frame: 'reply', id: sent.id, outcome: { outcome: 'done', reply } });
  for (const text of ['It charges', 'customers.']) {
    relay.deliver({
      frame: 'event',
      event: { event: 'peer_part', asset: 'billing', turn: 7, text },
    });
  }
  relay.deliver({ frame: 'event', event: { event: 'peer_done', asset: 'billing', turn: 7 } });

  const answer = await screen.findByText('It charges customers.');
  expect(answer.textContent).toBe('It charges\n\ncustomers.');
  expect(answer.getAttribute('aria-busy')).toBe('false');
});

test("does not take an earlier turn's parts for the answer of the message just sent", async () => {
  await connect();
  render(<AssetConversation asset="billing" />);
  fireEvent.change(screen.getByPlaceholderText('Pregúntale a tu peer sobre billing'), {
    target: { value: '¿Qué hace?' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Enviar' }));
  const sent = await vi.waitFor(() => {
    const last = relay.sent.at(-1);
    if (last?.frame !== 'command' || last.command.command !== 'send_message') throw new Error();
    return last;
  });

  // An earlier turn the person left mid-answer still streams before the new message's reply.
  const earlier = { event: 'peer_part', asset: 'billing', turn: 6, text: 'Old answer' } as const;
  relay.deliver({ frame: 'event', event: earlier });
  await Promise.resolve();
  const reply = { reply: 'message_accepted', turn: 7 } as const;
  relay.deliver({ frame: 'reply', id: sent.id, outcome: { outcome: 'done', reply } });
  relay.deliver({
    frame: 'event',
    event: { event: 'peer_part', asset: 'billing', turn: 7, text: 'It charges customers.' },
  });
  relay.deliver({ frame: 'event', event: { event: 'peer_done', asset: 'billing', turn: 7 } });

  const answer = await screen.findByText('It charges customers.');
  expect(answer.getAttribute('aria-busy')).toBe('false');
  expect(screen.queryByText('Old answer')).toBeNull();
});
