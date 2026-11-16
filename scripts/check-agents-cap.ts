import { readFileSync } from 'node:fs';

const MAX_LINES = 500;

const lines = readFileSync('AGENTS.md', 'utf8').trimEnd().split('\n').length;
if (lines > MAX_LINES) {
  console.error(`AGENTS.md: ${lines} lines, over ${MAX_LINES}`);
  process.exitCode = 1;
}
