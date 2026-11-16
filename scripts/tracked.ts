import { execFileSync } from 'node:child_process';

// Walking git's index instead of the file system leaves out whatever git ignores, the change
// worktrees under `.claude/worktrees/` included.
export function trackedFiles(): string[] {
  return execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' }).split('\0').filter(Boolean);
}
