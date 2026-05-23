/**
 * 平台检测 + 键盘快捷键归一化 helper（前端，运行时检测）。
 *
 * 用 `navigator.userAgentData.platform`（Chromium 90+）优先，fallback `navigator.platform`
 * （deprecated 但 Tauri WKWebView / 老 WebView2 仍可读）。SSR / Node 环境兜底返回 false。
 *
 * 键盘归一化部分（modKey / normalize / normalizeBinding / formatShortcut / matchEvent / parseShortcut）
 * 服务于 `ui/src/lib/keyboard/` 注册中心；详 `openspec/specs/keyboard-shortcuts/spec.md`。
 */

let cached: boolean | null = null;

export function isMac(): boolean {
  if (cached !== null) return cached;
  if (typeof navigator === "undefined") {
    cached = false;
    return cached;
  }
  // 优先 userAgentData（Chromium）；userAgentData.platform 是同步可读的 brand info
  const uaData = (navigator as { userAgentData?: { platform?: string } }).userAgentData;
  if (uaData?.platform) {
    cached = /mac/i.test(uaData.platform);
    return cached;
  }
  // Fallback：navigator.platform 在 macOS 上返回 "MacIntel" / "MacPPC" / "Mac68K"
  // Tauri WKWebView 上仍可用；Safari / 旧 Chrome 也仍 expose
  cached = /mac/i.test(navigator.platform || "") || /mac/i.test(navigator.userAgent || "");
  return cached;
}

/**
 * 测试 hook：重置 cache，让下次 isMac() 重新检测。
 * 仅用于 vitest / playwright spec 注入 navigator 平台后强制重读。
 */
export function _resetPlatformCache(): void {
  cached = null;
}

// ---------------------------------------------------------------------------
// 键盘快捷键归一化
// ---------------------------------------------------------------------------

/**
 * Binding 表达：单 string（两平台同义）或 `{ mac, other }`（双平台）。
 * 例：
 *  - `"mod+k"` —— mac 上展开为 `meta+k`、其他展开为 `ctrl+k`
 *  - `{ mac: "mod+ArrowDown", other: "ctrl+End" }` —— 跨平台分支
 */
export type ShortcutBinding = string | { mac: string; other: string };

/** 跨平台 mod 修饰键：mac 为 meta（Command）、其他为 ctrl（Control） */
export function modKey(): "meta" | "ctrl" {
  return isMac() ? "meta" : "ctrl";
}

// 内部排序：modifier 按字母顺序（避免 "shift+meta" 与 "meta+shift" 命中不同）
const MOD_ORDER_INTERNAL: Record<string, number> = {
  alt: 0,
  ctrl: 1,
  meta: 2,
  shift: 3,
};

// Apple HIG 推荐顺序（mac 展示）：⌃ Control / ⌥ Option / ⇧ Shift / ⌘ Command
const MOD_ORDER_DISPLAY: Record<string, number> = {
  ctrl: 0,
  alt: 1,
  shift: 2,
  meta: 3,
};

const MAC_SYMBOLS: Record<string, string> = {
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
  meta: "⌘",
};

// `meta: "Win"` 是防御性 fallback——`recordBindingFromEvent` 与 `normalizeBindingToMod`
// 已把所有录入与持久化路径统一到 `mod` 字面量，正常 binding 不会出现 `meta` token 在
// non-mac 平台被 formatShortcut 渲染。保留该映射防止"用户手工编辑 cdt-config 写死
// `meta+x`" 等极端 case 时 formatShortcut 输出 undefined 引发渲染错乱。
const WIN_TEXT: Record<string, string> = {
  ctrl: "Ctrl",
  alt: "Alt",
  shift: "Shift",
  meta: "Win",
};

