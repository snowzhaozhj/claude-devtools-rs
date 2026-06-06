<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { getSessionDetail, getToolOutput, type SessionDetail, type SessionDetailResponse, type Chunk, type AIChunk, type ChunkMetrics, type ToolExecution, type ToolOutput } from "../lib/api";
  import { getToolSummary, getToolStatus, getToolDurationMs, isToolPending, cleanDisplayText, parseTaskNotifications, getToolContextTokens, estimateTokens, viewerUsesOutput, shouldPrefetchOnChunkExpand } from "../lib/toolHelpers";
  import { buildDisplayItemsCached, buildSummary } from "../lib/displayItemBuilder";
  import { WRENCH, BRAIN, TERMINAL, SLASH, MESSAGE_SQUARE, CHEVRON_RIGHT, LAYERS, CLOCK_SVG, USER_SVG, USER_ICON, ALERT_TRIANGLE_SVG, CHEVRONS_DOWN_SVG } from "../lib/icons";
  import { formatClock, formatTokensCompact } from "../lib/formatters";
  import { getTimeFormat } from "../lib/displayPrefs.svelte";
  import { tick } from "svelte";
  import { clearHighlights } from "../lib/searchHighlight";
  import { processMermaidBlocks } from "../lib/mermaid";
  import { createLazyMarkdownObserver, estimatePlaceholderHeight, isScrollCompensating } from "../lib/lazyMarkdown.svelte";
  import { isAtBottom, captureScrollAnchor, restoreScrollAnchor, startBottomPin, type ScrollAnchorState } from "../lib/scrollAnchor";
  import { getTabUIState, saveTabUIState, getTabSessionId, getCachedSession, setCachedSession } from "../lib/tabStore.svelte";
  import { isMac } from "../lib/platform";
  import {
    registerSessionDetailHandlers,
    unregisterSessionDetailHandlers,
  } from "../lib/keyboard/session-detail-handlers";
  import { registerHandler, unregisterHandler, scheduleRefresh, cancelScheduledRefresh } from "../lib/fileChangeStore.svelte";
  import { contextMenu } from "../lib/contextMenu.svelte";
  import { buildUserMessageItems, buildAssistantMessageItems, type MenuItemContext } from "../lib/contextMenu/menu-items";
  import { getMenuSettings } from "../lib/contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../lib/contextMenu/dispatch";
  import BaseItem from "../components/BaseItem.svelte";
  import SubagentCard from "../components/SubagentCard.svelte";
  import WorkflowCard from "../components/WorkflowCard.svelte";
  import TeammateMessageItem from "../components/TeammateMessageItem.svelte";
  import { getTeamColorSet } from "../lib/teamColors";
  import SearchBar from "../components/SearchBar.svelte";
  import ContextPanel from "../components/ContextPanel.svelte";
  import SessionMetaMenu from "../components/SessionMetaMenu.svelte";
  import {
    parseInjections,
    selectActivePhaseInjections,
  } from "../lib/contextExtractor";
  import OngoingBanner from "../components/OngoingBanner.svelte";
  import SessionDetailSkeleton from "../components/SessionDetailSkeleton.svelte";
  import ImageBlock from "../components/ImageBlock.svelte";
  import DefaultToolViewer from "../components/tool-viewers/DefaultToolViewer.svelte";
  import ReadToolViewer from "../components/tool-viewers/ReadToolViewer.svelte";
  import EditToolViewer from "../components/tool-viewers/EditToolViewer.svelte";
  import WriteToolViewer from "../components/tool-viewers/WriteToolViewer.svelte";
  import BashToolViewer from "../components/tool-viewers/BashToolViewer.svelte";
  import ContextBadge from "../components/ContextBadge.svelte";
  import {
    getPerTurnStats,
    buildInjectionsByTurnMap,
    type TurnContextStats,
    formatTokens as fmtTokens,
    formatCategoryName,
    type ContextInjection as CtxInj,
  } from "../lib/contextExtractor";

  interface Props { tabId: string; projectId: string; sessionId: string; }
  let { tabId, projectId, sessionId }: Props = $props();

  let detail: SessionDetail | null = $state(null);
  const workflowMap = $derived.by(() => {
    const map = new Map<string, import("../lib/api").WorkflowItem>();
    for (const wf of detail?.workflowItems ?? []) {
      map.set(wf.runId, wf);
    }
    return map;
  });
  const workflowRunIds = $derived(new Set(workflowMap.keys()));
  let knownFingerprint: string | null = $state(null);
  let lastChunksFingerprint: string | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let conversationEl: HTMLElement | undefined = $state();

  let openPopoverId: string | null = $state(null);
  function togglePopover(id: string) {
    openPopoverId = openPopoverId === id ? null : id;
  }

  $effect(() => {
    if (!openPopoverId) return;
    function handleOutsideClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (target.closest(".context-badge") || target.closest(".ai-tokens")) return;
      openPopoverId = null;
    }
    function handleEscape(e: KeyboardEvent) {
      if (e.key === "Escape") openPopoverId = null;
    }
    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handleOutsideClick);
      document.removeEventListener("keydown", handleEscape);
    };
  });

  let injectionsByTurn = $derived.by(() => {
    const injs = detail?.contextInjections;
    if (!injs) return new Map<string, CtxInj[]>();
    return buildInjectionsByTurnMap(injs);
  });

  // Lazy markdown observer：root 必须是 conversation 容器；mount 时创建，
  // unmount 时 disconnect。observer 创建于 conversationEl 首次绑定后
  // （processMermaid 后处理需要它，所以 attach 时 lazy 检查）。
  let lazyObserver: ReturnType<typeof createLazyMarkdownObserver> | null = null;
  function ensureObserver(): ReturnType<typeof createLazyMarkdownObserver> | null {
    if (!lazyObserver && conversationEl) {
      lazyObserver = createLazyMarkdownObserver(conversationEl);
    }
    return lazyObserver;
  }
  function attachMarkdown(text: string, kind: "user" | "ai" | "system" | "thinking" | "output" | "slash" | "teammate") {
    return (el: HTMLElement) => {
      const obs = ensureObserver();
      if (!obs) return;
      // 占位高度估算：进入视口前 min-height 控制 layout 稳定
      el.style.minHeight = `${estimatePlaceholderHeight(text, kind)}px`;
      obs.observe(el, text, async (rendered) => {
        // 渲染完成后清理 min-height（让真实高度接管），并扫该子树的 mermaid block
        rendered.style.minHeight = "";
        await processMermaidBlocks(rendered);
      });
    };
  }

  // per-tab UI 状态（从 tabStore 恢复）—— tabId 在组件生命周期内不会变（切 tab 走 destroy/recreate），
  // 用 untrack 显式声明只读初始值，消 Svelte 5 state_referenced_locally warning
  let uiState = getTabUIState(untrack(() => tabId));
  let expandedItems: Set<string> = $state(new Set(uiState.expandedItems));
  let expandedChunks: Set<string> = $state(new Set(uiState.expandedChunks));
  let highlightedChunkId: string | null = $state(null);
  let highlightedToolUseId: string | null = $state(null);
  let highlightTimer: ReturnType<typeof setTimeout> | null = null;
  // Compact 折叠状态——per-chunk 局部 UI state（D4：默认折叠，切 tab 走 destroy/recreate
  // 重置为默认值，对齐原版 CompactBoundary.tsx 的 useState(false)，**不**进 tabStore 持久化）
  let expandedCompacts: Set<string> = $state(new Set());

  function toggleCompact(chunkId: string) {
    const n = new Set(expandedCompacts);
    if (n.has(chunkId)) n.delete(chunkId); else n.add(chunkId);
    expandedCompacts = n;
  }
  let searchVisible = $state(uiState.searchVisible);
  let contextPanelVisible = $state(uiState.contextPanelVisible);
  // SearchBar 内容版本号：refreshDetail 替换 detail 后递增，让 SearchBar
  // 在 visible+query 状态下自动重搜，避免 file-change 后 mark 索引过期。
  let searchContentVersion = $state(0);

  function toggleChunk(chunk: AIChunk) {
    const n = new Set(expandedChunks);
    const opening = !n.has(chunk.chunkId);
    if (opening) n.add(chunk.chunkId); else n.delete(chunk.chunkId);
    expandedChunks = n;
    if (opening) {
      prefetchReadOutputs(chunk);
    }
  }

  function isChunkToolsVisible(chunk: AIChunk): boolean {
    return expandedChunks.has(chunk.chunkId);
  }

  // ── Quick Anchor Navigation（change `session-jump-to-latest`）──
  //
  // 距底 > JUMP_THRESHOLD 时浮现"跳到最新消息"按钮 + 跨平台键盘快捷键
  // （mac ⌘+↓ / Win+Linux Ctrl+End）触发 smooth scroll 到末尾。状态机
  // 由 scrollend 主条件 + 1500ms fallback timer + 距底 ≤ 16px 兜底 + 用户
  // 主动输入打断 四路终止，详 design.md::D-V4。
  /** 距底距离阈值：> 此值时显按钮，与 wasAtBottom 16px 阈值用途不同（后者是自动跟随判定） */
  const JUMP_THRESHOLD = 300;
  /** programmatic-scroll fallback timer：scrollend 不触发的边缘环境兜底 */
  const PROG_SCROLL_FALLBACK_MS = 1500;
  /** 距底兜底：与 wasAtBottom 同阈值，programmatic-scroll 进入此距离即视为完成 */
  const PROG_SCROLL_BOTTOM_GUARD_PX = 16;

  let isFar = $state(false);
  let isProgrammaticScroll = $state(false);
  /**
   * 本轮 jump 完成后是否还需启动 bottom pin 兜底——独立于 `isProgrammaticScroll`：
   * `updateIsFar` 的 ≤16px bottom guard 会提前清 `isProgrammaticScroll` 让按钮立即
   * 消失，但 lazy markdown reveal 在那之后仍可能让 `scrollHeight` 继续增长。用单独
   * 标志避免 false-negative（codex 二审 PR #250 round 1 命中点）。
   */
  let pendingBottomPinAfterJump = $state(false);
  let progScrollTimer: ReturnType<typeof setTimeout> | null = null;
  let rAFid: number | null = null;

  // ── 滚动状态：锚点法（change `tab-scroll-restore-anchor`） ──
  //
  // 算法实现在 `lib/scrollAnchor.ts`，本组件持有「最后捕获的锚点快照」+
  // 「bottom pin 状态机的 cleanup 引用」两份组件级状态。
  //
  // `latestAnchor` 由 scroll listener 同步维护——element detach 后 onDestroy
  // 仍能读到切走前最后一帧的快照（rect/scrollTop 在 detached 状态不可靠）。
  let latestAnchor: ScrollAnchorState = {
    atBottom: false,
    anchorChunkId: null,
    anchorOffsetPx: 0,
  };
  /** bottom pin 状态机的 cleanup 引用——新一轮启动前 / onDestroy 都强制收敛 */
  let currentBottomPinCleanup: (() => void) | null = null;

  function startProgrammaticScroll() {
    isProgrammaticScroll = true;
    pendingBottomPinAfterJump = true;
    if (progScrollTimer !== null) clearTimeout(progScrollTimer);
    progScrollTimer = setTimeout(progScrollFallback, PROG_SCROLL_FALLBACK_MS);
  }

  function stopProgrammaticScroll() {
    isProgrammaticScroll = false;
    if (progScrollTimer !== null) {
      clearTimeout(progScrollTimer);
      progScrollTimer = null;
    }
  }

  /** scrollend 没触发的边缘环境兜底：仍尝试启动 pin，避免遗漏 reveal 增长 */
  function progScrollFallback() {
    stopProgrammaticScroll();
    triggerBottomPinAfterJump();
  }

  /**
   * 启动 bottom pin 兜底：处理 smooth scroll 锁定旧 scrollHeight 为目标 + 期间
   * lazy markdown reveal 让 scrollHeight 增长导致的"按钮重显 → 用户再点"循环。
   *
   * 视觉过渡：用二次 smooth `scrollTo` 顺滑覆盖 reveal 后的剩余距离；配合
   * `skipInitialJump: true` 让 MO 仅监听后续 mutation，避免 startBottomPin
   * 首行 hard set 取消 smooth animation 形成视觉瞬跳。
   * 二次 scroll 期间用 `isProgrammaticScroll=true` 抑制按钮重显，但**不**再设
   * `pendingBottomPinAfterJump` 防止 onScrollEnd 递归启动新一轮 pin。
   *
   * 不在 `isAtBottom(el)` 早退：bottom guard 触发后 scrollend 可能在 reveal 实际
   * 发生之前 fire（lazyMarkdown IntersectionObserver 异步），此时虽已到旧底但
   * scrollHeight 在 200ms 内仍可能跳变。始终启动 pin 让 MO 监听后续 mutation。
   */
  function triggerBottomPinAfterJump() {
    if (!pendingBottomPinAfterJump) return;
    pendingBottomPinAfterJump = false;
    if (!conversationEl) return;
    // 抑制二次 smooth 期间的按钮重显；不复用 startProgrammaticScroll 避免重置 pending flag
    isProgrammaticScroll = true;
    if (progScrollTimer !== null) clearTimeout(progScrollTimer);
    progScrollTimer = setTimeout(stopProgrammaticScroll, PROG_SCROLL_FALLBACK_MS);
    conversationEl.scrollTo({
      top: conversationEl.scrollHeight,
      behavior: "smooth",
    });
    currentBottomPinCleanup?.();
    currentBottomPinCleanup = startBottomPin(conversationEl, { skipInitialJump: true });
  }

  function scrollToLatest() {
    if (!conversationEl) return;
    const reduceMotion = typeof matchMedia !== "undefined"
      && matchMedia("(prefers-reduced-motion: reduce)").matches;
    startProgrammaticScroll();
    conversationEl.scrollTo({
      top: conversationEl.scrollHeight,
      behavior: reduceMotion ? "auto" : "smooth",
    });
    // reduced-motion 走 'auto' 立即到位 → 不会触发 scrollend，立即清 flag
    if (reduceMotion) {
      // 用 microtask 让浏览器先 commit scrollTop 更新再清 flag，避免 isFar derived 在
      // 同一 task 内还读到旧值
      queueMicrotask(stopProgrammaticScroll);
      // 'auto' 已同步落地 → skip initial hard jump（视觉冗余），仅启动 MO 监听后续
      // reveal 期间的高度增长。直接消费这一轮的 pending flag，避免 onScrollEnd / fallback
      // 重复触发
      pendingBottomPinAfterJump = false;
      currentBottomPinCleanup?.();
      currentBottomPinCleanup = startBottomPin(conversationEl, { skipInitialJump: true });
    }
    // smooth 路径：scrollend / progScrollFallback 调 triggerBottomPinAfterJump 启动 pin
  }

  function updateIsFar() {
    rAFid = null;
    if (!conversationEl) {
      isFar = false;
      return;
    }
    const { scrollTop, scrollHeight, clientHeight } = conversationEl;
    const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
    // programmatic-scroll 期间走两件事：
    // 1. 距底 ≤ 16px 时立即 stop，让 isFar derived 重新生效（提前结束）
    // 2. 否则保持当前 isFar，不让按钮在滚动半路重显
    if (isProgrammaticScroll) {
      if (distanceFromBottom <= PROG_SCROLL_BOTTOM_GUARD_PX) {
        stopProgrammaticScroll();
        isFar = false;
        // 同步启动 pin 监听后续 reveal——bottom guard 触发后 onScrollEnd 才启动 pin
        // 是 race-prone（reveal mutation 可能在 startBottomPin subscribe 前已发生），
        // 这里提前启动让 MO 在 200ms 稳定窗口内捕获 mutation。`triggerBottomPinAfterJump`
        // 内 `pendingBottomPinAfterJump = false` 保证只启动一次（onScrollEnd 调时 early return）
        triggerBottomPinAfterJump();
      }
      return;
    }
    isFar = distanceFromBottom > JUMP_THRESHOLD;
  }

  function scheduleUpdateIsFar() {
    if (rAFid !== null) return;
    rAFid = requestAnimationFrame(updateIsFar);
  }

  function attachConversationScroll(el: HTMLElement) {
    // bind:this 已经把 conversationEl 设上，attach 仅负责挂 listener + cleanup
    const onScroll = () => {
      if (conversationEl && isScrollCompensating(conversationEl)) return;
      latestAnchor = captureScrollAnchor(conversationEl);
      scheduleUpdateIsFar();
    };
    const onScrollEnd = () => {
      stopProgrammaticScroll();
      // smooth 自然完成 → 启动 pin 兜底处理 lazy markdown reveal 期间让
      // scrollHeight 增长导致落点偏离实际新 bottom 的"点多次"bug。
      // pendingBottomPinAfterJump 独立于 isProgrammaticScroll，bottom guard
      // (updateIsFar :≤16) 提前清 isProgrammaticScroll 不影响这里
      triggerBottomPinAfterJump();
    };
    const onUserInput = () => {
      // 用户主动 wheel / touchmove 打断 smooth scroll → 立即清抑制 + 不启动 pin
      if (isProgrammaticScroll) stopProgrammaticScroll();
      pendingBottomPinAfterJump = false;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    el.addEventListener("scrollend", onScrollEnd);
    el.addEventListener("wheel", onUserInput, { passive: true });
    el.addEventListener("touchmove", onUserInput, { passive: true });
    // 初次同步一次（首屏 long session 直接 scrollTop=0 + scrollHeight 巨大 → 立即显示）
    updateIsFar();
    return () => {
      el.removeEventListener("scroll", onScroll);
      el.removeEventListener("scrollend", onScrollEnd);
      el.removeEventListener("wheel", onUserInput);
      el.removeEventListener("touchmove", onUserInput);
      if (rAFid !== null) {
        cancelAnimationFrame(rAFid);
        rAFid = null;
      }
      if (progScrollTimer !== null) {
        clearTimeout(progScrollTimer);
        progScrollTimer = null;
      }
    };
  }

  // ── 仅保留：programmatic-scroll 中断 ──
  // Cmd+F / 跳到最新消息 / 多 pane 守卫 已迁出到 keyboard registry（PaneContainer
  // 的 shared dispatcher 走 `getActiveTabId()` 派发到本 tab 的回调，详 design.md::D8
  // 与 lib/keyboard/session-detail-handlers.ts）。本 listener 只负责一件事：
  //
  // 用户在 smooth-scroll 进行中按下"非本快捷键"按键 → 立即清 isProgrammaticScroll
  // 让按钮可重新显隐（spec scenario "用户主动 wheel/touchmove/非本快捷键 keydown 立即清 flag"）。
  //
  // **判定**：registry 命中 jump-to-latest 的 keydown 会在 dispatcher 内 preventDefault
  // → `e.defaultPrevented === true`；其它键（含用户自定义重绑后的别的 binding）不在此
  // 列。该信号比"自定义实现 isJumpToLatestKey 重判一遍"更鲁棒——用户改键后无需同步
  // 此处逻辑，事件已被 dispatcher 标记。
  //
  // dispatcher 走 bubble phase（capture: false）；本 listener 不指定 capture 也走
  // bubble，二者按注册先后顺序执行——dispatcher 先于本 handler 命中（App.svelte mount
  // 时已注册），preventDefault 已落，`e.defaultPrevented` 在本 handler 可读。
  function handleKeydown(e: KeyboardEvent) {
    if (isProgrammaticScroll && !e.defaultPrevented) {
      stopProgrammaticScroll();
      pendingBottomPinAfterJump = false;  // 用户其它键打断 → 不启动 pin
    }
  }

  const jumpTooltip = $derived(isMac() ? "跳到最新消息 (⌘↓)" : "跳到最新消息 (Ctrl+End)");

  function getAdaptiveDebounceMs(): number {
    const chunks = detail?.chunks.length ?? 0;
    if (chunks < 50) return 150;
    if (chunks < 200) return 300;
    if (chunks < 500) return 500;
    return 1000;
  }

  const fileChangeKey = `session-detail-${untrack(() => tabId)}`;

  async function refreshDetail() {
    const wasAtBottom = !!conversationEl && isAtBottom(conversationEl);
    try {
      // 后端 fingerprint 已编入 is_stale（mtime 距今 ≥5min）。写入停止后若有
      // file-change 事件触发本函数，fingerprint 变化会打破短路完成翻转；若无事件
      // 则依赖用户切走再切回（既有行为，非本次改动引入）。
      const fpToSend = knownFingerprint;
      const resp: SessionDetailResponse = await getSessionDetail(projectId, sessionId, fpToSend);
      const currentSid = getTabSessionId(tabId);
      if (currentSid !== null && currentSid !== sessionId) return;
      knownFingerprint = resp.fingerprint;
      if (resp.status === "unchanged") {
        return;
      }
      const d = resp.detail!;
      // Step 2: chunks fingerprint 二次短路——后端 fingerprint 不匹配（文件 stat 变了）
      // 但 chunks 内容实质未变时（如仅 mtime touch 无追加），跳过 store 更新 + Svelte reconcile。
      const cfp = computeChunksFingerprint(d);
      if (cfp === lastChunksFingerprint) {
        return;
      }
      lastChunksFingerprint = cfp;
      detail = d;
      setCachedSession(tabId, d);
      searchContentVersion++;
      if (wasAtBottom) {
        await tick();
        if (conversationEl) {
          conversationEl.scrollTop = conversationEl.scrollHeight;
        }
      }
    } catch (e) {
      console.warn("auto refresh getSessionDetail failed:", e);
    }
  }

  function computeChunksFingerprint(d: SessionDetail): string {
    const chunks = d.chunks;
    const lastChunk = chunks.length > 0 ? chunks[chunks.length - 1] : null;
    const lastMsgCount = lastChunk?.kind === "ai"
      ? (lastChunk as AIChunk).responses.length
      : 0;
    return `${chunks.length}:${lastMsgCount}:${d.isOngoing}:${d.metrics.message_count}:${d.title ?? ""}`;
  }

  onMount(async () => {
    document.addEventListener("keydown", handleKeydown);
    // 注册本 tab 的 SessionDetail 回调到共享派发表——PaneContainer 的 dispatcher
    // 命中 `session.jump-to-latest` / `search.in-session` 时按 active tabId 路由
    // 到这里。registry → trigger → 本 closure 回调（详 design.md::D8 与
    // session-detail-handlers.ts）。
    registerSessionDetailHandlers(tabId, {
      jumpToLatest: scrollToLatest,
      openSearch: () => {
        searchVisible = true;
      },
    });

    // 性能探针：拆 IPC / DOM-mount / mermaid 三段。仅首次（无缓存）首屏采样。
    // 走 console，便于在 Tauri devtools 里直接看；不接入正式 telemetry。
    const t_mount = performance.now();

    // 优先从 tabStore 缓存加载 session 数据
    const cached = getCachedSession(tabId);
    if (cached) {
      detail = cached;
      lastChunksFingerprint = computeChunksFingerprint(cached);
      loading = false;
      console.info(`[perf] SessionDetail ${sessionId.slice(0, 8)} cached hit`);
      // 切走再切回时 file-change handler 已 unmount，期间发生的文件追加事件
      // 全部错过；cache 直接渲染会停留在"切走那一刻"的旧快照（典型表现：
      // 还在跑的 Bash 完成后切回仍只看到输入；resume 老 session 后追加的
      // 内容看不到）。无差别后台静默拉一次最新 detail——chunk 用稳定 key，
      // 替换不会引起整列表 DOM 重建；后台 IPC 不阻塞 UI，CPU 开销可控。
      // **使用与 file-change handler 同 key 的 scheduleRefresh**：复用
      // fileChangeStore 的 in-flight 去重 + leading/trailing 节流，避免本路
      // background refresh 与紧随而至的 file-change refresh 并发触发两次
      // IPC，旧返回覆盖新 detail（codex review 找到的 bug）。
      scheduleRefresh(`detail:${projectId}|${sessionId}`, refreshDetail);
      // 等 Svelte commit `{:else if detail}` 分支，让 .conversation 真正
      // mount + bind:this 把 conversationEl 接上；否则下面 scrollTop 恢复
      // 条件 (conversationEl && ...) 在 cached hit 路径下静默失败，违反
      // spec `tab-management::滚动位置恢复` Scenario。
      await tick();
    } else {
      try {
        const t_ipc = performance.now();
        const resp = await getSessionDetail(projectId, sessionId, null);
        const currentSid = getTabSessionId(tabId);
        if (currentSid !== null && currentSid !== sessionId) return;
        const ipc_ms = performance.now() - t_ipc;
        knownFingerprint = resp.fingerprint;
        const d = resp.detail!;
        const chunks_len = d.chunks.length;
        const payload_kb = JSON.stringify(d).length / 1024;
        detail = d;
        lastChunksFingerprint = computeChunksFingerprint(d);
        setCachedSession(tabId, d);
        console.info(
          `[perf] SessionDetail ${sessionId.slice(0, 8)} IPC ${ipc_ms.toFixed(0)}ms (chunks=${chunks_len}, payload=${payload_kb.toFixed(0)}KB)`
        );
      } catch (e) { error = String(e); }
      finally { loading = false; }

      // 等 DOM 真正 mount 完
      await tick();
      const total_ms = performance.now() - t_mount;
      console.info(`[perf] SessionDetail ${sessionId.slice(0, 8)} first-paint ${total_ms.toFixed(0)}ms`);
    }

    // 恢复滚动位置（cached path 在前面已 await tick；非 cached path 也已 await tick）
    // —— 锚点法替代旧 scrollTop 数值方案。详 design.md::D2 / D3。
    if (conversationEl) {
      const initialState: ScrollAnchorState = {
        atBottom: uiState.atBottom,
        anchorChunkId: uiState.anchorChunkId,
        anchorOffsetPx: uiState.anchorOffsetPx,
      };
      // 强制收敛上一轮可能残留的 bottom pin（理论 onDestroy 已清，跨 mount 防御）
      currentBottomPinCleanup?.();
      currentBottomPinCleanup = restoreScrollAnchor(conversationEl, initialState);
      // 同步初始化 latestAnchor——避免恢复后用户没 scroll 时 onDestroy
      // 用初始空值覆盖刚恢复的锚点（与 PR #223 旧 latestScrollTop 同步原则）
      latestAnchor = initialState;
    }

    // 注册 file-change handler：命中当前 (projectId, sessionId) 时合并刷新
    registerHandler(fileChangeKey, (payload) => {
      if (payload.projectId !== projectId || payload.sessionId !== sessionId) return;
      scheduleRefresh(`detail:${projectId}|${sessionId}`, refreshDetail, getAdaptiveDebounceMs());
    });

    // 消费 deeplink 触发的 pendingScrollChunkId（spec session-display
    // "pendingScrollChunkId 绑定 tab lifecycle 消费一次"）。
    //
    // 已满足三条件：
    // (a) tab focused —— SessionDetail 仅在 tab active 时 mount（PaneView {#key}）
    // (b) SessionDetail mount —— 走到此处即满足
    // (c) chunks 加载完成 —— cached 路径直接命中；非 cached 路径上面 await 了 IPC
    //
    // 找不到 chunk 时弹 toast + clear 避免后续重试。
    if (uiState.pendingScrollChunkId && detail) {
      const target = uiState.pendingScrollChunkId;
      uiState.pendingScrollChunkId = null;
      saveTabUIState(tabId, uiState);
      const exists = detail.chunks.some((c) => c.chunkId === target);
      if (exists) {
        // tick 让 chunk DOM 完成首屏 commit 再 scroll（cached 路径下 detail 已就位
        // 但 anchor restore 刚跑完，再让 scrollAnchor 抢先一帧可能与 deeplink 冲突）
        await tick();
        await handleNavigateToChunk(target);
      } else {
        // 容忍：spec "目标 chunk 不存在时弹 toast"
        // 本仓暂无统一 toast 组件——降级到 console.warn + 不阻塞流程。
        // 后续 follow-up 接入 toast 系统时替换此处（见 D9 风险段）。
        console.warn(`[deeplink] target chunk not found in this session: ${target}`);
      }
    }
  });

  // Mermaid 图表后处理：旧版本在首屏 effect 全树扫描；现在迁移到
  // lazy markdown observer 的 onRendered 回调内（按 chunk 子树扫描），
  // 见 attachMarkdown 与 design.md decision 3。

  onDestroy(() => {
    document.removeEventListener("keydown", handleKeydown);
    unregisterSessionDetailHandlers(tabId);
    unregisterHandler(fileChangeKey);
    cancelScheduledRefresh(`detail:${projectId}|${sessionId}`);
    lazyObserver?.disconnect();
    lazyObserver = null;
    // 强制收敛 bottom pin 状态机——element unmount 后 MutationObserver / timer
    // 若没 disconnect 会持续持有 detached element 引用 + 触发回调写入 detached
    // scrollTop（无效但浪费）
    currentBottomPinCleanup?.();
    currentBottomPinCleanup = null;
    // 保存 per-tab UI 状态 —— 但仅在 tab 仍指向当前 sessionId 时保存。
    // openOrReplaceTab 会保留 tabId 仅换 sessionId 触发 destroy/recreate；
    // 若此处无条件 save，旧 session 的状态会覆盖 openOrReplaceTab 刚清掉的 slot，
    // 新 session mount 时 getTabUIState(tabId) 拿到的就是旧 session 残留（codex 二审 #1）。
    //
    // 滚动状态用 `latestAnchor` 而非 `captureScrollAnchor()` 当场读：Svelte 5
    // onDestroy 在 element unmount **之后**触发，conversationEl 仍指向原 ref 但
    // `.isConnected=false`，rect / scrollTop / scrollHeight 都不可靠。
    // `latestAnchor` 由 scroll listener 在每次滚动时同步写入，element 离开 DOM
    // 前的最后一帧锚点已被捕获，不依赖 onDestroy 那一刻的读取。
    if (getTabSessionId(tabId) === sessionId) {
      saveTabUIState(tabId, {
        expandedChunks: new Set(expandedChunks),
        expandedItems: new Set(expandedItems),
        searchVisible,
        contextPanelVisible,
        atBottom: latestAnchor.atBottom,
        anchorChunkId: latestAnchor.anchorChunkId,
        anchorOffsetPx: latestAnchor.anchorOffsetPx,
        // pendingScrollChunkId 由 onMount 消费一次后清；onDestroy 不需要透传——
        // 切走再切回时若仍有 pending（消费失败 / 未激活），从 getTabUIState 读取
        // 重新走消费路径，本地 save 不持有此字段
        pendingScrollChunkId: uiState.pendingScrollChunkId,
      });
    }
  });

  // tool output 懒拉缓存：toolUseId → ToolOutput。仅当 exec.outputOmitted=true
  // 且用户首次展开该 tool 时通过 getToolOutput IPC 拉取。展开后渲染走
  // `effectiveOutput()` —— cache 命中优先，否则用 exec.output（兼容老后端 / 回滚）。
  //
  // LRU 上限：长会话连续展开多 tool 时上限 200，超出按插入顺序（Map 迭代顺序）
  // 淘汰最旧项。命中时把 key 重新 set 到尾部，保持最近使用排序。
  const OUTPUT_CACHE_LIMIT = 200;
  let outputCache: Map<string, ToolOutput> = $state(new Map());
  const outputLoads = new Map<string, Promise<void>>();

  function cachedOutput(exec: ToolExecution): ToolOutput | undefined {
    const cached = outputCache.get(exec.toolUseId);
    return cached?.kind === "missing" ? undefined : cached;
  }

  function isOutputLoading(exec: ToolExecution): boolean {
    return !!exec.outputOmitted && !cachedOutput(exec);
  }

  function isOutputReady(exec: ToolExecution): boolean {
    return !exec.outputOmitted || !!cachedOutput(exec);
  }

  function effectiveExec(exec: ToolExecution): ToolExecution {
    const cached = cachedOutput(exec);
    if (!cached) return exec;
    return { ...exec, output: cached };
  }

  async function ensureToolOutput(exec: ToolExecution): Promise<void> {
    if (!exec.outputOmitted) return;
    const cached = outputCache.get(exec.toolUseId);
    if (cached && cached.kind !== "missing") {
      const next = new Map(outputCache);
      next.delete(exec.toolUseId);
      next.set(exec.toolUseId, cached);
      outputCache = next;
      return;
    }
    const existing = outputLoads.get(exec.toolUseId);
    if (existing) return existing;
    const load = (async () => {
      try {
        const out = await getToolOutput(sessionId, sessionId, exec.toolUseId);
        if (out.kind === "missing") return;
        const next = new Map(outputCache);
        next.set(exec.toolUseId, out);
        while (next.size > OUTPUT_CACHE_LIMIT) {
          const firstKey = next.keys().next().value;
          if (firstKey === undefined) break;
          next.delete(firstKey);
        }
        outputCache = next;
      } catch (e) {
        console.warn("[perf] getToolOutput failed", exec.toolUseId, e);
      } finally {
        outputLoads.delete(exec.toolUseId);
      }
    })();
    outputLoads.set(exec.toolUseId, load);
    return load;
  }

  function prefetchReadOutputs(chunk: AIChunk): void {
    for (const exec of chunk.toolExecutions) {
      if (shouldPrefetchOnChunkExpand(exec)) {
        void ensureToolOutput(exec);
      }
    }
  }

  // detail 替换后（首次加载 / cache hit / file-change refresh）自动补拉所有
  // 已展开 + outputOmitted 工具的最新 output——典型场景：用户提前展开一个
  // 还在跑的 Bash，切走再切回 / 工具完成后 file-change 推送新 detail，没有
  // 这层 effect 时 expandedItems 已包含 key 但不会再触发 toggle，OutputBlock
  // 会一直显示空。ensureToolOutput 内部判断 cache 命中才走 IPC。
  $effect(() => {
    if (!detail) return;
    const snapshot = detail;
    untrack(() => {
      snapshot.chunks.forEach((chunk, i) => {
        if (chunk.kind !== "ai") return;
        for (const exec of chunk.toolExecutions) {
          if (!exec.outputOmitted) continue;
          const key = `${chunk.chunkId}-tool-${exec.toolUseId}`;
          if (expandedItems.has(key)) {
            void ensureToolOutput(exec);
          }
        }
      });
    });
  });

  async function toggle(key: string, exec?: ToolExecution) {
    if (expandedItems.has(key)) {
      const next = new Set(expandedItems);
      next.delete(key);
      expandedItems = next;
      return;
    }
    if (exec && viewerUsesOutput(exec) && !isOutputReady(exec)) {
      await ensureToolOutput(exec);
      if (!isOutputReady(exec)) return;
    }
    const next = new Set(expandedItems);
    next.add(key);
    expandedItems = next;
  }

  function chunkKey(c: Chunk): string {
    return c.chunkId;
  }

  // ── Context menu ctx 构造（Task 9 / spec frontend-context-menu Phase 2）──
  //
  // 在 oncontextmenu 触发瞬间预读 selectionText 注入 ctx——factory 内部不读 DOM。
  // selection 跨容器时 `window.getSelection()?.toString()` 总能拿到当前选中文本。
  function buildMenuCtx(): MenuItemContext {
    return {
      sessionId,
      projectId,
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }

  // 最后一个 AIChunk 的索引。ongoing=true 时它的 lastOutput 位置被
  // `<OngoingBanner />` 替代；结束后换回真正的内容。对齐原版
  // `LastOutputDisplay.tsx` 的 `isLastGroup && isSessionOngoing` 语义——
  // banner 占 lastOutput 坑位，不作为独立节点追加到对话流尾部，从而避免
  // ongoing 切换时 scrollHeight 跳变引起的闪烁。
  // ContextPanel 的徽标 / Header count 用"Latest 视图"injection 数；Phase Selector
  // 切到旧 phase 是 panel 内部状态，不影响顶栏徽标。
  const contextInjectionsLatest = $derived(
    detail ? selectActivePhaseInjections(detail, null) : [],
  );
  const contextCount = $derived(contextInjectionsLatest.length);

  // Context Panel → SessionDetail 锚点跳转 helpers。
  // spec: session-display "Context Panel turn 锚点导航"。
  async function showAnchorHighlight(chunkId: string, toolUseId: string | null = null) {
    if (highlightTimer) clearTimeout(highlightTimer);
    highlightedChunkId = null;
    highlightedToolUseId = null;
    await tick();
    highlightedChunkId = chunkId;
    highlightedToolUseId = toolUseId;
    highlightTimer = setTimeout(() => {
      highlightedChunkId = null;
      highlightedToolUseId = null;
      highlightTimer = null;
    }, 2200);
  }

  function scrollAnchorIntoView(target: HTMLElement | null | undefined) {
    if (!target || !conversationEl) return;
    const containerRect = conversationEl.getBoundingClientRect();
    const targetRect = target.getBoundingClientRect();
    const targetCenter = targetRect.top - containerRect.top + conversationEl.scrollTop + targetRect.height / 2;
    const nextTop = Math.max(0, targetCenter - conversationEl.clientHeight * 0.45);
    conversationEl.scrollTo({ top: nextTop, behavior: "smooth" });
  }

  async function handleNavigateToChunk(chunkId: string) {
    if (!expandedChunks.has(chunkId)) {
      expandedChunks = new Set([...expandedChunks, chunkId]);
    }
    await tick();
    const chunkEl = conversationEl?.querySelector<HTMLElement>(
      `[data-chunk-id="${cssEscape(chunkId)}"]`,
    );
    scrollAnchorIntoView(chunkEl);
    void showAnchorHighlight(chunkId);
  }

  async function handleNavigateToTool(chunkId: string, toolUseId: string) {
    if (!expandedChunks.has(chunkId)) {
      expandedChunks = new Set([...expandedChunks, chunkId]);
    }
    await tick();
    await tick();
    const chunkEl = conversationEl?.querySelector<HTMLElement>(
      `[data-chunk-id="${cssEscape(chunkId)}"]`,
    );
    const toolEl = chunkEl?.querySelector<HTMLElement>(
      `[data-tool-use-id="${cssEscape(toolUseId)}"]`,
    );
    scrollAnchorIntoView(toolEl ?? chunkEl);
    void showAnchorHighlight(chunkId, toolEl ? toolUseId : null);
  }

  function handleNavigateToUserGroup(aiGroupId: string) {
    if (!detail) return;
    const aiIdx = detail.chunks.findIndex((c) => c.chunkId === aiGroupId);
    if (aiIdx < 0) {
      // 找不到对应 AIChunk，无法定位
      return;
    }
    // 向前找紧邻的 UserChunk
    for (let i = aiIdx - 1; i >= 0; i--) {
      if (detail.chunks[i].kind === "user") {
        void handleNavigateToChunk(detail.chunks[i].chunkId);
        return;
      }
    }
    // fallback：滚到 AIChunk 本身
    void handleNavigateToChunk(aiGroupId);
  }

  /** 简化 CSS.escape：转义 querySelector 用的 `"` 与 `\`。chunkId / toolUseId 实际只
   *  含字母数字 + `:` + `-`，不会有这些字符，但加 guard 以防上游 uuid 含特殊符号。 */
  function cssEscape(s: string): string {
    return s.replace(/["\\]/g, (m) => "\\" + m);
  }

  const lastAiIndex = $derived.by(() => {
    if (!detail) return -1;
    for (let i = detail.chunks.length - 1; i >= 0; i--) {
      if (detail.chunks[i].kind === "ai") return i;
    }
    return -1;
  });

  function sumMetrics(chunks: Chunk[]): ChunkMetrics {
    const r: ChunkMetrics = { inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0, cacheReadTokens: 0, toolCount: 0, costUsd: null };
    for (const c of chunks) {
      r.inputTokens += c.metrics.inputTokens;
      r.outputTokens += c.metrics.outputTokens;
      r.toolCount += c.metrics.toolCount;
    }
    return r;
  }

  function countByKind(chunks: Chunk[]): { ai: number; user: number } {
    let ai = 0;
    let user = 0;
    for (const c of chunks) {
      if (c.kind === "ai") ai++;
      else if (c.kind === "user") user++;
    }
    return { ai, user };
  }

  function lastActivityTs(chunks: Chunk[]): string | null {
    for (let i = chunks.length - 1; i >= 0; i--) {
      const t = chunks[i].timestamp;
      if (t) return t;
    }
    return null;
  }

  function fk(n: number): string {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }

  function ftime(ts: string): string {
    return formatClock(ts, getTimeFormat() === "12h");
  }

  /** 分钟级精度，用于顶 bar `LAST` 与 sidebar 时间密度对齐（无秒） */
  function ftimeMinutes(ts: string): string {
    try {
      const d = new Date(ts);
      if (Number.isNaN(d.getTime())) return "";
      return d.toLocaleTimeString("zh-CN", {
        hour: "2-digit",
        minute: "2-digit",
        hour12: getTimeFormat() === "12h",
      });
    } catch {
      return "";
    }
  }

  function fduration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    const s = ms / 1000;
    if (s < 60) return `${s.toFixed(1)}s`;
    const m = Math.floor(s / 60);
    const rs = Math.floor(s % 60);
    return `${m}m ${rs}s`;
  }

  function utext(content: string | unknown[]): string {
    let raw = "";
    if (typeof content === "string") {
      raw = content;
    } else if (Array.isArray(content)) {
      for (const b of content) {
        if (b && typeof b === "object" && "type" in b) {
          const x = b as Record<string, unknown>;
          if (x.type === "text" && typeof x.text === "string") { raw = x.text; break; }
        }
      }
    }
    return cleanDisplayText(raw);
  }

  /** 抽出 user content blocks 内的所有 image，附带稳定 blockId（chunkUuid:blockIndex）。*/
  function uimages(content: string | unknown[], chunkUuid: string): { source: import("../lib/api").ImageSource; blockId: string }[] {
    if (!Array.isArray(content)) return [];
    const out: { source: import("../lib/api").ImageSource; blockId: string }[] = [];
    content.forEach((b, idx) => {
      if (b && typeof b === "object" && "type" in b) {
        const x = b as Record<string, unknown>;
        if (x.type === "image" && x.source && typeof x.source === "object") {
          out.push({
            source: x.source as import("../lib/api").ImageSource,
            blockId: `${chunkUuid}:${idx}`,
          });
        }
      }
    });
    return out;
  }

  function aiModel(chunk: AIChunk): string {
    if (chunk.responses.length > 0) {
      const m = chunk.responses[chunk.responses.length - 1].model;
      if (m) return m.replace("claude-", "").replace(/-\d{8}$/, "");
    }
    return "Claude";
  }

  function isReadTool(exec: ToolExecution): boolean { return exec.toolName === "Read" && !exec.isError; }
  function isEditTool(exec: ToolExecution): boolean { return exec.toolName === "Edit"; }
  function isWriteTool(exec: ToolExecution): boolean { return exec.toolName === "Write" && !exec.isError; }
  function isBashTool(exec: ToolExecution): boolean { return ["Bash", "bash"].includes(exec.toolName); }

  /**
   * 会话标题：直接消费 backend `extract_session_metadata_from_parsed` 派生（与
   * sidebar `SessionSummary.title` 共用单一真相源），`null/undefined` 时 fallback
   * 到 `sessionId.slice(0, 8)` 与 sidebar 一致。
   * Spec：`ipc-data-api::SessionDetail 暴露与 SessionSummary 同源派生的 title`。
   */
  function detailTitle(d: SessionDetail | null): string {
    if (d?.title) return d.title;
    return sessionId.slice(0, 8);
  }
</script>

<div class="session-detail">
{#if loading}
  <SessionDetailSkeleton />
{:else if error}
  <div class="state-msg state-err">{error}</div>
{:else if detail}
  {@const m = sumMetrics(detail.chunks)}
  {@const counts = countByKind(detail.chunks)}
  {@const lastActivity = lastActivityTs(detail.chunks)}
  {@const totalTokens = m.inputTokens + m.outputTokens}
  {@const metaCwdRaw = detail.metadata && typeof detail.metadata === "object" ? (detail.metadata as { cwd?: unknown }).cwd : undefined}
  {@const metaCwd = typeof metaCwdRaw === "string" && metaCwdRaw.length > 0 ? metaCwdRaw : undefined}

  <!-- Top bar：18px 标题 + 副标题密度行（chunks · tools · tokens · last activity） -->
  <div class="top-bar">
    <div class="top-titles">
      <h1 class="top-title">{detailTitle(detail)}</h1>
      <div class="top-stats" aria-label="Session statistics">
        <span class="top-stat">
          <span class="top-stat-num">{counts.ai}</span>
          <span class="top-stat-unit">AI</span>
        </span>
        <span class="top-stat-sep">·</span>
        <span class="top-stat">
          <span class="top-stat-num">{counts.user}</span>
          <span class="top-stat-unit">USER</span>
        </span>
        <span class="top-stat-sep">·</span>
        <span class="top-stat">
          <span class="top-stat-num">{m.toolCount}</span>
          <span class="top-stat-unit">TOOLS</span>
        </span>
        {#if totalTokens > 0}
          <span class="top-stat-sep">·</span>
          <span class="top-stat">
            <span class="top-stat-num">{fk(totalTokens)}</span>
            <span class="top-stat-unit">TOK</span>
          </span>
        {/if}
        {#if lastActivity}
          <span class="top-stat-sep">·</span>
          <span class="top-stat top-stat-time">
            <span class="top-stat-unit">LAST</span>
            <span class="top-stat-num">{ftimeMinutes(lastActivity)}</span>
          </span>
        {/if}
      </div>
    </div>
    <div class="top-meta">
      <SessionMetaMenu cwd={metaCwd} sessionId={sessionId} />
      {#if contextCount > 0}
        <button
          type="button"
          class="top-badge"
          class:top-badge-active={contextPanelVisible}
          onclick={() => contextPanelVisible = !contextPanelVisible}
          aria-pressed={contextPanelVisible}
        >
          <svg class="top-badge-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M14 3v4a1 1 0 0 0 1 1h4" />
            <path d="M17 21H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7l5 5v11a2 2 0 0 1-2 2z" />
          </svg>
          <span>Context</span>
          <span class="top-badge-count">{contextCount}</span>
        </button>
      {/if}
    </div>
  </div>

  <!-- Search bar -->
  <SearchBar
    visible={searchVisible}
    containerEl={conversationEl ?? null}
    onClose={() => searchVisible = false}
    onBeforeSearch={() => lazyObserver?.flushAll()}
    contentVersion={searchContentVersion}
  />

  <!-- Content area (conversation + optional context panel) -->
  <div class="content-area">
  <!-- Conversation -->
  <div class="conversation" bind:this={conversationEl} {@attach attachConversationScroll}>
    {#each detail.chunks as chunk, i (chunkKey(chunk))}

      <!-- User -->
      {#if chunk.kind === "user"}
        {@const text = utext(chunk.content)}
        {@const images = uimages(chunk.content, chunk.uuid)}
        {@const taskNotifications = parseTaskNotifications(chunk.content)}
        {#if text || images.length > 0 || taskNotifications.length > 0}
          <div
            class="msg-row msg-row-user"
            class:msg-row-anchor-hit={highlightedChunkId === chunk.chunkId}
            data-chunk-id={chunk.chunkId}
            use:contextMenu={() => buildUserMessageItems(chunk, buildMenuCtx())}
          >
            <div class="msg-spacer"></div>
            <div class="user-stack">
              <!-- meta row 外置在 bubble 上方，右边缘紧贴 conversation 内右
                   padding（= AI .ai-header-row 内 time 的同一 x 列）。
                   "YOU"在前 / time 在后，让 time 真正落在最右、形成"时间戳列"。 -->
              <div class="user-meta">
                <span class="user-meta-name">You</span>
                <span class="user-meta-sep">·</span>
                <span class="user-meta-time">{ftime(chunk.timestamp)}</span>
              </div>
              <div class="user-row">
                <div class="msg-bubble msg-bubble-user">
                  {#if text}
                    <div class="prose lazy-md" {@attach attachMarkdown(text, "user")}></div>
                  {/if}
                {#each images as img (img.blockId)}
                  <ImageBlock
                    source={img.source}
                    rootSessionId={sessionId}
                    sessionId={sessionId}
                    blockId={img.blockId}
                  />
                {/each}
                {#each taskNotifications as notif (notif.taskId)}
                  {@const isFailed = notif.status === "failed" || notif.status === "error"}
                  {@const isCompleted = notif.status === "completed"}
                  {@const cmdMatch = /"([^"]+)"/.exec(notif.summary)}
                  {@const cmdName = cmdMatch?.[1] ?? notif.summary}
                  {@const exitMatch = /\(exit code (\d+)\)/.exec(notif.summary)}
                  {@const exitCode = exitMatch?.[1]}
                  {@const fileBase = notif.outputFile.split("/").pop() ?? ""}
                  <div
                    class="task-notif"
                    class:task-notif-done={isCompleted}
                    class:task-notif-fail={isFailed}
                  >
                    <span class="task-notif-icon" aria-hidden="true">
                      {#if isFailed}✕{:else if isCompleted}✓{:else}○{/if}
                    </span>
                    <div class="task-notif-body">
                      <div class="task-notif-name">{cmdName || notif.taskId}</div>
                      <div class="task-notif-meta">
                        <span class="task-notif-status">{notif.status}</span>
                        {#if exitCode != null}
                          <span>exit {exitCode}</span>
                        {/if}
                        {#if fileBase}
                          <span class="task-notif-file">{fileBase}</span>
                        {/if}
                      </div>
                    </div>
                  </div>
                  {/each}
                </div>
                <span class="user-avatar" aria-hidden="true">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html USER_SVG}</svg>
                </span>
              </div>
            </div>
          </div>
        {/if}

      <!-- AI -->
      {:else if chunk.kind === "ai"}
        {@const di = buildDisplayItemsCached(chunk)}
        {@const summaryText = buildSummary(di.items, workflowRunIds)}
        {@const toolsVisible = isChunkToolsVisible(chunk)}
        {@const interruptions = chunk.semanticSteps.filter((s) => s.kind === "interruption")}
        {@const isLastAi = i === lastAiIndex}
        {@const isLiveTail = isLastAi && detail.isOngoing}
        {@const lastOutputText = cleanDisplayText(di.lastOutput?.text ?? "")}
        <!--
          对齐原版 AIChatGroup.tsx:234-248 "Get the LAST assistant message's
          usage (represents current context window snapshot)"——Anthropic API
          的 usage 每次 call 都返回整段历史的 cache 大小，累加会重复计数；
          取最后一条 response.usage 即为该 AI turn 结束时的 context snapshot。
        -->
        {@const lastUsage = [...chunk.responses].reverse().find((r) => r.usage)?.usage ?? null}
        {@const headerInputTokens = lastUsage?.input_tokens ?? 0}
        {@const headerOutputTokens = lastUsage?.output_tokens ?? 0}
        {@const headerCacheRead = lastUsage?.cache_read_input_tokens ?? 0}
        {@const headerCacheCreation = lastUsage?.cache_creation_input_tokens ?? 0}
        {@const aiTotalTokens = headerInputTokens + headerOutputTokens + headerCacheRead + headerCacheCreation}
        {@const chunkTurnStats = getPerTurnStats(detail?.turnContextStats, chunk.chunkId)}
        <!-- hasAiContent 兜底：token-only chunk（context snapshot 有价值）也要保 header，
             不能让"无 items + 无 lastOutput + 无 interruption"的 chunk 把 token 数据丢掉。
             修 codex CR Bug 2（PR #126 r1）。 -->
        {@const hasAiContent =
          di.items.length > 0 ||
          lastOutputText !== "" ||
          interruptions.length > 0 ||
          isLiveTail ||
          aiTotalTokens > 0 ||
          (chunkTurnStats != null && chunkTurnStats.newCount > 0)}
        {#if hasAiContent}
        <div
          class="msg-row msg-row-ai"
          class:msg-row-anchor-hit={highlightedChunkId === chunk.chunkId}
          data-chunk-id={chunk.chunkId}
          use:contextMenu={() => buildAssistantMessageItems(chunk, buildMenuCtx())}
        >
          <div
            class="msg-ai-container"
            class:msg-ai-container-live={isLiveTail}
            class:msg-ai-container-tools-open={toolsVisible}
          >
            <span class="ai-thread-node" class:ai-thread-node-live={isLiveTail} aria-hidden="true"></span>
            <!-- AI header -->
            <div class="ai-header-row">
              <span class="ai-avatar" aria-hidden="true">
                <!-- lucide Bot：与原版 AIChatGroup.tsx 行 408 的 <Bot/> 对齐（多 path / rect） -->
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 8V4H8" />
                  <rect width="16" height="12" x="4" y="8" rx="2" />
                  <path d="M2 14h2" />
                  <path d="M20 14h2" />
                  <path d="M15 13v2" />
                  <path d="M9 13v2" />
                </svg>
              </span>
              <span class="ai-label">Claude</span>
              <span class="ai-model">{aiModel(chunk)}</span>
              {#if summaryText}
                <button
                  type="button"
                  class="ai-tool-toggle"
                  onclick={() => toggleChunk(chunk)}
                  aria-expanded={toolsVisible}
                  aria-label={toolsVisible ? "折叠工具调用列表" : "展开工具调用列表"}
                  title={summaryText}
                >
                  <span class="ai-tool-chevron" class:ai-tool-chevron-open={toolsVisible}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT} /></svg>
                  </span>
                  {summaryText}
                </button>
              {/if}
              <span class="ai-header-spacer"></span>
              <ContextBadge
                stats={getPerTurnStats(detail?.turnContextStats, chunk.chunkId)}
                injections={injectionsByTurn.get(chunk.chunkId) ?? []}
                popoverId={`${chunk.chunkId}:context`}
                {openPopoverId}
                onToggle={togglePopover}
              />
              {#if aiTotalTokens > 0}
                <button
                  type="button"
                  class="ai-tokens"
                  class:ai-tokens-active={openPopoverId === `${chunk.chunkId}:tokens`}
                  onclick={(e) => { e.stopPropagation(); togglePopover(`${chunk.chunkId}:tokens`); }}
                  aria-expanded={openPopoverId === `${chunk.chunkId}:tokens`}
                  aria-label="Token usage: {fk(aiTotalTokens)}"
                >
                  <!-- lucide Info -->
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="ai-tokens-info" aria-hidden="true">
                    <circle cx="12" cy="12" r="10" />
                    <path d="M12 16v-4" />
                    <path d="M12 8h.01" />
                  </svg>
                  <span>{fk(aiTotalTokens)}</span>
                  {#if openPopoverId === `${chunk.chunkId}:tokens`}
                    {@const turnStats = getPerTurnStats(detail?.turnContextStats, chunk.chunkId)}
                    {@const sessionContextTokens = detail?.contextInjections ? detail.contextInjections.reduce((s, inj) => s + (inj.estimatedTokens ?? 0), 0) : 0}
                    {@const cumulativeTokens = turnStats?.cumulativeEstimatedTokens ?? sessionContextTokens}
                    {@const visPct = cumulativeTokens > 0 && aiTotalTokens > 0 ? Math.round((cumulativeTokens / aiTotalTokens) * 100) : 0}
                    <span class="ai-tokens-popover" role="dialog" aria-label="Token breakdown">
                      <span class="tok-row tok-row-total"><span>Total</span><span>{aiTotalTokens.toLocaleString()}</span></span>
                      <span class="tok-row"><span>Input</span><span>{headerInputTokens.toLocaleString()}</span></span>
                      <span class="tok-row"><span>Output</span><span>{headerOutputTokens.toLocaleString()}</span></span>
                      <span class="tok-row"><span>Cache create</span><span>{headerCacheCreation.toLocaleString()}</span></span>
                      <span class="tok-row"><span>Cache read</span><span>{headerCacheRead.toLocaleString()}</span></span>
                      {#if visPct > 0}
                        <span class="tok-divider"></span>
                        <span class="tok-row tok-row-visible">
                          <span>Visible Context</span>
                          <span>≈{visPct}%</span>
                        </span>
                      {/if}
                    </span>
                  {/if}
                </button>
              {/if}
              {#if chunk.durationMs != null && chunk.durationMs > 0}
                <span class="ai-duration" title="耗时">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html CLOCK_SVG}</svg>
                  {fduration(chunk.durationMs)}
                </span>
              {/if}
              <span class="ai-time">{ftime(chunk.timestamp)}</span>
            </div>

            <!-- Display items (toggle visibility) -->
            {#if toolsVisible}
              <div class="ai-tools-section">
                {#each di.items as item, di_idx}
                  {#if item.type === "slash"}
                    {@const slashKey = `${chunk.chunkId}-slash-${di_idx}`}
                    {@const hasInstructions = !!item.slash.instructions}
                    <BaseItem
                      svgIcon={SLASH}
                      label={"/" + item.slash.name}
                      summary={item.slash.args ?? item.slash.message ?? ""}
                      tokenCount={hasInstructions ? Math.ceil((item.slash.instructions ?? "").length / 4) : undefined}
                      status={hasInstructions ? "ok" : undefined}
                      isExpanded={hasInstructions && expandedItems.has(slashKey)}
                      onclick={hasInstructions ? () => toggle(slashKey) : () => {}}
                    >
                      {#snippet children()}
                        {#if item.slash.instructions}
                          <div class="prose slash-instructions lazy-md" {@attach attachMarkdown(item.slash.instructions, "slash")}></div>
                        {/if}
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "tool"}
                    {@const exec = item.execution}
                    {@const matchedWf = exec.workflowRunId ? workflowMap.get(exec.workflowRunId) : undefined}
                    {#if matchedWf}
                      <WorkflowCard workflow={matchedWf} {sessionId} {projectId} />
                    {:else}
                      {@const key = `${chunk.chunkId}-tool-${exec.toolUseId}`}
                      {@const eff = effectiveExec(exec)}
                      <div
                        class:tool-anchor-hit={highlightedToolUseId === exec.toolUseId}
                        data-tool-use-id={exec.toolUseId}
                      >
                        <BaseItem
                          svgIcon={WRENCH}
                          label={exec.toolName}
                          summary={getToolSummary(exec.toolName, exec.input)}
                          tokenCount={getToolContextTokens(exec)}
                          status={getToolStatus(exec)}
                          durationMs={getToolDurationMs(exec)}
                          pendingLabel={isToolPending(exec) ? "pending" : undefined}
                          isExpanded={expandedItems.has(key)}
                          onclick={() => toggle(key, exec)}
                        >
                          {#snippet children()}
                            {#if isReadTool(exec)}
                              <ReadToolViewer exec={eff} {sessionId} {projectId} />
                            {:else if isEditTool(exec)}
                              <EditToolViewer exec={eff} {sessionId} {projectId} />
                            {:else if isWriteTool(exec)}
                              <WriteToolViewer exec={eff} {sessionId} {projectId} />
                            {:else if isBashTool(exec)}
                              <BashToolViewer exec={eff} {sessionId} {projectId} />
                            {:else}
                              <DefaultToolViewer exec={eff} />
                            {/if}
                          {/snippet}
                        </BaseItem>
                      </div>
                    {/if}
                  {:else if item.type === "thinking"}
                    {@const key = `${chunk.chunkId}-think-${di_idx}`}
                    <BaseItem
                      svgIcon={BRAIN}
                      label="Thinking"
                      tokenCount={estimateTokens(item.text)}
                      isExpanded={expandedItems.has(key)}
                      onclick={() => toggle(key)}
                    >
                      {#snippet children()}
                        <div class="prose prose-thinking lazy-md" {@attach attachMarkdown(item.text, "thinking")}></div>
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "output"}
                    {@const key = `${chunk.chunkId}-output-${di_idx}`}
                    <BaseItem
                      svgIcon={MESSAGE_SQUARE}
                      label="Output"
                      summary={item.text.length > 60 ? item.text.slice(0, 60) + "…" : item.text}
                      tokenCount={estimateTokens(item.text)}
                      isExpanded={expandedItems.has(key)}
                      onclick={() => toggle(key)}
                    >
                      {#snippet children()}
                        <div class="prose lazy-md" {@attach attachMarkdown(item.text, "output")}></div>
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "user_message"}
                    {@const key = `${chunk.chunkId}-usermsg-${di_idx}`}
                    <BaseItem
                      svgIcon={USER_ICON}
                      label="User"
                      summary={item.text.length > 60 ? item.text.slice(0, 60) + "…" : item.text}
                      isExpanded={expandedItems.has(key)}
                      onclick={() => toggle(key)}
                    >
                      {#snippet children()}
                        <div class="prose lazy-md" {@attach attachMarkdown(item.text, "output")}></div>
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "subagent"}
                    <SubagentCard process={item.process} rootSessionId={sessionId} />
                  {:else if item.type === "teammate_message"}
                    <TeammateMessageItem
                      teammateMessage={item.teammateMessage}
                      attachBody={attachMarkdown(item.teammateMessage.body, "teammate")}
                      rootSessionId={sessionId}
                    />
                  {:else if item.type === "teammate_spawn"}
                    {@const colors = getTeamColorSet(item.color)}
                    <div class="teammate-spawn-row">
                      <span class="teammate-spawn-dot" style:background-color={colors.border}></span>
                      <span
                        class="teammate-spawn-badge"
                        style:background-color={colors.badge}
                        style:color={colors.text}
                        style:border-color="{colors.border}40"
                      >
                        {item.name}
                      </span>
                      <span class="teammate-spawn-label">Teammate spawned</span>
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}

            <!-- Last output (always visible). -->
            <div class="ai-body">
              {#if i === lastAiIndex && detail.isOngoing}
                <!-- 对齐原版 LastOutputDisplay：最后 AI 组在 ongoing 时
                     banner 占 lastOutput 位置，结束后换回真正的内容 -->
                <OngoingBanner />
              {:else if lastOutputText}
                <div class="prose lazy-md" {@attach attachMarkdown(lastOutputText, "ai")}></div>
              {/if}
              {#each interruptions as _interrupt}
                <div class="interruption-block" role="status">
                  <svg class="interruption-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    {@html ALERT_TRIANGLE_SVG}
                  </svg>
                  <span class="interruption-label">INTERRUPTED</span>
                  <span class="interruption-text">Request interrupted by user</span>
                  <span class="interruption-glyph" aria-hidden="true">↩</span>
                </div>
              {/each}
            </div>
          </div>
        </div>
        {/if}

      <!-- System：与 AI 共享同一条 thread rail（3px solid border-emphasis）；
           左外侧 timeline marker 用方形 outline 与 AI 圆形 node 形态对立做语义
           区分；header chip / bubble 起点对齐 conversation 27px 内容基线。 -->
      {:else if chunk.kind === "system"}
        {@const sysText = cleanDisplayText(chunk.contentText)}
        {#if sysText}
          <div
            class="msg-row msg-row-system-left"
            class:msg-row-anchor-hit={highlightedChunkId === chunk.chunkId}
            data-chunk-id={chunk.chunkId}
          >
            <div class="system-block">
              <div class="system-header">
                <svg class="system-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={TERMINAL}/></svg>
                <span class="system-label">System</span>
                <span class="system-meta-sep">·</span>
                <span class="system-time">{ftime(chunk.timestamp)}</span>
              </div>
              <pre class="system-pre">{sysText}</pre>
            </div>
          </div>
        {/if}

      <!-- Compact (对齐原版 CompactBoundary.tsx：折叠头 + ChevronRight + Layers
           + token delta + Phase 徽章 + 时间，amber 风格背景；展开 markdown) -->
      {:else if chunk.kind === "compact"}
        {@const compactText = cleanDisplayText(chunk.summaryText)}
        {@const isCompactExpanded = expandedCompacts.has(chunk.chunkId)}
        {@const td = chunk.tokenDelta}
        <div
          class="msg-row msg-row-compact"
          class:msg-row-anchor-hit={highlightedChunkId === chunk.chunkId}
          data-chunk-id={chunk.chunkId}
        >
          <div class="compact-block">
            <button
              type="button"
              class="compact-button"
              class:compact-button-expanded={isCompactExpanded}
              onclick={() => toggleCompact(chunk.chunkId)}
              aria-expanded={isCompactExpanded}
              aria-label="Toggle compacted content"
            >
              <svg class="compact-chevron" class:compact-chevron-rotate={isCompactExpanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>
              <svg class="compact-layers-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={LAYERS}/></svg>
              <span class="compact-label">Compacted</span>
              {#if td}
                <span class="compact-token-delta">
                  {formatTokensCompact(td.preCompactionTokens)} → {formatTokensCompact(td.postCompactionTokens)}
                  <span class="compact-token-freed">({formatTokensCompact(Math.abs(td.delta))} freed)</span>
                </span>
              {/if}
              {#if chunk.phaseNumber != null}
                <span class="compact-phase-badge">Phase {chunk.phaseNumber}</span>
              {/if}
              <span class="compact-time">{ftime(chunk.timestamp)}</span>
            </button>
            {#if isCompactExpanded && compactText}
              <div class="compact-expanded">
                <div class="prose lazy-md compact-content" {@attach attachMarkdown(compactText, "system")}></div>
              </div>
            {/if}
          </div>
        </div>
      {/if}
    {/each}
  </div>

  <!-- Quick Anchor Navigation：距底 > 300px 时浮现的"跳到最新消息"按钮 -->
  <button
    type="button"
    class="jump-to-latest"
    class:jump-to-latest-visible={isFar}
    class:jump-to-latest-shifted={contextPanelVisible && contextCount > 0}
    onclick={scrollToLatest}
    title={jumpTooltip}
    aria-label="跳到最新消息"
    aria-hidden={!isFar}
    tabindex={isFar ? 0 : -1}
  >
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      {@html CHEVRONS_DOWN_SVG}
    </svg>
  </button>

  {#if contextPanelVisible && contextCount > 0 && detail}
    <ContextPanel
      {detail}
      onClose={() => (contextPanelVisible = false)}
      onNavigateToChunk={handleNavigateToChunk}
      onNavigateToTool={handleNavigateToTool}
      onNavigateToUserGroup={handleNavigateToUserGroup}
    />
  {/if}
  </div>
{/if}
</div>

<style>
  /* ── Root ── */
  .session-detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* ── Teammate spawn 极简单行（对齐原版 LinkedToolItem.tsx::isTeammateSpawned）── */
  .teammate-spawn-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
  }
  .teammate-spawn-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .teammate-spawn-badge {
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.03em;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid transparent;
    flex-shrink: 0;
  }
  .teammate-spawn-label {
    font-size: 12px;
    color: var(--card-icon-muted);
  }

  /* ── States ── */
  .state-msg {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--color-text-muted);
    font-size: 14px;
  }
  .state-err { color: var(--tool-result-error-text); }

  /* ── Top bar ──
     左侧 4px accent rail + 18px 标题 + 副标题密度行（counts · tokens · last activity · LIVE 标记）
     标题字号从 14/500 跃升至 18/650，与 prose body / metadata 形成 18→14→11 三档清晰节奏。
  */
  /* 顶部 border 移除：与上方 TabBar 行底 1 px border 紧贴会形成视觉加粗
     幻觉（左侧 tab 宽度的下划线与全宽 border 错位），详见 change
     unified-title-bar design D8 + app-chrome spec scenario
     "SessionDetail 顶部不与 TabBar 行底 border 重叠"。
     底部加 1 px border 与下方对话区做分隔。 */
  .top-bar {
    position: relative;
    display: flex;
    align-items: center;
    padding: 14px 24px;
    gap: 16px;
    flex-shrink: 0;
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
  }

  .top-titles {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .top-title {
    font-size: 18px;
    font-weight: 650;
    line-height: 1.25;
    letter-spacing: -0.005em;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 0;
  }

  .top-stats {
    display: flex;
    align-items: center;
    flex-wrap: nowrap;
    gap: 7px;
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.2;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .top-stat {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
  }

  .top-stat-num {
    color: var(--color-text-secondary);
    font-weight: 600;
  }

  .top-stat-unit {
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-size: 10px;
    color: var(--color-text-muted);
    font-weight: 500;
  }

  .top-stat-time .top-stat-num {
    color: var(--color-text-muted);
    font-weight: 500;
  }

  .top-stat-sep {
    color: var(--color-border-emphasis);
    font-weight: 600;
    user-select: none;
  }


  .msg-row-anchor-hit {
    animation: anchor-target-pulse 2200ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .tool-anchor-hit {
    border-radius: 8px;
    animation: anchor-tool-pulse 2200ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes anchor-target-pulse {
    0% {
      background: color-mix(in oklch, var(--color-accent-blue) 16%, transparent);
      box-shadow: inset 0 0 0 1px color-mix(in oklch, var(--color-accent-blue) 48%, transparent);
    }
    55% {
      background: color-mix(in oklch, var(--color-accent-blue) 10%, transparent);
      box-shadow: inset 0 0 0 1px color-mix(in oklch, var(--color-accent-blue) 34%, transparent);
    }
    100% {
      background: transparent;
      box-shadow: inset 0 0 0 1px transparent;
    }
  }

  @keyframes anchor-tool-pulse {
    0% {
      background: color-mix(in oklch, var(--color-accent-blue) 20%, transparent);
      box-shadow: 0 0 0 2px color-mix(in oklch, var(--color-accent-blue) 48%, transparent);
    }
    55% {
      background: color-mix(in oklch, var(--color-accent-blue) 12%, transparent);
      box-shadow: 0 0 0 2px color-mix(in oklch, var(--color-accent-blue) 30%, transparent);
    }
    100% {
      background: transparent;
      box-shadow: 0 0 0 2px transparent;
    }
  }

  .top-meta {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .top-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-secondary);
    background: transparent;
    padding: 6px 10px 6px 9px;
    border-radius: 6px;
    border: 1px solid transparent;
    cursor: pointer;
    font-family: inherit;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }

  .top-badge-icon {
    width: 13px;
    height: 13px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .top-badge-count {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    padding: 0 6px;
    border-radius: 9999px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    line-height: 1.5;
  }

  /* idle hover：限定 :not(.top-badge-active)，否则伪类特异性 (0,2,0)
     会覆盖 .top-badge-active (0,1,0)，active+hover 时会退回灰底 —— 与
     active 蓝态视觉脱节。 */
  .top-badge:not(.top-badge-active):hover {
    background: var(--color-surface-raised);
    color: var(--color-text);
    border-color: var(--color-border);
  }

  .top-badge:not(.top-badge-active):hover .top-badge-icon {
    color: var(--color-text-secondary);
  }

  .top-badge:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 2px;
  }

  /* Context 按钮 active 态：用 neutral 层级（更深 surface + emphasis
     border + 主文本色），与 ai-tool-toggle tools-open 状态视觉语言
     统一。蓝色留给 focus / live 等真的强调状态。详见 DESIGN.md
     `Status Owns the Color Rule`——panel toggle 不属于 selection /
     focus / 语义状态。 */
  .top-badge-active {
    background: var(--color-surface-overlay);
    color: var(--color-text);
    border-color: var(--color-border-emphasis);
  }

  /* active 态已经是最深 raised 层级（surface-overlay），hover 不再加深，
     保持稳态——与桌面工具 (VS Code / IntelliJ) panel toggle 行为一致。
     仍显式声明 :hover 规则覆盖通用 :not(.top-badge-active):hover 兜底。 */
  .top-badge-active:hover {
    background: var(--color-surface-overlay);
    border-color: var(--color-border-emphasis);
    color: var(--color-text);
  }

  .top-badge-active .top-badge-icon {
    color: var(--color-text);
  }

  .top-badge-active .top-badge-count {
    background: var(--color-surface);
    border-color: var(--color-border);
    color: var(--color-text-secondary);
  }

  .top-badge-active:hover .top-badge-count {
    background: var(--color-surface);
  }

  /* ── Content area ── */
  .content-area {
    flex: 1;
    display: flex;
    position: relative;
    overflow: hidden;
    min-height: 0;
    min-width: 0;
  }

  /* ── Conversation ──
     节奏：从 4px 全统一 gap 改为 14px 默认（user→ai 之间 18px，user 是新轮起点要换气）。
  */
  .conversation {
    flex: 1;
    min-width: 0;
    overflow-y: scroll;
    overflow-anchor: none;
    overflow-x: hidden;
    scrollbar-gutter: stable;
    padding: 20px 28px 56px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  /* user→ai 节奏：用户是新一轮 turn 起点，前后视觉换气 */
  .msg-row-user + .msg-row-ai,
  .msg-row-ai + .msg-row-user {
    margin-top: 4px;
  }

  .msg-row {
    display: flex;
    min-width: 0;
  }

  :global(.lazy-md[data-rendered="1"]) {
    min-height: 0 !important;
  }

  .msg-spacer { flex: 1; min-width: 80px; }

  /* ── Jump to latest（floating affordance；DESIGN.md::Floating affordances）──
     contextPanelVisible 时按钮 right offset 让位（CSS-only，不读 JS 常量；
     min(320px, 50vw) 与 ContextPanel 实际 width: min(320px, 100%) 同源对齐——
     50vw 让按钮在窄屏 < 320px 时仍位于可见 conversation 区域内，不被推出
  */
  .jump-to-latest {
    position: absolute;
    bottom: 16px;
    right: 16px;
    z-index: 20;
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border-radius: 6px;
    background: var(--color-surface-raised);
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border-emphasis);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
    cursor: pointer;
    /* 默认隐藏：opacity + translateY 进出，不动 layout 属性 */
    opacity: 0;
    transform: translateY(8px);
    pointer-events: none;
    transition:
      opacity 150ms ease-out,
      transform 150ms ease-out,
      right 200ms ease-out,
      box-shadow 120ms ease-out,
      background-color 120ms ease-out;
  }
  .jump-to-latest svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }
  .jump-to-latest-visible {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
    transition:
      opacity 200ms cubic-bezier(0.16, 1, 0.3, 1),
      transform 200ms cubic-bezier(0.16, 1, 0.3, 1),
      right 200ms ease-out,
      box-shadow 120ms ease-out,
      background-color 120ms ease-out;
  }
  .jump-to-latest:hover {
    background: var(--color-surface-overlay);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  }
  .jump-to-latest:focus-visible {
    outline: none;
    box-shadow:
      0 2px 8px rgba(0, 0, 0, 0.06),
      0 0 0 2px rgba(59, 130, 246, 0.15);
    border-color: var(--color-text-secondary);
  }
  .jump-to-latest:active {
    transform: translateY(0) scale(0.96);
    background: var(--color-surface-overlay);
    transition: transform 200ms ease-out, background-color 80ms ease-out;
  }
  .jump-to-latest-shifted {
    right: calc(min(320px, 50vw) + 16px);
  }
  @media (prefers-reduced-motion: reduce) {
    .jump-to-latest,
    .jump-to-latest-visible,
    .jump-to-latest:active {
      transition: right 0s, box-shadow 0s, background-color 0s;
      transform: none;
    }
    .jump-to-latest-visible { transform: none; }
  }
  :global([data-theme="dark"]) .jump-to-latest {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  }
  :global([data-theme="dark"]) .jump-to-latest:hover {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  /* ── User bubble ──
     时间外置在 bubble 上方右对齐到 conversation 内 right padding（即与 AI
     .ai-header-row 内 time 落在同一垂直 column）；bubble + 30×30 indigo
     avatar 横向排列；bubble 14.5px + inset top 高光 + 微 chiaroscuro shadow。
  */
  .msg-row-user {
    justify-content: flex-end;
  }

  .user-stack {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    min-width: 0;
    max-width: 78%;
    /* msg-row-user justify-content: flex-end → stack 整段右对齐到
       conversation 内 right padding，与 AI .ai-msg-container 的 right edge
       共线（因为 ai-msg-container width: 100%，无 right padding）。 */
  }

  .user-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    line-height: 1.2;
    /* 紧贴 user-stack 右边缘——与 ai-msg-container 的 right edge 共线 */
  }

  .user-meta-name {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 700;
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .user-meta-sep {
    color: var(--color-border-emphasis);
    font-size: 11px;
  }

  .user-meta-time {
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    color: var(--color-text-muted);
  }

  .user-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    min-width: 0;
    max-width: 100%;
    /* 不限制 row max-width，让 row right edge = stack right edge = conversation right padding */
  }

  .msg-bubble {
    border-radius: 14px;
    padding: 11px 15px;
    min-width: 0;
    max-width: 100%;
    box-sizing: border-box;
    flex: 0 1 auto;
    /* 不限制 bubble width — user-stack max-width 78% 已隐式限宽；
       bubble 在 stack 内自然填满，避免再叠 max-width 让短消息无谓收窄
       (用户反馈 visual b)。 */
  }

  .msg-bubble-user {
    background: var(--chat-user-bg);
    color: var(--chat-user-text);
    border: 1px solid var(--chat-user-border);
    box-shadow:
      0 1px 2px rgba(60, 55, 45, 0.08),
      inset 0 1px 0 rgba(255, 255, 255, 0.45);
    font-size: 14.5px;
    line-height: 1.55;
  }

  :global([data-theme="dark"]) .msg-bubble-user,
  :global([data-theme="system"]) .msg-bubble-user {
    box-shadow:
      0 1px 2px rgba(0, 0, 0, 0.3),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
  }

  .user-avatar {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: var(--color-accent-indigo);
    background: color-mix(in oklch, var(--color-accent-indigo) 8%, var(--color-surface));
    border: 1px solid color-mix(in oklch, var(--color-accent-indigo) 32%, transparent);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
    margin-top: 2px;
  }

  .user-avatar svg {
    width: 14px;
    height: 14px;
  }

  /* task-notification 卡片：移植自原版 UserChatGroup.tsx 末尾的 task notif 渲染 */
  .task-notif {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 6px 12px;
    margin-top: 6px;
    border-radius: 8px;
    background: var(--card-bg);
    border: 1px solid var(--card-border);
  }

  .task-notif-icon {
    flex-shrink: 0;
    margin-top: 1px;
    font-size: 14px;
    line-height: 1;
    color: var(--color-text-muted);
  }

  .task-notif-done .task-notif-icon { color: var(--color-success-bright); }
  .task-notif-fail .task-notif-icon { color: var(--color-danger-bright); }

  .task-notif-body {
    min-width: 0;
    flex: 1;
  }

  .task-notif-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-notif-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .task-notif-status { text-transform: capitalize; }
  .task-notif-file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 240px;
  }

  /* ── AI message ──
     thread rail 是页面最重要的视觉脉络。
     - 默认：3px solid border-emphasis；左缘外置 timeline node（7×7 圆点）
     - ongoing tail：accent-blue + glow + 顶端"扫光"动效
     节奏：thread rail 把所有 AIChunk 串成执行轨迹，避免"消息列表"感。
  */
  .msg-row-ai {
    justify-content: flex-start;
    /* 给 timeline node 流出在容器外的空间 */
    padding-left: 8px;
  }

  .msg-ai-container {
    position: relative;
    width: 100%;
    /* 不限制 max-width，让 AI header 内 time 的右边缘 = conversation
       内 right padding（与 user-meta time 同一 x 列）。 */
    min-width: 0;
    border-left: 3px solid var(--color-border-emphasis);
    padding-left: 16px;
    transition: border-color 320ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .msg-ai-container::before {
    /* 顶端 cap：让 thread 头部有一个可见 anchor 点，避免边线"突兀"。 */
    content: "";
    position: absolute;
    left: -3px;
    top: 0;
    width: 3px;
    height: 16px;
    background: linear-gradient(180deg, transparent 0%, currentColor 100%);
    color: var(--color-border-emphasis);
    transition: color 320ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  /* live thread rail 弱化：用纯蓝 + 透明度（暖白底透过来），不混灰避免
     脏色。3px 全长 border 用满 accent-blue 会把眼睛锁在 rail 上而不是
     消息内容，与 product register 的"克制工作台"风格冲突。 */
  .msg-ai-container-live {
    border-left-color: color-mix(in oklch, var(--color-accent-blue) 55%, transparent);
  }

  .msg-ai-container-live::before {
    color: color-mix(in oklch, var(--color-accent-blue) 55%, transparent);
    opacity: 1;
  }

  /* 左外侧 timeline node：执行轨迹的"节点" */
  .ai-thread-node {
    position: absolute;
    left: -7px;
    top: 14px;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--color-surface);
    border: 2.5px solid var(--color-border-emphasis);
    box-shadow: 0 0 0 2px var(--color-surface);
    transition: border-color 320ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  /* timeline live node：outline 形态 + 纯蓝 55% 透明度，与 thread rail
     统一弱化语言；保留 surface 隔离环避免与 timeline 主线条粘连。
     详见 DESIGN.md `The Static-vs-Live Shape Rule` 与 `One Live Signal Rule`。 */
  .ai-thread-node-live {
    border-width: 2px;
    border-color: color-mix(in oklch, var(--color-accent-blue) 55%, transparent);
    background: var(--color-surface);
    box-shadow: 0 0 0 2px var(--color-surface);
  }

  @media (prefers-reduced-motion: reduce) {
    .msg-ai-container::before {
      display: none;
    }
  }

  .ai-header-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 0 8px;
  }

  .ai-avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    color: var(--color-text-secondary);
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .ai-avatar svg {
    width: 14px;
    height: 14px;
  }

  .ai-label {
    font-size: 14px;
    font-weight: 700;
    color: var(--color-text);
    flex-shrink: 0;
    letter-spacing: -0.005em;
  }

  /* model badge：从填充 badge 改"工程标记" — 1px dashed border + uppercase mono */
  .ai-model {
    font-size: 10px;
    font-weight: 600;
    color: var(--color-text-secondary);
    background: transparent;
    padding: 2px 7px;
    border-radius: 4px;
    border: 1px dashed var(--color-border-emphasis);
    font-family: var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    flex-shrink: 0;
  }

  /* AI tools toggle：从 inline link 改 chip 形态，明示"可点击展开 N 项" */
  .ai-tool-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--color-text-secondary);
    cursor: pointer;
    padding: 3px 9px 3px 8px;
    border-radius: 9999px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
    flex-shrink: 1;
    min-width: 0;
    font-family: inherit;
    max-width: min(640px, calc(100% - 240px));
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ai-tool-toggle:hover {
    background: var(--color-surface-overlay);
    color: var(--color-text);
    border-color: var(--color-border-emphasis);
  }

  /* tools-open 选中态：用 neutral 层级（更深一档 surface + emphasis
     border + 主文本色）+ chevron 90° 旋转表达"已展开"，把蓝色留给
     focus / live 等真的强调状态。详见 DESIGN.md `Status Owns the
     Color Rule`——展开/折叠不属于 selection / focus / 语义状态。 */
  .msg-ai-container-tools-open .ai-tool-toggle {
    background: var(--color-surface-overlay);
    border-color: var(--color-border-emphasis);
    color: var(--color-text);
  }

  .ai-tool-toggle:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 2px;
  }

  .ai-tool-chevron {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: transform 220ms cubic-bezier(0.16, 1, 0.3, 1);
    color: currentColor;
    opacity: 0.7;
  }

  .ai-tool-chevron svg {
    width: 11px;
    height: 11px;
  }

  .ai-tool-chevron-open {
    transform: rotate(90deg);
    opacity: 1;
  }

  .ai-tools-section {
    padding: 4px 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 4px;
    min-width: 0;
  }

  .ai-header-spacer { flex: 1; }

  .ai-tokens {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
    cursor: pointer;
    background: none;
    border: none;
    padding: 2px 4px;
    border-radius: 4px;
    transition: background 120ms ease;
  }

  .ai-tokens:hover {
    background: var(--color-surface-raised);
  }

  .ai-tokens-active {
    background: var(--color-surface-overlay);
  }

  .ai-tokens:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.15);
  }

  .ai-tokens-info {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    opacity: 0.7;
  }

  .ai-tokens-popover {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 20;
    min-width: 180px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    box-shadow:
      0 12px 32px rgba(0, 0, 0, 0.14),
      0 0 0 1px color-mix(in oklch, var(--color-accent-blue) 0%, transparent);
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 11.5px;
    font-family: var(--font-mono);
    text-align: left;
    cursor: default;
  }

  .tok-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .tok-row > :first-child {
    color: var(--color-text-muted);
  }

  .tok-row > :last-child {
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .tok-row-total {
    padding-bottom: 4px;
    margin-bottom: 2px;
    border-bottom: 1px solid var(--card-separator, var(--card-border));
    font-weight: 600;
  }

  .tok-row-total > :last-child {
    color: var(--color-text);
  }

  .tok-divider {
    height: 1px;
    background: var(--color-border);
    margin: 3px 0;
  }

  .tok-row-visible {
    font-weight: 500;
    color: var(--color-text-secondary);
  }

  .ai-duration {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
    white-space: nowrap;
  }

  .ai-duration svg {
    width: 11px;
    height: 11px;
  }

  .ai-time {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .ai-body {
    padding: 0 0 8px 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  /* ── System ──
     与 AI 共享同一条左侧 thread rail（3px solid border-emphasis），形成贯穿整页
     的执行轨迹；turn 类型差异通过左外侧 timeline marker 形态承载——AI 圆 / SYSTEM
     方（伪元素），不通过新增独立 rail 表达。header chip / bubble 起点都对齐到
     conversation 27px 内容基线（与 AI avatar / 内容左缘同列），消除原 dotted +
     双重 padding-left 造成的横向错位。
  */
  .msg-row-system-left {
    padding: 4px 0 4px 8px;
    justify-content: flex-start;
  }

  .system-block {
    position: relative;
    width: 100%;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    border-left: 3px solid var(--color-border-emphasis);
    padding-left: 16px;
  }

  /* 顶端 cap：与 AI .msg-ai-container::before 同源，给 thread head 一个 anchor */
  .system-block::before {
    content: "";
    position: absolute;
    left: -3px;
    top: 0;
    width: 3px;
    height: 16px;
    background: linear-gradient(180deg, transparent 0%, var(--color-border-emphasis) 100%);
  }

  /* 左外侧 timeline marker：方形 outline（与 AI 圆形 .ai-thread-node 形态对立）
     box-shadow 0 0 0 2px surface 形成隔离环，避免与主 rail 实线粘连。 */
  .system-block::after {
    content: "";
    position: absolute;
    left: -7px;
    top: 14px;
    width: 11px;
    height: 11px;
    border-radius: 2px;
    background: var(--color-surface);
    border: 2.5px solid var(--color-border-emphasis);
    box-shadow: 0 0 0 2px var(--color-surface);
  }

  .system-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 0 4px;
  }

  .system-meta-sep {
    color: var(--color-border-emphasis);
    font-size: 11px;
  }

  .system-icon {
    width: 12px;
    height: 12px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .system-label {
    color: var(--color-text-secondary);
    font-weight: 700;
    font-size: 10px;
    font-family: var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
  }

  .system-time {
    color: var(--color-text-muted);
    font-size: 10.5px;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .system-pre {
    font-size: 13px;
    font-family: var(--font-mono);
    color: var(--chat-system-text);
    background: var(--chat-system-bg);
    border: 1px solid var(--color-border);
    border-radius: 10px;
    padding: 12px 16px;
    margin: 0;
    white-space: pre-wrap;
    overflow-x: auto;
    max-height: 384px;
    overflow-y: auto;
    scrollbar-gutter: stable;
    line-height: 1.6;
  }

  .msg-row-compact {
    padding: 18px 0;
    justify-content: stretch;
  }

  .compact-block {
    width: 100%;
    position: relative;
  }

  .compact-button {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 10px;
    width: 100%;
    padding: 12px 16px;
    background: linear-gradient(
      90deg,
      color-mix(in oklch, var(--color-warning) 9%, transparent) 0%,
      color-mix(in oklch, var(--color-warning) 4%, transparent) 100%
    );
    border: 1px solid color-mix(in oklch, var(--color-warning) 32%, transparent);
    border-radius: 10px;
    color: var(--color-warning-text);
    cursor: pointer;
    font-family: inherit;
    text-align: left;
    transition: background 180ms ease, border-color 180ms ease, transform 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .compact-button:hover {
    background: linear-gradient(
      90deg,
      color-mix(in oklch, var(--color-warning) 14%, transparent) 0%,
      color-mix(in oklch, var(--color-warning) 7%, transparent) 100%
    );
    border-color: color-mix(in oklch, var(--color-warning) 45%, transparent);
  }

  .compact-button:focus-visible {
    outline: 2px solid var(--color-warning);
    outline-offset: 2px;
  }

  .compact-chevron {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    transition: transform 220ms cubic-bezier(0.16, 1, 0.3, 1);
    opacity: 0.85;
  }
  .compact-chevron-rotate {
    transform: rotate(90deg);
  }

  .compact-layers-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    opacity: 0.85;
  }

  .compact-label {
    font-size: 11px;
    font-weight: 700;
    font-family: var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.14em;
    flex-shrink: 0;
    color: var(--color-warning-text);
  }

  .compact-token-delta {
    display: inline-flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    max-width: 100%;
    overflow: hidden;
    white-space: nowrap;
    font-size: 12.5px;
    color: var(--color-text-secondary);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .compact-token-freed {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--color-success);
    font-weight: 700;
    font-size: 12px;
    letter-spacing: 0.02em;
    padding: 0 2px;
  }
  .compact-token-freed::before {
    content: "";
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    flex-shrink: 0;
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-success) 14%, transparent);
  }

  .compact-phase-badge {
    flex-shrink: 0;
    padding: 2px 8px;
    border-radius: 4px;
    background: color-mix(in oklch, var(--color-accent-indigo) 14%, transparent);
    color: var(--color-accent-indigo);
    font-size: 10px;
    font-weight: 700;
    font-family: var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    white-space: nowrap;
    border: 1px solid color-mix(in oklch, var(--color-accent-indigo) 28%, transparent);
  }

  .compact-time {
    flex-shrink: 0;
    font-size: 10.5px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .compact-expanded {
    margin-top: 10px;
    border: 1px solid var(--color-border);
    border-left: 3px solid color-mix(in oklch, var(--color-warning) 35%, transparent);
    border-radius: 8px;
    overflow: hidden;
  }

  .compact-content {
    max-height: 384px;
    overflow-y: auto;
    scrollbar-gutter: stable;
    padding: 14px 18px;
    font-size: 13px;
  }

  /* ── Prose (markdown) ── */
  .prose {
    min-width: 0;
    max-width: 100%;
    font-size: 14px;
    color: var(--prose-body);
    line-height: 1.65;
    overflow-wrap: anywhere;
    word-break: break-word;
  }
  .prose :global(p) { margin: 0.35em 0; }
  .prose :global(p:first-child) { margin-top: 0; }
  .prose :global(p:last-child) { margin-bottom: 0; }
  .prose :global(h1), .prose :global(h2), .prose :global(h3), .prose :global(h4) {
    color: var(--prose-heading);
    font-weight: 600;
    margin: 0.9em 0 0.35em;
  }
  .prose :global(h1) { font-size: 1.25em; }
  .prose :global(h2) { font-size: 1.12em; }
  .prose :global(h3) { font-size: 1.05em; }
  .prose :global(ul), .prose :global(ol) { margin: 0.35em 0; padding-left: 1.4em; }
  .prose :global(li) { margin: 0.15em 0; }
  .prose :global(code) {
    max-width: 100%;
    background: var(--prose-code-bg);
    color: var(--prose-code-text);
    padding: 1px 5px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.87em;
    overflow-wrap: anywhere;
  }
  .prose :global(pre) {
    min-width: 0;
    max-width: 100%;
    box-sizing: border-box;
    background: var(--prose-pre-bg);
    border: 1px solid var(--prose-pre-border);
    border-radius: 6px;
    padding: 12px 14px;
    overflow-x: auto;
    margin: 0.6em 0;
    line-height: 1.5;
  }
  .prose :global(pre code) {
    display: block;
    width: max-content;
    min-width: 100%;
    max-width: none;
    background: none;
    color: var(--color-text-secondary);
    padding: 0;
    border-radius: 0;
    overflow-wrap: normal;
    word-break: normal;
  }
  .prose :global(a) { color: var(--prose-link); text-decoration: none; }
  .prose :global(a:hover) { text-decoration: underline; }
  .prose :global(blockquote) {
    border-left: 3px solid var(--prose-blockquote-border);
    margin: 0.5em 0;
    padding: 2px 0 2px 14px;
    color: var(--prose-muted);
  }
  .prose :global(strong) { color: var(--prose-heading); }
  .prose :global(.table-scroll) {
    max-width: 100%;
    overflow-x: auto;
    margin: 0.6em 0;
  }
  .prose :global(table) {
    border-collapse: collapse;
    font-size: 0.92em;
    width: max-content;
    min-width: 100%;
  }
  .prose :global(th), .prose :global(td) {
    border: 1px solid var(--prose-table-border);
    padding: 5px 10px;
    text-align: left;
  }
  .prose :global(th) {
    background: var(--prose-table-header-bg);
    font-weight: 600;
  }

  /* Prose 内的 hljs token 颜色统一在 app.css 的 .hljs-* 全局规则里 */

  /* Thinking 正文：与 prose 同字体同行高，13px 略小暗示次级。
     身份靠 BRAIN icon + label 区分，不靠颜色——与 ExecutionTrace 同步。 */
  .prose-thinking {
    color: var(--color-text);
    font-size: 13px;
    line-height: 1.65;
  }

  /* Interruption：用户显式 Esc 操作，视觉权重高于普通工具行但低于 error。
     从早期 1px dashed line → warning chip：浅 amber bg + 1px border + 实
     icon + mono UPPERCASE label + sentence-case 文案 + 末尾 ↩ 锚定字符。
     注意：DESIGN.md 禁 side-stripe，用 background tint 表达，不用左侧粗条。 */
  .interruption-block {
    margin-top: 10px;
    padding: 8px 12px;
    border-radius: 6px;
    background: var(--color-warning-bg);
    border: 1px solid var(--color-warning-border);
    color: var(--color-warning-text);
    font-size: 13px;
    line-height: 1.3;
    display: flex;
    width: 100%;
    align-items: center;
    gap: 8px;
  }
  .interruption-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--color-warning-text);
  }
  .interruption-label {
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: var(--color-warning-text);
    text-transform: uppercase;
    flex-shrink: 0;
  }
  .interruption-text {
    font-weight: 500;
    flex: 1;
    min-width: 0;
  }
  .interruption-glyph {
    font-family: var(--font-mono);
    font-size: 12px;
    opacity: 0.7;
    flex-shrink: 0;
  }
</style>
