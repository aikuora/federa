// Records the stdout of the minimum version of `claude` for the offline tests of this crate and for
// the fake `claude` in `../fake`, which every test of the project runs. Only the person runs it, on
// their own subscription, once per rise of the minimum:
//
//   node crates/claude/recordings/record.ts
//
// It installs that version into `target/`, apart from any other install, and launches it with the
// arguments `src/launch.rs` builds, on a configuration folder of its own in the main checkout's
// `target/` that the person signs in once through the official flow. Each scenario runs in a
// workdir full of Claude files with a marker, and fails if the marker, or a plugin, skill or
// command from anywhere but the binary itself, shows up: agents must load none of them. It reports
// what the plugins built into the binary bring.
// It never reads, copies or prints credentials: sign-in stays in the binary, and every recording
// is scrubbed of the person's home, user name, emails and ids.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import { builtins, unexpected, workdirWithClaudeFiles } from './agent-files.ts';
import { type Mcp, mcpServer } from './mcp-server.ts';
import { scrubber } from './scrub.ts';

const VERSION = '2.1.311';
const HERE = import.meta.dirname;
const TARGET = join(HERE, '..', '..', '..', 'target');
const INSTALL = join(TARGET, `claude-${VERSION}`);
const BINARY = join(INSTALL, 'node_modules', '.bin', 'claude');
// In the main checkout, so every worktree signs in and records on the same folder, and its
// keychain entry outlives any of them. Also the folder the live test runs on.
const GIT = execFileSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], {
  cwd: HERE,
  encoding: 'utf8',
}).trim();
const CONFIG = join(dirname(GIT), 'target', 'claude-config');
const TURN_TIMEOUT_MS = 180_000;

type Scenario = {
  name: string;
  messages: string[];
  mcp?: boolean;
  signedOut?: boolean;
};

// The fake swaps the answer of `open` and the server, tool and text of `mcp-tool` for its own, in
// `RECORDED` of `../fake/claude.ts`; keep both alike.
const SCENARIOS: Scenario[] = [
  { name: 'open', messages: ['Reply with the single word: ready'] },
  {
    name: 'two-turns',
    messages: [
      'Remember the word orange. Reply only: ok',
      'Which word did I ask you to remember? Reply with that word only.',
    ],
  },
  {
    name: 'mcp-tool',
    mcp: true,
    messages: ['Call the secret_word tool and reply with the word it returns, nothing else.'],
  },
  { name: 'not-signed-in', signedOut: true, messages: ['Reply with the single word: ready'] },
];

function install(): void {
  if (!existsSync(BINARY)) {
    const pkg = `@anthropic-ai/claude-code@${VERSION}`;
    execFileSync('npm', ['install', '--prefix', INSTALL, '--no-save', pkg], { stdio: 'inherit' });
  }
  const printed = execFileSync(BINARY, ['--version'], { encoding: 'utf8' });
  if (!printed.startsWith(`${VERSION} `))
    throw new Error(`${BINARY} is not ${VERSION}: ${printed}`);
}

// The person's environment for signing in, without any key that would sign in instead of the
// subscription.
function environment(config: string): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = { ...process.env, DISABLE_AUTOUPDATER: '1' };
  const keys = [
    'ANTHROPIC_API_KEY',
    'ANTHROPIC_AUTH_TOKEN',
    'CLAUDE_CODE_OAUTH_TOKEN',
    'CLAUDECODE',
  ];
  for (const key of keys) delete env[key];
  // The keychain entry is keyed to this folder, so no other folder's sign-in is ever reached.
  env.CLAUDE_CONFIG_DIR = config;
  return env;
}

// What an agent runs with: the variables `INHERITED` in `src/launch.rs` lets through, and its
// folder; keep both alike. The updater stays off so the installed version holds.
function agentEnvironment(config: string): NodeJS.ProcessEnv {
  const inherited = ['HOME', 'PATH', 'USER', 'LOGNAME', 'TMPDIR', 'LANG', 'LC_ALL', 'LC_CTYPE'];
  const env: NodeJS.ProcessEnv = { DISABLE_AUTOUPDATER: '1', CLAUDE_CONFIG_DIR: config };
  for (const name of inherited) if (process.env[name] !== undefined) env[name] = process.env[name];
  return env;
}

