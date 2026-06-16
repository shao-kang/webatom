
import { defineConfig } from 'rolldown';

export default defineConfig([
  {
    input: './dom/index.ts',
    external: [
      /^webatom_ext_native:/
    ],
    output: {
      file: './/dom/dist/index.js',
      format: 'es',   //
      name: '__dom',
    },
    platform: 'neutral', // 不注入 browser / node 全局变量
  }
]);

