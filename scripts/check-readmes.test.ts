import { describe, expect, test } from 'vite-plus/test';

import { readmeProblems, units } from './check-readmes.ts';

describe('units', () => {
  test('finds each crate and app once', () => {
    const paths = ['crates/kernel/src/lib.rs', 'crates/kernel/Cargo.toml', 'apps/runtime/x.rs'];
    expect(units([...paths, 'AGENTS.md', 'crates/README.md'])).toEqual([
      'apps/runtime',
      'crates/kernel',
    ]);
  });
});

describe('readmeProblems', () => {
  test('accepts the fixed headings in order, skipping some', () => {
    const text = [
      '# kernel',
      '',
      'What it is.',
      '## Scope',
      '### Responsibilities',
      '### Design guidelines',
      '## Development',
      '```sh',
      '# a shell comment, not a heading',
      '```',
      '### Caveats',
    ].join('\n');
    expect(readmeProblems('crates/kernel', text)).toEqual([]);
  });

  test('accepts a tool with only its opening and how to build it', () => {
    expect(
      readmeProblems('apps/tool', '# tool\n\nIt.\n## Development\n### Build and run\n'),
    ).toEqual([]);
  });

  test('requires the unit name as title', () => {
    expect(readmeProblems('crates/kernel', '# Kernel\n')).toEqual([
      'its first line is not `# kernel`',
    ]);
  });

  test('rejects other headings', () => {
    expect(readmeProblems('crates/kernel', '# kernel\n## Usage\n')).toEqual([
      '`## Usage` is not one of the fixed headings',
    ]);
  });

  test('rejects headings out of order or repeated', () => {
    expect(readmeProblems('crates/kernel', '# kernel\n## Development\n## Scope\n')).toEqual([
      '`## Scope` is out of order',
    ]);
    expect(readmeProblems('crates/kernel', '# kernel\n## Scope\n## Scope\n')).toEqual([
      '`## Scope` is out of order',
    ]);
  });

  test('rejects a subsection outside its section', () => {
    expect(readmeProblems('crates/kernel', '# kernel\n## Scope\n### Testing\n')).toEqual([
      '`### Testing` is not under its section',
    ]);
  });

  test('rejects a README over the cap', () => {
    const text = ['# kernel', ...Array.from({ length: 150 }, () => 'line')].join('\n');
    expect(readmeProblems('crates/kernel', text)).toEqual(['151 lines, over 150']);
  });
});
