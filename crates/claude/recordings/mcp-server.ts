import { randomUUID } from 'node:crypto';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import type { AddressInfo } from 'node:net';

// A tiny MCP server over HTTP with one tool. It answers only requests carrying `token`, so a
// recording that calls the tool proves the token reached it through the environment.
export type Mcp = { url: string; token: string; refused: number; close: () => void };

export async function mcpServer(): Promise<Mcp> {
  const token = randomUUID();
  const mcp: Mcp = { url: '', token, refused: 0, close: () => server.close() };
  const server = createServer((request, response) => {
    if (request.headers.authorization !== `Bearer ${token}`) {
      mcp.refused += 1;
      return void response.writeHead(401).end();
    }
    if (request.method !== 'POST') return void response.writeHead(405).end();
    void body(request).then((text) => answer(JSON.parse(text) as Rpc, response));
  });
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
  mcp.url = `http://127.0.0.1:${(server.address() as AddressInfo).port}/mcp`;
  return mcp;
}

type Rpc = { id?: number | string; method: string; params?: { protocolVersion?: string } };

function answer(rpc: Rpc, response: ServerResponse): void {
  if (rpc.id === undefined) return void response.writeHead(202).end();
  const results: Record<string, unknown> = {
    initialize: {
      protocolVersion: rpc.params?.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'recorder', version: '1.0.0' },
    },
    'tools/list': {
      tools: [
        {
          name: 'secret_word',
          description: 'Returns the secret word.',
          inputSchema: { type: 'object', properties: {} },
        },
      ],
    },
    'tools/call': { content: [{ type: 'text', text: 'lighthouse' }] },
  };
  const reply =
    rpc.method in results
      ? { jsonrpc: '2.0', id: rpc.id, result: results[rpc.method] }
      : { jsonrpc: '2.0', id: rpc.id, error: { code: -32601, message: 'method not found' } };
  response.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(reply));
}

async function body(request: IncomingMessage): Promise<string> {
  let text = '';
  for await (const chunk of request) text += String(chunk);
  return text;
}
