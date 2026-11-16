// Only how the interface is served and built; formatting, lint and tests come from the root config.

import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite-plus';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri expects the port its configuration names, and shows the native side's errors itself.
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
