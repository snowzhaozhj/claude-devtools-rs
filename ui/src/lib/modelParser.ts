/**
 * Claude 模型字符串解析：对齐原版
 * `../claude-devtools/src/shared/utils/modelParser.ts`。
 *
 * 输入 `claude-haiku-4-5-20251001` → `{ name: "haiku4.5", family: "haiku" }`。
 * 输入 `<synthetic>` / 空串 / 非 `claude` 前缀 → `null`。
 */

export type KnownModelFamily = "sonnet" | "opus" | "haiku";
export type ModelFamily = KnownModelFamily | string;

export interface ModelInfo {
  name: string;
  family: ModelFamily;
  majorVersion: number;
  minorVersion: number | null;
}

const KNOWN_FAMILIES: KnownModelFamily[] = ["sonnet", "opus", "haiku"];

export function parseModelString(model: string | undefined | null): ModelInfo | null {
  if (!model || model.trim() === "" || model === "<synthetic>") return null;

  const normalized = model.toLowerCase().trim();
  if (!normalized.startsWith("claude")) return null;

  const parts = normalized.split("-");
  if (parts.length < 3) return null;

  let family: ModelFamily | null = null;
  let familyIndex = -1;

  for (let i = 1; i < parts.length; i++) {
    if (KNOWN_FAMILIES.includes(parts[i] as KnownModelFamily)) {
      family = parts[i] as KnownModelFamily;
      familyIndex = i;
      break;
    }
  }
  if (family === null) {
    for (let i = 1; i < parts.length; i++) {
      const p = parts[i];
      if (!/^\d+$/.test(p) && !/^\d{8}$/.test(p) && p.length > 1) {
        family = p;
        familyIndex = i;
        break;
      }
    }
  }
  if (family === null || familyIndex === -1) return null;

  let majorVersion: number;
  let minorVersion: number | null = null;

  if (familyIndex === 1) {
    // 新格式 claude-{family}-{major}[-{minor}]-{date}
    if (parts.length < 4) return null;
    majorVersion = parseInt(parts[2], 10);
    if (Number.isNaN(majorVersion)) return null;
    if (parts.length >= 4 && parts[3].length <= 2) {
      const m = parseInt(parts[3], 10);
      if (!Number.isNaN(m)) minorVersion = m;
    }
  } else {
    // 老格式 claude-{major}[-{minor}]-{family}-{date}
    majorVersion = parseInt(parts[1], 10);
    if (Number.isNaN(majorVersion)) return null;
    if (familyIndex > 2) {
      const m = parseInt(parts[2], 10);
      if (!Number.isNaN(m)) minorVersion = m;
    }
  }

  const versionString = minorVersion !== null ? `${majorVersion}.${minorVersion}` : `${majorVersion}`;
  return {
    name: `${family}${versionString}`,
    family,
    majorVersion,
    minorVersion,
  };
}

/**
 * 根据 family 给模型名元素分配颜色 class。暂沿用原版一致 muted 风格。
 */
export function getModelColorClass(family: ModelFamily): string {
  switch (family) {
    case "opus":
    case "sonnet":
    case "haiku":
      return "model-family-known";
    default:
      return "model-family-unknown";
  }
}
