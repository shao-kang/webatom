import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    lib: {
      entry: './dom/index.ts',
      name: '__dom',
      fileName: 'index',
      formats: ['es'],
    },
    outDir: './dom/dist',
    emptyOutDir: false,
    rollupOptions: {
      external: [/^webatom_ext_native:/],
    },
    target: 'esnext',
    minify: false,
  },
});
