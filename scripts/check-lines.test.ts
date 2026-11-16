import { describe, expect, test } from 'vite-plus/test';

import { codeLines, isSource } from './check-lines.ts';

describe('isSource', () => {
  test('takes Rust and TypeScript code', () => {
    expect(isSource('crates/kernel/src/lib.rs')).toBe(true);
    expect(isSource('apps/desktop/src/main.tsx')).toBe(true);
    expect(isSource('vite.config.ts')).toBe(true);
  });

  test('leaves out tests and other files', () => {
    expect(isSource('apps/desktop/src/main.test.tsx')).toBe(false);
    expect(isSource('crates/kernel/tests/flow.rs')).toBe(false);
    expect(isSource('AGENTS.md')).toBe(false);
  });
});

describe('codeLines', () => {
  test('counts every line of a file without tests', () => {
    expect(codeLines('src/a.ts', 'a\n\nb\n')).toBe(3);
  });

  test('stops a Rust file at its test module', () => {
    const text =
      'fn a() {}\nfn b() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n';
    expect(codeLines('src/a.rs', text)).toBe(2);
  });

  test('counts the code after an out-of-line test module', () => {
    const text = 'mod store;\n#[cfg(test)]\nmod tests;\nfn a() {}\nfn b() {}\n';
    expect(codeLines('src/a.rs', text)).toBe(5);
  });

  test('counts the code after a test item that is not a module', () => {
    const text = '#[cfg(test)]\nuse std::fmt;\nfn a() {}\nfn b() {}\n';
    expect(codeLines('src/a.rs', text)).toBe(4);
  });

  test('stops at a public test module with a blank line before it', () => {
    const text = 'fn a() {}\n#[cfg(test)]\n\npub(crate) mod tests {\n}\n';
    expect(codeLines('src/a.rs', text)).toBe(1);
  });

  test('stops at a test module that carries more attributes', () => {
    const text = 'fn a() {}\n#[cfg(test)]\n#[allow(clippy::unwrap_used)]\nmod tests {\n}\n';
    expect(codeLines('src/a.rs', text)).toBe(1);
  });

  test('counts a Rust file whole when it has no test module', () => {
    expect(codeLines('src/a.rs', 'fn a() {}\nfn b() {}\n')).toBe(2);
  });
});
