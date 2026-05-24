/**
 * Sidebar metadata-pending shimmer 触发判定。
 *
 * Issue #259：原触发条件 `!title && messageCount===0 && !isOngoing` 让
 * 任何骨架态 session 进入 1500 ms infinite CSS animation，启动期 sidebar
 * 同时存在数十个 pending 项时持续触发 GPU compositor 合成帧。本模块把判定
 * 收紧为「pending 且首次观察距今 > METADATA_SHIMMER_DELAY_MS」——绝大多数
 * metadata 在阈值前到达的 session 不再闪 shimmer 一帧。
 *
 * Sidebar 内消费：
 *   1. `isSessionMetadataPending(s)` 判定是否仍是骨架（消费侧用于决定是否
 *      在 metadataRequestedAt map 中登记/清除）。
 *   2. `shouldShowMetadataShimmer(s, requestedAt, now)` 判定是否真要挂
 *      .metadata-pending CSS class。
 *
 * 参数化阈值便于单测注入（不暴露给运行时调用方修改）。
 */
import type { SessionSummary } from "./api";

export const METADATA_SHIMMER_DELAY_MS = 1500;

export function isSessionMetadataPending(s: SessionSummary): boolean {
  return !s.title && s.messageCount === 0 && !s.isOngoing;
}

export function shouldShowMetadataShimmer(
  s: SessionSummary,
  requestedAt: number | null | undefined,
  now: number,
  thresholdMs: number = METADATA_SHIMMER_DELAY_MS,
): boolean {
  if (!isSessionMetadataPending(s)) return false;
  if (requestedAt === null || requestedAt === undefined) return false;
  return now - requestedAt > thresholdMs;
}
