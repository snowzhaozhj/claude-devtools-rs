/**
 * 把 `Worktree.cwd_relative_to_repo_root` 压缩为 sidebar 行 chip 标签：
 * 取相对路径最后两段。spec `Session row branch + cwd chip` Scenario：
 * - `crates` → `crates`
 * - `crates/cdt-discover` → `crates/cdt-discover`
 * - `.claude/worktrees/feat-x` → `worktrees/feat-x`
 * 空 / null / 仅分隔符返回空串，调用方按 truthy 判断决定是否渲染。
 */
export function cwdRelativeHintLabel(cwdRel: string | undefined | null): string {
  if (!cwdRel) return ''
  const parts = cwdRel.split(/[/\\]/).filter(Boolean)
  if (parts.length === 0) return ''
  if (parts.length === 1) return parts[0]
  return parts.slice(-2).join('/')
}
