import { existsSync, readFileSync } from 'node:fs';

import { trackedFiles } from './tracked.ts';

const MAX_LINES = 150;

// The only headings a unit's README may have, in the order it must keep them.
const HEADINGS = [
  '## Scope',
  '### Responsibilities',
  '### Out of scope',
  '### Design guidelines',
  '## Public interface',
  '## Development',
  '### Build and run',
  '### Testing',
  '### Caveats',
];

export function units(paths: string[]): string[] {
  const roots = paths
    .map((path) => path.split('/'))
    .filter((parts) => parts.length > 2 && (parts[0] === 'apps' || parts[0] === 'crates'))
    .map((parts) => `${parts[0]}/${parts[1]}`);
  return [...new Set(roots)].sort();
}

export function readmeProblems(unit: string, text: string): string[] {
  const lines = text.trimEnd().split('\n');
  const problems: string[] = [];
  if (lines.length > MAX_LINES) problems.push(`${lines.length} lines, over ${MAX_LINES}`);
  const name = unit.split('/').at(-1);
  if (lines[0] !== `# ${name}`) problems.push(`its first line is not \`# ${name}\``);

  let fenced = false;
  let last = -1;
  let section = -1;
  for (const line of lines.slice(1)) {
    if (line.startsWith('```')) fenced = !fenced;
    if (fenced || !/^#{1,6} /.test(line)) continue;
    const index = HEADINGS.indexOf(line.trimEnd());
    if (index === -1) {
      problems.push(`\`${line}\` is not one of the fixed headings`);
      continue;
    }
    if (index <= last) problems.push(`\`${line}\` is out of order`);
    if (line.startsWith('## ')) section = index;
    else if (HEADINGS.slice(0, index).findLast((h) => h.startsWith('## ')) !== HEADINGS[section]) {
      problems.push(`\`${line}\` is not under its section`);
    }
    last = Math.max(last, index);
  }
  return problems;
}

if (import.meta.main) {
  let failed = false;
  for (const unit of units(trackedFiles())) {
    const path = `${unit}/README.md`;
    const problems = existsSync(path)
      ? readmeProblems(unit, readFileSync(path, 'utf8'))
      : ['the unit has no README.md'];
    for (const problem of problems) console.error(`${path}: ${problem}`);
    failed ||= problems.length > 0;
  }
  process.exitCode = failed ? 1 : 0;
}
