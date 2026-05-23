/** Lucide icon SVG paths (viewBox="0 0 24 24", stroke-based) */

export const WRENCH =
  "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z";

export const BRAIN =
  "M9.5 2A5.5 5.5 0 0 0 5 5.06C3.35 5.4 2 6.93 2 9.11c0 1.63.67 2.82 1.69 3.63C3.2 13.64 3 14.79 3 16c0 2.76 2.24 5 5 5h1M14.5 2A5.5 5.5 0 0 1 19 5.06C20.65 5.4 22 6.93 22 9.11c0 1.63-.67 2.82-1.69 3.63.49.9.69 2.05.69 3.26 0 2.76-2.24 5-5 5h-1M12 2v20";

export const MESSAGE_SQUARE =
  "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z";

export const TERMINAL =
  "M4 17l6-6-6-6M12 19h8";

export const CHEVRON_RIGHT =
  "M9 18l6-6-6-6";

/** Lucide layers — Compact boundary 标识 */
export const LAYERS =
  "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83zM2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17";

export const SLASH =
  "M22 2L2 22";

/** Lucide bell — 通知 */
export const BELL =
  "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9M10.3 21a1.94 1.94 0 0 0 3.4 0";

/** Lucide settings (齿轮) */
export const SETTINGS =
  "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z";

/** Lucide chevron-down — 下拉箭头（单 path） */
export const CHEVRON_DOWN = "M6 9l6 6 6-6";

/** Lucide chevrons-down — 双下箭头，用于"跳到最新消息"浮层按钮（多段，使用 {@html}） */
export const CHEVRONS_DOWN_SVG = `
<path d="m7 6 5 5 5-5"/>
<path d="m7 13 5 5 5-5"/>
`;

/** Lucide git-branch — 分支显示（多段，使用 {@html}） */
export const GIT_BRANCH_SVG = `
<line x1="6" x2="6" y1="3" y2="15"/>
<circle cx="18" cy="6" r="3"/>
<circle cx="6" cy="18" r="3"/>
<path d="M18 9a9 9 0 0 1-9 9"/>
`;

/** Lucide panel-left — 侧栏折叠/展开按钮（多段，使用 {@html}） */
export const PANEL_LEFT_SVG = `
<rect width="18" height="18" x="3" y="3" rx="2"/>
<path d="M9 3v18"/>
`;

/** Lucide alert-triangle — interruption 警告 icon（多段，使用 {@html}） */
export const ALERT_TRIANGLE_SVG = `
<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/>
<path d="M12 9v4"/>
<path d="M12 17h.01"/>
`;

/** Lucide corner-down-left — 单 path 折返箭头（用于 reply-to chip） */
export const CORNER_DOWN_LEFT = "M9 10l-5 5 5 5M20 4v7a4 4 0 0 1-4 4H4";

/** Lucide refresh-cw — 单 path 简化（resend 标记） */
export const REFRESH_CW = "M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5";

/** Lucide check-check — 批量标记已读（两条对勾） */
export const CHECK_CHECK_SVG = `
<path d="M18 6 7 17l-5-5"/>
<path d="m22 10-7.5 7.5L13 16"/>
`;

/** Lucide check — 单条标记已读（一条对勾） */
export const CHECK_SVG = `
<path d="M20 6 9 17l-5-5"/>
`;

/** Lucide trash-2 — 清空通知 */
export const TRASH2_SVG = `
<path d="M3 6h18"/>
<path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/>
<path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
<line x1="10" x2="10" y1="11" y2="17"/>
<line x1="14" x2="14" y1="11" y2="17"/>
`;

/** Lucide x — 关闭 / 删除 */
export const X_SVG = `
<path d="M18 6 6 18"/>
<path d="m6 6 12 12"/>
`;

/** Lucide file-text — session tab icon（多段，使用 {@html}） */
export const FILE_TEXT_SVG = `
<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z"/>
<path d="M14 2v6h6"/>
<path d="M16 13H8"/>
<path d="M16 17H8"/>
<path d="M10 9H8"/>
`;

/** Lucide book-open-text — Memory tab / sidebar icon（多段，使用 {@html}） */
export const BOOK_OPEN_TEXT_SVG = `
<path d="M12 7v14"/>
<path d="M16 12h2"/>
<path d="M16 8h2"/>
<path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"/>
<path d="M6 12h2"/>
<path d="M6 8h2"/>
`;

/** Lucide folder-git-2 — 项目卡片/侧栏 icon（多段，使用 {@html}） */
export const FOLDER_GIT2_SVG = `
<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H20a2 2 0 0 1 2 2v1"/>
<circle cx="13" cy="12" r="2"/>
<path d="M18 19c-2.8 0-5-2.2-5-5v8"/>
<circle cx="20" cy="19" r="2"/>
`;

/** Lucide clock — 耗时 */
export const CLOCK_SVG = `
<circle cx="12" cy="12" r="10"/>
<polyline points="12 6 12 12 16 14"/>
`;

