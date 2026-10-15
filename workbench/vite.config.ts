import { fileURLToPath } from 'node:url';
import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
const novaProductionRuntime: Plugin = {
  name: 'novasheet-production-jsx-runtime',
  apply: 'build',
  enforce: 'pre',
  resolveId(source, importer) {
    if (source === 'react/jsx-dev-runtime' && importer?.includes('novasheet')) {
      return fileURLToPath(new URL('./src/shared/ui/nova-jsx-runtime.ts', import.meta.url));
    }
  },
};
export default defineConfig({ plugins: [novaProductionRuntime, react(), tailwindcss()], resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } }, server: { strictPort: true }, clearScreen: false });
