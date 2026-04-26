/// <reference types="node" />
// Production bundle 不应包含 mockIPC / fixture 代码（spec
// frontend-test-pyramid §"Production bundle 不含 mockIPC 代码" Scenario）。
//
// 默认 skip——build 慢且与单元测节奏不匹配。CI / 显式想跑时设
// RUN_BUNDLE_TESTS=1：`RUN_BUNDLE_TESTS=1 npm run test:unit --prefix ui`。

import { execSync } from 'node:child_process'
import { readdirSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

const RUN = process.env.RUN_BUNDLE_TESTS === '1'
const HERE = dirname(fileURLToPath(import.meta.url))

describe('production bundle', () => {
  test.skipIf(!RUN)(
    'dist 不含 mockIPC / __fixtures__ / 虚构 fixture 项目名',
    () => {
      const uiRoot = resolve(HERE, '../../')
      // 显式 NODE_ENV=production，避免 vitest 父进程把 NODE_ENV=test 传染给
      // 子进程的 vite build——后者会让 import.meta.env.DEV 不被替换为 false。
      execSync('npm run build', {
        cwd: uiRoot,
        stdio: 'pipe',
        env: { ...process.env, NODE_ENV: 'production' },
      })

      const assetsDir = join(uiRoot, 'dist', 'assets')
      const files = readdirSync(assetsDir).filter((f: string) => f.endsWith('.js'))
      expect(files.length).toBeGreaterThan(0)

      const forbiddenSubstrings = [
        'mockIPC',
        '__fixtures__',
        'mock-rich-rust',
        'mock-rich-ts',
        'mock-single-proj',
      ]

      for (const f of files) {
        const content = readFileSync(join(assetsDir, f), 'utf8')
        for (const needle of forbiddenSubstrings) {
          expect(
            content.includes(needle),
            `production bundle ${f} MUST 不含 "${needle}"`,
          ).toBe(false)
        }
      }
    },
    120_000,
  )
})
