<script lang="ts">
  import type { TeammateMessage } from "../lib/api";
  import { getTeamColorSet } from "../lib/teamColors";
  import { CHEVRON_RIGHT, CORNER_DOWN_LEFT, MESSAGE_SQUARE, REFRESH_CW } from "../lib/icons";

  type AttachFn = (el: HTMLElement) => void | (() => void);

  interface Props {
    teammateMessage: TeammateMessage;
    /**
     * SessionDetail 注入的 lazy markdown 附着工厂：父级调
     * `attachMarkdown(text, "teammate")` 返回 attach 函数，子级以
     * `{@attach attachBody}` 直接挂载。设计参见
     * `openspec/changes/teammate-message-rendering/design.md` D5。
     */
    attachBody: AttachFn;
    /** 顶层 SessionDetail 的 sessionId；预留给未来 hover→spotlight SendMessage 联动。 */
    rootSessionId?: string;
  }

  let { teammateMessage, attachBody }: Props = $props();

  let isExpanded = $state(false);

  const colorSet = $derived(getTeamColorSet(teammateMessage.color));

  const truncatedSummary = $derived.by(() => {
    const s = teammateMessage.summary ?? "";
    if (!s) return "Teammate message";
    return s.length > 80 ? s.slice(0, 80) + "…" : s;
  });

  const tokenLabel = $derived.by(() => {
    const n = teammateMessage.tokenCount;
    if (!n || n <= 0) return "";
    if (n >= 1000) return `~${(n / 1000).toFixed(1)}k tokens`;
    return `~${n} tokens`;
  });

  const noiseLabel = $derived.by(() => {
    if (!teammateMessage.isNoise) return "";
    const body = teammateMessage.body.trim();
    if (body.startsWith("{")) {
      try {
        const parsed = JSON.parse(body) as { type?: string; message?: string };
        if (parsed.message) return parsed.message;
        if (parsed.type) return humanizeNoiseType(parsed.type);
      } catch {
        // fall through
      }
    }
    return body.length > 0 ? body : "Operational signal";
  });

  function humanizeNoiseType(t: string): string {
    switch (t) {
      case "idle_notification":
        return "Idle";
      case "shutdown_approved":
        return "Shutdown confirmed";
      case "teammate_terminated":
        return "Terminated";
      case "shutdown_request":
        return "Shutdown requested";
      default:
        return t;
    }
  }

  function toggle() {
    isExpanded = !isExpanded;
  }
</script>

{#if teammateMessage.isNoise}
  <!-- 噪声态：极简单行 + opacity 0.45 -->
  <div class="tm-noise" title={teammateMessage.body}>
    <span class="tm-noise-dot" style:background-color={colorSet.border}></span>
    <span class="tm-noise-id">{teammateMessage.teammateId}</span>
    <span class="tm-noise-label">{noiseLabel}</span>
  </div>
{:else}
  <div
    class="tm-card"
    class:tm-resent={teammateMessage.isResend}
    style:border-left-color={colorSet.border}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="tm-header" class:tm-header-expanded={isExpanded} onclick={toggle}>
      <svg
        class="tm-chevron"
        class:tm-chevron-open={isExpanded}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d={CHEVRON_RIGHT} />
      </svg>

      <svg
        class="tm-msg-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        style:color={colorSet.border}
      >
        <path d={MESSAGE_SQUARE} stroke="currentColor" />
      </svg>

      <span
        class="tm-badge"
        style:background-color={colorSet.badge}
        style:color={colorSet.text}
        style:border-color="{colorSet.border}40"
      >
        {teammateMessage.teammateId}
      </span>

      <span class="tm-type-label">Message</span>

      {#if teammateMessage.replyToToolUseId}
        <span class="tm-reply-chip" title="Reply to SendMessage {teammateMessage.replyToToolUseId}">
          <svg
            class="tm-reply-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d={CORNER_DOWN_LEFT} />
          </svg>
          <span class="tm-reply-text">reply</span>
        </span>
      {/if}

      {#if teammateMessage.isResend}
        <span class="tm-resend-chip">
          <svg
            class="tm-resend-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d={REFRESH_CW} />
          </svg>
          Resent
        </span>
      {/if}

      <span class="tm-summary">{truncatedSummary}</span>

      {#if tokenLabel}
        <span class="tm-tokens">{tokenLabel}</span>
      {/if}
    </div>

    {#if isExpanded}
      <div class="tm-body">
        <div class="prose lazy-md" {@attach attachBody}></div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .tm-card {
    border-radius: 6px;
    border: 1px solid var(--card-border);
    border-left: 3px solid var(--card-border);
    background: var(--card-bg);
    overflow: hidden;
    transition: opacity 0.2s;
  }
  .tm-resent {
    opacity: 0.6;
  }

  .tm-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    cursor: pointer;
    transition: background-color 0.1s;
  }
  .tm-header:hover {
    background: var(--card-header-hover);
  }
  .tm-header-expanded {
    background: var(--card-header-bg);
    border-bottom: 1px solid var(--card-border);
  }

  .tm-chevron {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--card-icon-muted);
    transition: transform 0.15s ease;
  }
  .tm-chevron-open {
    transform: rotate(90deg);
  }

  .tm-msg-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .tm-badge {
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.03em;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid transparent;
    flex-shrink: 0;
  }

  .tm-type-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--card-icon-muted);
    flex-shrink: 0;
  }

  .tm-reply-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 10px;
    color: var(--card-icon-muted);
    flex-shrink: 0;
  }
  .tm-reply-icon {
    width: 10px;
    height: 10px;
  }

  .tm-resend-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 10px;
    color: var(--card-icon-muted);
    flex-shrink: 0;
  }
  .tm-resend-icon {
    width: 10px;
    height: 10px;
  }

  .tm-summary {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--card-text-light);
  }

  .tm-tokens {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--card-icon-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .tm-body {
    padding: 12px;
  }

  .tm-noise {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    opacity: 0.45;
  }
  .tm-noise-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .tm-noise-id {
    font-size: 11px;
    color: var(--card-icon-muted);
    flex-shrink: 0;
  }
  .tm-noise-label {
    font-size: 11px;
    color: var(--card-icon-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
