import { fileURLToPath, URL } from 'node:url';
import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vite';
import vueDevTools from 'vite-plugin-vue-devtools';
import wasm from 'vite-plugin-wasm';

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue(), vueDevTools(), wasm()],
  publicDir: 'playground/public',
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./playground', import.meta.url)),
    },
  },
  css: {
    preprocessorOptions: {
      scss: {
        quietDeps: true,
        silenceDeprecations: ['import'], // Bootstrap require this.
      },
    },
  },
  build: {
    outDir: 'web',
  },
});