const ARROW_DISPLAY: Record<string, string> = {
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

/**
 * 把 KeyboardEvent.key + KeyboardEvent.code 转成 canonical 主键 token。
 * - modifier 自身（Meta / Control / Alt / Shift）返回空串
 * - 字母统一小写
 * - Numpad 数字键归一化为顶部数字（"Numpad1" → "1"，与 "Digit1" 同义）
 * - Numpad 功能键归一为对应 main row（"NumpadEnter" → "Enter" 等）
 * - 物理位置键（`[` / `]` / `\\` / `/` / `-` / `=` 等）按 event.code 兜底
 * - 命名键（ArrowUp / Escape / Enter / F1..F12 等）保持 PascalCase
 */
export function canonicalKey(key: string, code: string): string {
  // 过滤 modifier 自身（按下单个修饰键时 event.key 是 "Meta" / "Control" / ...）
  if (key === "Meta" || key === "Control" || key === "Alt" || key === "Shift") return "";

  // Numpad 数字键
  if (/^Numpad[0-9]$/.test(code)) return code.charAt(6);
  // Numpad 功能键
  switch (code) {
    case "NumpadEnter":
      return "Enter";
    case "NumpadAdd":
      // **不**返回字面 "+"——`+` 是 binding 字符串的修饰键分隔符（normalizeBinding
      // L210 split("+")）；返回 "+" 时 `ctrl++` 被 split 成 ["ctrl",""] 主键丢失。
      // 用 "Plus" token 避免歧义；formatShortcut Windows 仍展示字面 "+"。
      return "Plus";
    case "NumpadSubtract":
      return "-";
    case "NumpadMultiply":
      return "*";
    case "NumpadDivide":
      return "/";
    case "NumpadDecimal":
      return ".";
  }

  // 物理位置键：event.code 兜底（应对 AZERTY / Dvorak 等布局）
  switch (code) {
    case "BracketLeft":
      return "[";
    case "BracketRight":
      return "]";
    case "Backslash":
      return "\\";
    case "Slash":
      return "/";
    case "Minus":
      return "-";
    case "Equal":
      return "=";
    case "Comma":
      return ",";
    case "Period":
      return ".";
    case "Quote":
      return "'";
    case "Semicolon":
      return ";";
    case "Backquote":
      return "`";
  }
  if (/^Digit[0-9]$/.test(code)) return code.charAt(5);

  // 字母键统一小写
  if (key.length === 1 && /^[a-zA-Z]$/.test(key)) return key.toLowerCase();
  // Space 归一
  if (key === " " || code === "Space") return "Space";

  // 其他：保持 event.key（PascalCase 命名键如 ArrowUp / Enter / Escape / End / F1）
  return key;
}

function sortMods(mods: string[]): string[] {
  const uniq = Array.from(new Set(mods));
  uniq.sort((a, b) => (MOD_ORDER_INTERNAL[a] ?? 99) - (MOD_ORDER_INTERNAL[b] ?? 99));
  return uniq;
}

/**
 * 把 KeyboardEvent 归一化为 NormalizedKey 字符串（如 `"meta+shift+k"`），供 Map 索引。
 * - non-mac 平台禁止把 metaKey 识别为 mod（防 Win 键 / 神秘键盘的误触发）
 * - **AltGraph 守卫**：Windows 德语 / 法语等键盘的 AltGr 物理键被浏览器合成为
 *   `ctrlKey=true + altKey=true`（实际语义是输入扩展字符如 `@` / `\`），若按 mod
 *   识别会把 `AltGr+Q` 误命中 `ctrl+alt+q`。`event.getModifierState("AltGraph")`
 *   返回 true 时强制把 ctrl / alt 视作未按下，让用户按 AltGr+任意字符不触发任何
 *   shortcut（Numpad 等其他主键归一仍生效）。
 * - 修饰键按字母顺序排列
 * - 主键经 `canonicalKey` 处理
 * - 仅按下修饰键自身（无主键）返回空串
 */
export function normalize(event: KeyboardEvent): string {
  // KeyboardEvent.ctrlKey / altKey 是 read-only，AltGraph 守卫用本地变量遮罩
  let ctrl = event.ctrlKey;
  let alt = event.altKey;
  // jsdom 下 getModifierState 可能不存在，做存在性判定
  if (typeof event.getModifierState === "function" && event.getModifierState("AltGraph")) {
    ctrl = false;
    alt = false;
  }

  const mods: string[] = [];
  // 平台分流：non-mac 平台禁识 metaKey
  if (isMac() && event.metaKey) mods.push("meta");
  if (ctrl) mods.push("ctrl");
  if (alt) mods.push("alt");
  if (event.shiftKey) mods.push("shift");

  const main = canonicalKey(event.key || "", event.code || "");
  if (!main) return "";

  const sorted = sortMods(mods);
  return sorted.length > 0 ? `${sorted.join("+")}+${main}` : main;
}

/**
 * 录键产物函数：把 KeyboardEvent 转为跨平台 `mod` 字面量 binding（如 `"mod+shift+p"`）。
 *
 * 流程：
 * 1. 调 `normalize(event)` 得平台特化字符串（mac `meta+shift+p` / win `ctrl+shift+p`）
 * 2. 把当前平台主修饰键反写为 `mod`：mac 上替换 `meta` → `mod`、win/linux 上替换 `ctrl` → `mod`
 *
 * 仅按下 modifier（无主键）时返回 `null`。
 *
 * 这是 `KeyRecorderInput` commit binding 的唯一来源——录键产物 SHALL NOT 直接使用
 * `normalize(event)` 的平台特化字面量，确保 cdt-config 持久化 binding 跨平台一致。
 *
 * 注意 mac 双修饰键 case：`Cmd+Ctrl+X` 经 normalize 输出 `ctrl+meta+x`（按内部排序），
 * 反写主修饰键 `meta` → `mod` 得 `ctrl+mod+x`，dispatcher 入口 `normalizeBinding` 在 mac
 * 展开为 `ctrl+meta+x` 与原事件等价。
 */
export function recordBindingFromEvent(event: KeyboardEvent): string | null {
  const normalized = normalize(event);
  if (!normalized) return null;

  const platformMod = isMac() ? "meta" : "ctrl";
  const tokens = normalized.split("+");
  const lastIdx = tokens.length - 1;
  // 仅在 modifier 位置（除主键外）找平台主修饰键
  for (let i = 0; i < lastIdx; i++) {
    if (tokens[i] === platformMod) {
      tokens[i] = "mod";
      return tokens.join("+");
    }
  }
  // 没主修饰键（如 alt+x / shift+x），原样返回
  return normalized;
}

/**
 * 字面量迁移函数：把存量平台特化字面量（`meta+x` / `ctrl+x`）转为 `mod` 字面量。
 *
 * **token-level 算法**（不依赖 token 位置或前缀，鲁棒覆盖用户手编非 sorted 字面量）：
 * 1. `binding.split("+")` 得 token 数组
 * 2. 若数组中**已含** `mod` token：保留 token 顺序，移除主键之外位置的 `meta` / `ctrl`（防御
 *    异常字面量如 `meta+mod+x` → `mod+x`）；不重排
 * 3. 否则按 **主修饰键优先级 meta > ctrl** 在 modifier 位置（除主键外）找替换目标：
 *    - 优先找第一个 `meta` token 替换为 `mod`（mac 双修饰键 `ctrl+meta+x` 优先替 meta 得
 *      `ctrl+mod+x`，保留 ctrl 为辅助修饰键）
 *    - 若数组无 `meta`，再找第一个 `ctrl` token 替换为 `mod`
 *    - 找到一个即返回
 * 4. 不含 `meta` / `ctrl` 主修饰键的 binding（如 `alt+x`、`shift+x`、`F1`）原样返回
 *
 * 不在内部重排 sort——sort 由 dispatcher 入口 `normalizeBinding` 在 register 时统一处理。
 *
 * 详见 `openspec/specs/keyboard-shortcuts/spec.md::跨平台修饰键归一化`。
 */
export function normalizeBindingToMod(binding: string): string {
  if (!binding) return "";
  const tokens = binding
    .split("+")
    .map((t) => t.trim())
    .filter(Boolean);
  if (tokens.length === 0) return "";

  const lastIdx = tokens.length - 1;
  const hasMod = tokens.slice(0, lastIdx).includes("mod");

  if (hasMod) {
    // 移除主键之外位置的 meta / ctrl（防御异常字面量）
    const cleaned = tokens.filter((t, i) => {
      if (i === lastIdx) return true;
      return t !== "meta" && t !== "ctrl";
    });
    return cleaned.join("+");
  }

  // 优先替换 meta（mac 主修饰键）
  for (let i = 0; i < lastIdx; i++) {
    if (tokens[i] === "meta") {
      tokens[i] = "mod";
      return tokens.join("+");
    }
  }
  // 退而替换 ctrl（win/linux 主修饰键）
  for (let i = 0; i < lastIdx; i++) {
    if (tokens[i] === "ctrl") {
      tokens[i] = "mod";
      return tokens.join("+");
    }
  }
  // 无主修饰键，原样返回
  return tokens.join("+");
}

/**
 * 把 binding 字符串（`"mod+shift+K"`）归一化到当前平台对应的 NormalizedKey
 * （mac → `"meta+shift+k"`、其他 → `"ctrl+shift+k"`）。
 * 修饰键按字母顺序排列，字母键统一小写，命名键保持 PascalCase。
 */
export function normalizeBinding(binding: string): string {
  if (!binding) return "";
  const parts = binding
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean);
  if (parts.length === 0) return "";
  const mods: string[] = [];
  let main = "";

  for (const p of parts) {
    const lower = p.toLowerCase();
    if (lower === "mod") {
      mods.push(modKey());
    } else if (lower === "ctrl" || lower === "control") {
      mods.push("ctrl");
    } else if (lower === "alt" || lower === "option" || lower === "opt") {
      mods.push("alt");
    } else if (lower === "shift") {
      mods.push("shift");
    } else if (lower === "meta" || lower === "cmd" || lower === "command") {
      mods.push("meta");
    } else if (lower === "win") {
      // `"win"` 是历史保留 token：mac 上展开为 meta（Command）兼容旧 binding；
      // **non-mac 平台 normalize() 不识 metaKey**（见上面 normalize 注释），所以把
      // Windows 用户的 `"win+x"` 当 mod 写实际永远不命中。dev mode 显式 warn
      // 提醒作者改用 `mod` 关键字（mac 展为 meta、非 mac 展为 ctrl）。
      if (import.meta.env.DEV) {
        // eslint-disable-next-line no-console
        console.warn(
          `[keyboard] "win" token in binding has no effect on Windows; metaKey is intentionally ignored on non-mac platforms (use "mod" instead)`,
        );
      }
      mods.push("meta");
    } else {
      // 主键
      if (p.length === 1 && /^[a-zA-Z]$/.test(p)) {
        main = p.toLowerCase();
      } else if (lower === "plus") {
        // 用户写 "ctrl+plus" / "ctrl+Plus" 都归一到内部 token "Plus"（PascalCase
        // 与 canonicalKey("NumpadAdd") 输出一致），避免 case 错配
        main = "Plus";
      } else {
        main = p; // 命名键（ArrowDown / Enter / Escape / End）保留原样
      }
    }
  }

  const sorted = sortMods(mods);
  if (!main) return "";
  return sorted.length > 0 ? `${sorted.join("+")}+${main}` : main;
}

