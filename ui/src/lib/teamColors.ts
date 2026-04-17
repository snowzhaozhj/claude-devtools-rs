/**
 * Team & subagent color palette.
 *
 * 移植自 `../claude-devtools/src/renderer/constants/teamColors.ts`。
 * 原版用于 TeammateMessageItem 与 SubagentItem 的彩色 badge / 圆点。
 *
 * 调色板：8 色（blue/green/red/yellow/purple/cyan/orange/pink）。
 * 每个 color 提供 border/badge/text 三联，用于 CSS 渲染。
 */

export interface TeamColorSet {
  border: string;
  badge: string;
  text: string;
}

const TEAMMATE_COLORS: Record<string, TeamColorSet> = {
  blue: { border: "#3b82f6", badge: "rgba(59, 130, 246, 0.15)", text: "#60a5fa" },
  green: { border: "#22c55e", badge: "rgba(34, 197, 94, 0.15)", text: "#4ade80" },
  red: { border: "#ef4444", badge: "rgba(239, 68, 68, 0.15)", text: "#f87171" },
  yellow: { border: "#eab308", badge: "rgba(234, 179, 8, 0.15)", text: "#facc15" },
  purple: { border: "#a855f7", badge: "rgba(168, 85, 247, 0.15)", text: "#c084fc" },
  cyan: { border: "#06b6d4", badge: "rgba(6, 182, 212, 0.15)", text: "#22d3ee" },
  orange: { border: "#f97316", badge: "rgba(249, 115, 22, 0.15)", text: "#fb923c" },
  pink: { border: "#ec4899", badge: "rgba(236, 72, 153, 0.15)", text: "#f472b6" },
};

const COLOR_NAMES = Object.keys(TEAMMATE_COLORS);
const DEFAULT_COLOR: TeamColorSet = TEAMMATE_COLORS.blue;

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash * 31 + str.charCodeAt(i)) | 0;
  }
  return Math.abs(hash);
}

/** Team 成员颜色：named preset → hex fallback → default blue。 */
export function getTeamColorSet(colorName: string | null | undefined): TeamColorSet {
  if (!colorName) return DEFAULT_COLOR;
  const named = TEAMMATE_COLORS[colorName.toLowerCase()];
  if (named) return named;
  if (colorName.startsWith("#")) {
    return { border: colorName, badge: `${colorName}26`, text: colorName };
  }
  return DEFAULT_COLOR;
}

/**
 * Subagent type 颜色：优先查 agent config 的 color 字段，未命中走
 * `hash(subagentType) % palette.length` 决定性映射。
 */
export function getSubagentTypeColorSet(
  subagentType: string,
  agentConfigs?: Record<string, { color?: string | null }>,
): TeamColorSet {
  const configColor = agentConfigs?.[subagentType]?.color;
  if (configColor) {
    return getTeamColorSet(configColor);
  }
  const idx = hashString(subagentType) % COLOR_NAMES.length;
  return TEAMMATE_COLORS[COLOR_NAMES[idx]];
}
