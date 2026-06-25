/**
 * Context Panel 的 user-message injection 点击导航：解析应滚动到的目标 chunkId。
 *
 * - 完整 turn：injection 的 `aiGroupId` 是某 `AIChunk` 的 chunkId → 向前找紧邻的
 *   `UserChunk`（用户问题气泡）；无前置 UserChunk 则退化为 AIChunk 本身。
 * - 被打断 turn（issue #540）：响应被打断、不产 AIChunk，injection 的 `aiGroupId`
 *   本身就是该 `UserChunk` 的 chunkId → 直接返回它，**不**向前回溯（否则会跳到
 *   上一条用户消息）。
 *
 * spec：`openspec/specs/session-display/spec.md` Context Panel turn 锚点导航。
 */

/** duck-typed chunk：只取导航需要的两个字段，避免与 api.ts 循环依赖。 */
interface NavChunk {
  chunkId: string;
  kind: string;
}

/**
 * 返回点击某条 user-message injection 时应滚动到的 chunkId；命中不到对应 chunk
 * 时返回 `null`（调用方不导航）。
 */
export function resolveUserGroupNavTarget(
  chunks: ReadonlyArray<NavChunk>,
  aiGroupId: string,
): string | null {
  const idx = chunks.findIndex((c) => c.chunkId === aiGroupId);
  if (idx < 0) {
    return null;
  }
  // 被打断 turn：aiGroupId 即 UserChunk 自身，直接定位。
  if (chunks[idx].kind === "user") {
    return aiGroupId;
  }
  // 完整 turn：向前找紧邻 UserChunk。
  for (let i = idx - 1; i >= 0; i--) {
    if (chunks[i].kind === "user") {
      return chunks[i].chunkId;
    }
  }
  // 无前置 UserChunk：退化为 AIChunk 本身。
  return aiGroupId;
}