/**
 * 把 ShortcutBinding 解析为当前平台 NormalizedKey 字符串。
 */
export function resolveBinding(binding: ShortcutBinding): string {
  if (typeof binding === "string") return normalizeBinding(binding);
  return normalizeBinding(isMac() ? binding.mac : binding.other);
}

/**
 * 判断 KeyboardEvent 是否匹配指定 binding。
 */
export function matchEvent(binding: ShortcutBinding, event: KeyboardEvent): boolean {
  const target = resolveBinding(binding);
  if (!target) return false;
  return normalize(event) === target;
}

function formatMainKey(key: string): string {
  if (key.length === 1 && /^[a-z]$/.test(key)) return key.toUpperCase();
  if (key in ARROW_DISPLAY) return ARROW_DISPLAY[key];
  // 内部 token "Plus" 用于避免与 binding 分隔符 "+" 冲突；展示层还原为字面 "+"
  if (key === "Plus") return "+";
  // Space 在 mac 用 U+2423 OPEN BOX 符号（对齐 Apple HIG 推荐表达），其他平台保留文本
  if (key === "Space") return isMac() ? "␣" : "Space";
  return key;
}

/**
 * 展示给用户的快捷键文本：
 * - macOS：Apple HIG 顺序（⌃⌥⇧⌘）+ 主键，如 `"⇧⌘K"`、`"⌥⇧⌘K"`
 * - Windows / Linux：`Ctrl+Alt+Shift+K` 文本前缀
 *
 * 注意 NormalizedKey 内部的字母顺序与展示顺序解耦——`formatShortcut` 是纯展示层。
 */
