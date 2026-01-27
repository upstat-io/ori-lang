import { defineConfig } from 'astro/config';
import svelte from '@astrojs/svelte';
import mdx from '@astrojs/mdx';

export default defineConfig({
  site: 'https://ori-lang.com',
  integrations: [svelte(), mdx()],
  vite: {
    optimizeDeps: {
      exclude: ['monaco-editor'],
    },
    build: {
      rollupOptions: {
        output: {
          manualChunks: {
            'monaco-editor': ['monaco-editor'],
          },
        },
      },
    },
  },
});
