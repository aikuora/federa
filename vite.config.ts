import { configDefaults, defineConfig } from 'vite-plus';

export default defineConfig({
  fmt: {
    singleQuote: true,
  },
  lint: {
    options: {
      typeAware: true,
      typeCheck: true,
    },
  },
  // Oxfmt and Oxlint skip what git ignores; Vitest does not, so it would run the tests of every
  // change worktree too.
  test: {
    exclude: [...configDefaults.exclude, '.claude/worktrees/**'],
  },
});
