import { emptyFixture } from './empty'
import { multiProjectRichFixture } from './multi-project-rich'
import { singleProjectFixture } from './single-project'
import type { Fixture } from './types'

const REGISTRY: Record<string, Fixture> = {
  empty: emptyFixture,
  'single-project': singleProjectFixture,
  'multi-project-rich': multiProjectRichFixture,
}

/** 按名查找 fixture；找不到时 fallback 到 multi-project-rich 并 console.warn。 */
export function selectFixture(name: string | undefined | null): Fixture {
  const key = name ?? 'multi-project-rich'
  const fx = REGISTRY[key]
  if (!fx) {
    console.warn(
      `[mockIPC] unknown fixture "${key}", falling back to "multi-project-rich"`,
    )
    return multiProjectRichFixture
  }
  return fx
}

export { emptyFixture, multiProjectRichFixture, singleProjectFixture }
export type { Fixture }
