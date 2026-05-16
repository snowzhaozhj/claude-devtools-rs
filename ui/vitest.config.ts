import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test-setup.ts'],
      globals: false,
      include: ['src/**/*.{test,spec}.ts', 'src/**/*.{test,spec}.svelte.ts'],
      exclude: ['tests/e2e/**', 'node_modules/**'],
    },
    // Svelte 5 在 jsdom 下 mount 需要 client 端 export condition；
    // 默认 vitest 走 server condition 会拿到 mount throw `lifecycle_function_unavailable`。
    resolve: {
      conditions: ['browser'],
    },
  }),
)
