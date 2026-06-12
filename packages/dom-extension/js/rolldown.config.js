
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/index.ts',
  output: {
    file: './dist/index.js',
    format: 'iife',   // QuickJS 不支持 ES modules，打包为单文件立即执行
    name: '__dom',
  },
  platform: 'neutral', // 不注入 browser / node 全局变量
});