// `auth status` exits 1 when the folder is not signed in; its output is never read.
function signedIn(): boolean {
  try {
    execFileSync(BINARY, ['auth', 'status'], { env: environment(CONFIG), stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

// The settings `src/config.rs` writes into the agents' configuration folder; keep both alike.
function prepare(config: string): void {
  mkdirSync(config, { recursive: true });
  const settings = {
    crossSessionInbound: 'refuse',
    syncClaudeAiPlugins: false,
    syncClaudeAiSkills: false,
    pluginConfigs: {
      'cc-plugin-agents-md@builtin': { options: { instructionFiles: 'claude-md' } },
    },
  };
  writeFileSync(join(config, 'settings.json'), `${JSON.stringify(settings, null, 2)}\n`);
}

function signIn(): void {
  prepare(CONFIG);
  if (signedIn()) return;
  console.log('Sign in to Claude for the recordings; the browser opens the official sign-in.');
  execFileSync(BINARY, ['auth', 'login'], { env: environment(CONFIG), stdio: 'inherit' });
  if (!signedIn()) throw new Error(`${CONFIG} is still not signed in`);
}

// The arguments of `launch::command` in `src/launch.rs`; keep both alike.
function args(mcp: Mcp | undefined): string[] {
  const settings = {
    autoMemoryEnabled: false,
    sandbox: { enabled: true, failIfUnavailable: true, allowUnsandboxedCommands: false },
  };
  const servers = mcp
    ? {
        recorder: {
          type: 'http',
          url: mcp.url,
          headers: { Authorization: '${MCP_AUTHORIZATION_0}' },
        },
      }
    : {};
  return [
    '-p',
    '--input-format',
    'stream-json',
    '--output-format',
    'stream-json',
    '--verbose',
    '--append-system-prompt=You are a test agent. Answer as briefly as you are asked to.',
    `--settings=${JSON.stringify(settings)}`,
    '--setting-sources=user',
    '--tools=Read,Glob,Grep',
    '--strict-mcp-config',
    `--mcp-config=${JSON.stringify({ mcpServers: servers })}`,
    ...(mcp ? ['--allowedTools=mcp__recorder__*'] : []),
  ];
}

async function record(scenario: Scenario, workdir: string, config: string): Promise<string[]> {
  const mcp = scenario.mcp ? await mcpServer() : undefined;
  const env = agentEnvironment(config);
  if (mcp) env.MCP_AUTHORIZATION_0 = `Bearer ${mcp.token}`;
  const child = spawn(BINARY, args(mcp), {
    cwd: workdir,
    env,
    stdio: ['pipe', 'pipe', 'inherit'],
  });
  const lines: string[] = [];
  let pending = '';
  let turnEnded = (): void => {};
  child.stdout.on('data', (chunk: Buffer) => {
    pending += chunk.toString('utf8');
    const complete = pending.split('\n');
    pending = complete.pop() ?? '';
    for (const line of complete.filter(Boolean)) {
      lines.push(line);
      if ((JSON.parse(line) as { type?: string }).type === 'result') turnEnded();
    }
  });
  // A binary that exits early ends the turn too; the recording then shows how it ended.
  const exited = new Promise((done) => child.on('exit', done)).then(() => turnEnded());
  try {
    for (const text of scenario.messages) {
      const ended = new Promise<void>((done, fail) => {
        turnEnded = done;
        const late = () => fail(new Error(`${scenario.name}: no result in time`));
        setTimeout(late, TURN_TIMEOUT_MS).unref();
      });
      child.stdin.write(
        `${JSON.stringify({ type: 'user', message: { role: 'user', content: text } })}\n`,
      );
      await ended;
    }
  } catch (error) {
    child.kill();
    throw error;
  } finally {
    child.stdin.end();
    await exited;
    mcp?.close();
  }
  if (mcp && mcp.refused > 0) {
    throw new Error(
      `${scenario.name}: the MCP server refused ${mcp.refused} requests without the token`,
    );
  }
  return lines;
}

install();
signIn();
const found: string[] = [];
for (const scenario of SCENARIOS) {
  const workdir = workdirWithClaudeFiles();
  // An empty folder is a configuration never signed in.
  const config = scenario.signedOut ? mkdtempSync(join(tmpdir(), 'claude-config-')) : CONFIG;
  prepare(config);
  try {
    const lines = await record(scenario, workdir, config);
    const scrubbed = lines.map(scrubber(workdir, config));
    writeFileSync(join(HERE, `${scenario.name}.jsonl`), `${scrubbed.join('\n')}\n`);
    const kinds = lines.map((line) => (JSON.parse(line) as { type?: string }).type);
    console.log(`${scenario.name}: ${lines.length} lines (${kinds.join(', ')})`);
    found.push(...unexpected(scrubbed).map((leak) => `${scenario.name}: ${leak.slice(0, 300)}`));
    for (const line of builtins(scrubbed)) console.log(`  ${line}`);
  } finally {
    rmSync(workdir, { recursive: true, force: true });
    if (config !== CONFIG) rmSync(config, { recursive: true, force: true });
  }
}
if (found.length > 0) {
  console.error(`The agent loaded what it must not:\n${found.join('\n')}`);
  process.exitCode = 1;
}
