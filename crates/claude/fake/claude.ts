// A fake `claude` that every test of the project runs instead of the real binary, so none needs
// the provider's binary or the person's subscription. Its version and every line it prints come
// from the real recordings in `../recordings`, so recording again when the minimum version rises
// updates it too. It never reads or writes anything of the person's own Claude.
import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { isAbsolute, join } from 'node:path';
import { createInterface } from 'node:readline';

type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

const RECORDINGS = join(import.meta.dirname, '..', 'recordings');
// What the recordings hold that the fake swaps for its own: the answer of `open`, and the server,
// the tool and its text of `mcp-tool`.
const RECORDED = {
  answer: 'ready',
  server: 'recorder',
  tool: 'secret_word',
  text: 'lighthouse',
};
// Owned by the fake, so it never stands for a sign-in of the real binary.
const SIGNED_IN = 'fake-signed-in';

type Line = { [key: string]: Json };

function recording(name: string): Line[] {
  const text = readFileSync(join(RECORDINGS, `${name}.jsonl`), 'utf8');
  return text
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line) as Line);
}

// A string field the fake needs from the first line of a recording.
function field(name: string, key: string): string {
  const found = recording(name)[0][key];
  if (typeof found !== 'string') throw new Error(`${name}.jsonl without ${key}`);
  return found;
}

// The lines of one recorded turn, with every string value found in `swaps` replaced, the recorded
// session id among them.
function turn(name: string, session: string, swaps: [string, string][]): string {
  const replacements = new Map([[field(name, 'session_id'), session], ...swaps]);
  const replaced = (value: Json): Json => {
    if (typeof value === 'string') return replacements.get(value) ?? value;
    if (Array.isArray(value)) return value.map(replaced);
    if (value !== null && typeof value === 'object') {
      return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, replaced(item)]));
    }
    return value;
  };
  return recording(name)
    .map((line) => `${JSON.stringify(replaced(line))}\n`)
    .join('');
}

function configFolder(): string {
  const folder = process.env.CLAUDE_CONFIG_DIR;
  // The launcher runs from the fake's own folder, where a relative one would land in the repository.
  if (!folder || !isAbsolute(folder)) {
    throw new Error('the fake claude runs only on an absolute CLAUDE_CONFIG_DIR of its own');
  }
  return folder;
}

function signedIn(): boolean {
  return existsSync(join(configFolder(), SIGNED_IN));
}

function value(args: string[], flag: string): string | undefined {
  return args.find((arg) => arg.startsWith(`${flag}=`))?.slice(flag.length + 1);
}

type Server = { name: string; url: string; headers: Record<string, string> };
type Tool = { server: Server; name: string };

// Each header with its `${VAR}` expanded from the environment, as the real binary does.
function servers(args: string[]): Server[] {
  const config = JSON.parse(value(args, '--mcp-config') ?? '{"mcpServers":{}}') as {
    mcpServers: Record<string, { url: string; headers?: Record<string, string> }>;
  };
  return Object.entries(config.mcpServers).map(([name, server]) => {
    const expand = (text: string) =>
      text.replace(/\$\{(\w+)\}/g, (_, variable: string) => process.env[variable] ?? '');
    const headers = Object.entries(server.headers ?? {}).map(([key, text]) => [key, expand(text)]);
    return { name, url: server.url, headers: Object.fromEntries(headers) };
  });
}

type Reply = { result?: Json; error?: { message: string } };

// One JSON-RPC request with one JSON body back, which is what the runtime's MCP server answers.
async function rpc(server: Server, method: string, params: Json, id?: number): Promise<Json> {
  const response = await fetch(server.url, {
    method: 'POST',
    headers: {
      ...server.headers,
      'content-type': 'application/json',
      accept: 'application/json, text/event-stream',
    },
    body: JSON.stringify({ jsonrpc: '2.0', method, params, ...(id === undefined ? {} : { id }) }),
  });
  if (!response.ok) throw new Error(`${server.name} answered ${method} with ${response.status}`);
  if (id === undefined) return null;
  const reply = (await response.json()) as Reply;
  if (reply.error) throw new Error(`${server.name} failed ${method}: ${reply.error.message}`);
  return reply.result ?? null;
}

async function tools(server: Server): Promise<Tool[]> {
  const initialize = {
    protocolVersion: '2025-11-25',
    capabilities: {},
    clientInfo: { name: 'claude-code', version: version() },
  };
  await rpc(server, 'initialize', initialize, 1);
  await rpc(server, 'notifications/initialized', {});
  const listed = (await rpc(server, 'tools/list', {}, 2)) as { tools: { name: string }[] };
  return listed.tools.map((tool) => ({ server, name: tool.name }));
}

async function call(tool: Tool): Promise<string> {
  const result = (await rpc(tool.server, 'tools/call', { name: tool.name, arguments: {} }, 3)) as {
    content: { type: string; text?: string }[];
  };
  return result.content.map((block) => block.text ?? '').join('');
}

async function converse(args: string[]): Promise<void> {
  const session = randomUUID();
  const instructions = value(args, '--append-system-prompt') ?? '';
  let offered: Promise<Tool[]> | undefined;
  for await (const line of createInterface({ input: process.stdin })) {
    const message = (JSON.parse(line) as { message: { content: string } }).message.content;
    if (!signedIn()) {
      process.stdout.write(turn('not-signed-in', session, []));
      continue;
    }
    // Listed once, on the first turn that can call them, as the real binary connects once.
    offered ??= Promise.all(servers(args).map(tools)).then((lists) => lists.flat());
    const named = (await offered).find(
      (tool) => message.includes(tool.name) || instructions.includes(tool.name),
    );
    if (!named) {
      process.stdout.write(turn('open', session, [[RECORDED.answer, message]]));
      continue;
    }
    const text = await call(named);
    const swaps: [string, string][] = [
      [`mcp__${RECORDED.server}__${RECORDED.tool}`, `mcp__${named.server.name}__${named.name}`],
      [RECORDED.server, named.server.name],
      [RECORDED.tool, named.name],
      [RECORDED.text, text],
    ];
    process.stdout.write(turn('mcp-tool', session, swaps));
  }
}

function version(): string {
  return field('open', 'claude_code_version');
}

async function main(args: string[]): Promise<number> {
  const [first, second] = args;
  if (first === '--version') {
    process.stdout.write(`${version()} (Claude Code)\n`);
    return 0;
  }
  if (first === 'auth' && second === 'login') {
    mkdirSync(configFolder(), { recursive: true });
    writeFileSync(join(configFolder(), SIGNED_IN), '');
    return 0;
  }
  if (first === 'auth' && second === 'status') return signedIn() ? 0 : 1;
  if (args.includes('-p')) {
    await converse(args);
    return 0;
  }
  throw new Error(`the fake claude does not do: ${args.join(' ')}`);
}

// Setting the exit code instead of exiting lets every line already written to stdout reach the
// pipe. A failure, such as an MCP server that refuses the token, ends the binary with its reason
// on stderr, which the adapter reports.
try {
  process.exitCode = await main(process.argv.slice(2));
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
  process.stdin.destroy();
}