/** Lucide user — 用户气泡 avatar */
export const USER_SVG = `
<path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
<circle cx="12" cy="7" r="4"/>
`;

/** Lucide bell-off — 通知 empty state */
export const BELL_OFF_SVG = `
<path d="M8.7 3A6 6 0 0 1 18 8c0 2.5 1.5 5 2 5h-2"/>
<path d="M16.5 16.5C15.8 16.8 14.9 17 14 17H3s3-2 3-9c0-.7.1-1.4.3-2"/>
<path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>
<line x1="2" y1="2" x2="22" y2="22"/>
`;

/** Lucide arrow-up — SearchBar prev */
export const ARROW_UP_SVG = `
<line x1="12" y1="19" x2="12" y2="5"/>
<polyline points="5 12 12 5 19 12"/>
`;

/** Lucide arrow-down — SearchBar next */
export const ARROW_DOWN_SVG = `
<line x1="12" y1="5" x2="12" y2="19"/>
<polyline points="19 12 12 19 5 12"/>
`;

/** Lucide sliders-horizontal — 设置 / 常规 section */
export const SLIDERS_HORIZONTAL_SVG = `
<line x1="21" x2="14" y1="4" y2="4"/>
<line x1="10" x2="3" y1="4" y2="4"/>
<line x1="21" x2="12" y1="12" y2="12"/>
<line x1="8" x2="3" y1="12" y2="12"/>
<line x1="21" x2="16" y1="20" y2="20"/>
<line x1="12" x2="3" y1="20" y2="20"/>
<line x1="14" x2="14" y1="2" y2="6"/>
<line x1="8" x2="8" y1="10" y2="14"/>
<line x1="16" x2="16" y1="18" y2="22"/>
`;

/** Lucide monitor — 显示 section */
export const MONITOR_SVG = `
<rect width="20" height="14" x="2" y="3" rx="2"/>
<line x1="8" x2="16" y1="21" y2="21"/>
<line x1="12" x2="12" y1="17" y2="21"/>
`;

/** Lucide info — 关于 section */
export const INFO_SVG = `
<circle cx="12" cy="12" r="10"/>
<line x1="12" x2="12" y1="16" y2="12"/>
<line x1="12" x2="12.01" y1="8" y2="8"/>
`;

/** Lucide keyboard — 键盘快捷键 section */
export const KEYBOARD_SVG = `
<rect width="20" height="16" x="2" y="4" rx="2" ry="2"/>
<path d="M6 8h.01"/>
<path d="M10 8h.01"/>
<path d="M14 8h.01"/>
<path d="M18 8h.01"/>
<path d="M8 12h.01"/>
<path d="M12 12h.01"/>
<path d="M16 12h.01"/>
<path d="M7 16h10"/>
`;

/** Lucide folder — 目录选择按钮 */
export const FOLDER_SVG = `
<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>
`;

/** Lucide rotate-ccw — 恢复默认 */
export const ROTATE_CCW_SVG = `
<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
<path d="M3 3v5h5"/>
`;

/** Lucide plus — 添加 */
export const PLUS_SVG = `
<path d="M5 12h14"/>
<path d="M12 5v14"/>
`;

/** Lucide check-circle — 已是最新 */
export const CHECK_CIRCLE_SVG = `
<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
<polyline points="22 4 12 14.01 9 11.01"/>
`;

/** Lucide download-cloud — 发现新版本 */
export const DOWNLOAD_CLOUD_SVG = `
<path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242"/>
<path d="M12 12v9"/>
<path d="m16 17-4 4-4-4"/>
`;

/** Lucide alert-circle — 检查失败 */
export const ALERT_CIRCLE_SVG = `
<circle cx="12" cy="12" r="10"/>
<line x1="12" x2="12" y1="8" y2="12"/>
<line x1="12" x2="12.01" y1="16" y2="16"/>
`;

/** Lucide bell-ring — 通知 empty state CTA icon */
export const BELL_RING_SVG = `
<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/>
<path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>
<path d="M4 2C2.8 3.7 2 5.7 2 8"/>
<path d="M22 8c0-2.3-.8-4.3-2-6"/>
`;

export const WIFI_SVG = `
<path d="M12 20h.01"/>
<path d="M8.5 16.5a5 5 0 0 1 7 0"/>
<path d="M5 13a10 10 0 0 1 14 0"/>
<path d="M2 9.5a15 15 0 0 1 20 0"/>
`;

export const WIFI_OFF_SVG = `
<path d="M12 20h.01"/>
<path d="M8.5 16.5a5 5 0 0 1 7 0"/>
<path d="M2 8.82a15 15 0 0 1 4.17-2.65"/>
<path d="M10.66 5.11A15 15 0 0 1 22 8.82"/>
<path d="m2 2 20 20"/>
`;

export const SERVER_SVG = `
<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/>
<rect width="20" height="8" x="2" y="14" rx="2" ry="2"/>
<line x1="6" x2="6.01" y1="6" y2="6"/>
<line x1="6" x2="6.01" y1="18" y2="18"/>
`;
