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