export function formatShortcut(binding: ShortcutBinding): string {
  const normalized = resolveBinding(binding);
  if (!normalized) return "";
  const parts = normalized.split("+");
  if (parts.length === 0) return "";
  const main = parts[parts.length - 1];
  const mods = parts.slice(0, -1);

  if (isMac()) {
    mods.sort((a, b) => (MOD_ORDER_DISPLAY[a] ?? 99) - (MOD_ORDER_DISPLAY[b] ?? 99));
    const sym = mods.map((m) => MAC_SYMBOLS[m] ?? m).join("");
    return sym + formatMainKey(main);
  }
  mods.sort((a, b) => (MOD_ORDER_DISPLAY[a] ?? 99) - (MOD_ORDER_DISPLAY[b] ?? 99));
  const text = mods.map((m) => WIN_TEXT[m] ?? m);
  text.push(formatMainKey(main));
  return text.join("+");
}

/**
 * 解析 binding 字符串为结构化 { mods, key }。归一化失败返回 null。
 */
export function parseShortcut(binding: string): { mods: string[]; key: string } | null {
  const normalized = normalizeBinding(binding);
  if (!normalized) return null;
  const parts = normalized.split("+");
  const key = parts[parts.length - 1];
  const mods = parts.slice(0, -1);
  return { mods, key };
}
