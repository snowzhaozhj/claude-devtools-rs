import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test-setup.ts'],
      globals: false,
      include: ['src/**/*.{test,spec}.ts'],
      exclude: ['tests/e2e/**', 'node_modules/**'],
    },
  }),
)
