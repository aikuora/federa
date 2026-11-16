import { readFileSync } from 'node:fs';

import { trackedFiles } from './tracked.ts';

const MAX_LINES = 300;

export function isSource(path: string): boolean {
  if (/\.test\.tsx?$/.test(path)) return false;
  if (path.endsWith('.rs')) return !path.split('/').includes('tests');
  return /\.tsx?$/.test(path);
}

// A Rust file keeps its unit tests in an inline `#[cfg(test)]` module at its end, so the count stops
// there. Any other `#[cfg(test)]` item, such as `mod tests;` or a `use`, still counts.
export function codeLines(path: string, text: string): number {
  const lines = text.split('\n');
  const tests = path.endsWith('.rs') ? lines.findIndex(opensTestModule) : -1;
  const code = (tests === -1 ? lines : lines.slice(0, tests)).join('\n').trimEnd();
  return code === '' ? 0 : code.split('\n').length;
}

function opensTestModule(line: string, index: number, lines: string[]): boolean {
  if (line.trim() !== '#[cfg(test)]') return false;
  const next = lines
    .slice(index + 1)
    .find((later) => later.trim() !== '' && !later.trim().startsWith('#['));
  return next !== undefined && /^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*\{/.test(next);
}

if (import.meta.main) {
  const tooLong = trackedFiles()
    .filter(isSource)
    .map((path) => ({ path, lines: codeLines(path, readFileSync(path, 'utf8')) }))
    .filter(({ lines }) => lines > MAX_LINES);
  for (const { path, lines } of tooLong) {
    console.error(`${path}: ${lines} lines without tests, over ${MAX_LINES}`);
  }
  process.exitCode = tooLong.length > 0 ? 1 : 0;
}
