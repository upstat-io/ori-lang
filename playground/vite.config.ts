import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';

export default defineConfig({
    plugins: [svelte()],
    resolve: {
        alias: [
            // Resolve imports from the symlinked components
            {
                find: /^\.\.\/\.\.\/wasm\/(.*)/,
                replacement: path.resolve(__dirname, '../website/src/wasm/$1'),
            },
        ],
        preserveSymlinks: false,
    },
    server: {
        port: 3000,
        open: true,
        fs: {
            // Allow serving files from website directory
            allow: ['..'],
        },
    },
});
