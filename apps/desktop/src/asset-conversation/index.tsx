// The person's conversation with the peer inside an asset: the stored messages when the asset
// opens, and the peer's answer shown as its parts arrive.

import type { AssetName } from 'protocol/AssetName';
import type { Event } from 'protocol/Event';
import type { Message } from 'protocol/Message';
import type { TurnId } from 'protocol/TurnId';
import { type FormEvent, type KeyboardEvent, useEffect, useRef, useState } from 'react';

import { describe, onEvent, request } from '../runtime/index.ts';
import { Button } from '../ui/button.tsx';
import { cn } from '../ui/cn.ts';
import { Textarea } from '../ui/textarea.tsx';

type PeerEvent = Exclude<Event, { event: 'sign_in_changed' }>;

export function AssetConversation({ asset }: { asset: AssetName }) {
  const [messages, setMessages] = useState<Message[]>([]);
  // The answer in progress, `null` while the peer is not answering.
  const [answer, setAnswer] = useState<string | null>(null);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string>();
  // Refs, not state, read by the event listener as they are now. The runtime sends a turn's reply
  // before its parts, but the relay can hand over the reply and the first parts at once, before the
  // code awaiting the reply runs. So while a message awaits its reply, this asset's events are kept,
  // and the reply's turn picks its own from them; the rest belong to an earlier turn still running,
  // such as one the person left mid-answer, and are dropped.
  const turn = useRef<TurnId | null>(null);
  const awaiting = useRef(false);
  const kept = useRef<PeerEvent[]>([]);
  const parts = useRef<string[]>([]);

  /** One event of the turn being answered. */
  function follow(event: PeerEvent) {
    if (event.event === 'peer_part') {
      parts.current.push(event.text);
      setAnswer(joined(parts.current));
      return;
    }
    if (event.event === 'peer_done') {
      const text = joined(parts.current);
      setMessages((earlier) => [...earlier, { author: 'peer', text }]);
    } else {
      setError(describe(event.failure));
    }
    turn.current = null;
    setAnswer(null);
  }

  useEffect(() => {
    let current = true;
    setMessages([]);
    setAnswer(null);
    setError(undefined);
    turn.current = null;
    awaiting.current = false;
    kept.current = [];
    request({ command: 'load_conversation', asset }).then(
      // The stored conversation goes before any message the person sent while it loaded.
      (reply) =>
        current &&
        reply.reply === 'conversation' &&
        setMessages((sent) => [...reply.conversation.messages, ...sent]),
      (failure) => current && setError(describe(failure)),
    );
    const stop = onEvent((event) => {
      if (event.event === 'sign_in_changed' || event.asset !== asset) return;
      if (awaiting.current) kept.current.push(event);
      else if (event.turn === turn.current) follow(event);
    });
    return () => {
      current = false;
      stop();
    };
  }, [asset]);

  async function send(event?: FormEvent) {
    event?.preventDefault();
    const text = draft.trim();
    if (text === '' || answer !== null) return;
    setDraft('');
    setError(undefined);
    setMessages((earlier) => [...earlier, { author: 'person', text }]);
    parts.current = [];
    setAnswer('');
    turn.current = null;
    awaiting.current = true;
    try {
      const reply = await request({ command: 'send_message', asset, text });
      // Not when the person left for another asset meanwhile, which started over.
      if (!awaiting.current || reply.reply !== 'message_accepted') return;
      awaiting.current = false;
      turn.current = reply.turn;
      const early = kept.current.filter((event) => event.turn === reply.turn);
      kept.current = [];
      for (const event of early) follow(event);
    } catch (failure) {
      awaiting.current = false;
      kept.current = [];
      setError(describe(failure));
      setAnswer(null);
    }
  }

  function sendOnEnter(event: KeyboardEvent) {
    if (event.key !== 'Enter' || event.shiftKey) return;
    event.preventDefault();
    void send();
  }

  return (
    <section className="flex h-full flex-col">
      <header className="border-b px-6 py-3">
        <h1 className="font-semibold">{asset}</h1>
      </header>
      <ol className="flex flex-1 flex-col gap-3 overflow-y-auto px-6 py-4">
        {messages.map((message, index) => (
          <Bubble key={index} author={message.author} text={message.text} />
        ))}
        {answer !== null && <Bubble author="peer" text={answer || '…'} writing />}
      </ol>
      {error && <p className="px-6 pb-2 text-sm text-destructive">{error}</p>}
      <form onSubmit={(event) => void send(event)} className="flex gap-2 border-t px-6 py-3">
        <Textarea
          placeholder={`Pregúntale a tu peer sobre ${asset}`}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={sendOnEnter}
          className="max-h-40"
        />
        <Button type="submit" disabled={draft.trim() === '' || answer !== null}>
          Enviar
        </Button>
      </form>
    </section>
  );
}

/** A turn's parts as the runtime stores them in its message. */
function joined(parts: string[]): string {
  return parts.join('\n\n');
}

/** A message; `writing` while the peer's answer still arrives. */
function Bubble({ author, text, writing = false }: Message & { writing?: boolean }) {
  return (
    <li
      aria-busy={writing}
      className={cn(
        'max-w-[75%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap',
        author === 'person' ? 'self-end bg-primary text-primary-foreground' : 'self-start bg-muted',
      )}
    >
      {text}
    </li>
  );
}
