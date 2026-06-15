
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/window.ts',
  external: [
    /^webatom_ext_native:/
  ],
  output: {
    file: './dist/index.js',
    format: 'es',   //
    name: '__dom',
  },
  platform: 'neutral', // 不注入 browser / node 全局变量
});

