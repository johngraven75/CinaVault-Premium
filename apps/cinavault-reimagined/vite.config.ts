import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  css: {
    // Keep this isolated app from inheriting the legacy repository-level
    // PostCSS/Tailwind configuration. ReImagined ships plain CSS and does not
    // require the legacy Tailwind dependency tree.
    postcss: {},
  },
  server: {
    port: 1421,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome105', 'safari13'],
    minify: 'esbuild',
    sourcemap: true,
  },
});
