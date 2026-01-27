import { defineConfig } from 'astro/config';
import svelte from '@astrojs/svelte';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import { readFileSync } from 'fs';

const oriGrammar = JSON.parse(
  readFileSync('./src/shiki/ori.tmLanguage.json', 'utf-8')
);

const ebnfGrammar = JSON.parse(
  readFileSync('./src/shiki/ebnf.tmLanguage.json', 'utf-8')
);

const oriLanguage = {
  id: 'ori',
  scopeName: 'source.ori',
  grammar: oriGrammar,
  aliases: ['ori'],
};

const ebnfLanguage = {
  id: 'ebnf',
  scopeName: 'source.ebnf',
  grammar: ebnfGrammar,
  aliases: ['ebnf', 'bnf'],
};

export default defineConfig({
  site: 'https://ori-lang.com',
  integrations: [svelte(), mdx(), sitemap()],
  markdown: {
    shikiConfig: {
      langs: [oriLanguage, ebnfLanguage],
    },
  },
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
