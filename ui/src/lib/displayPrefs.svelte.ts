/**
 * Display 偏好的全局响应式 store——目前仅承载 timeFormat，留口子未来叠 compact /
 * showTimestamps 等渲染期偏好。
 *
 * 启动时由 App.svelte 从 backend config (`display.timeFormat`) 同步；
 * SettingsView 修改时立即 setter 同步，无需等组件 reload。
 *
 * 详见 `openspec/specs/configuration-management/spec.md` §"Display config
 * exposes time format preference"。
 */
import type { TimeFormat } from "./api";

let timeFormat: TimeFormat = $state("24h");

export function getTimeFormat(): TimeFormat {
  return timeFormat;
}

export function setTimeFormat(value: TimeFormat): void {
  timeFormat = value;
}
