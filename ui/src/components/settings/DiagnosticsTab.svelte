<script lang="ts">
  // Diagnostics tab — pull-based telemetry snapshot dashboard.
  //
  // Visual contract（详 PR description D-V1/D-V2/D-V3）：
  //   ① 健康度横幅 —— success/warning/danger 三态，复用 DESIGN.md 状态色 token
  //   ② 关键指标 —— SettingsField 风格的 list 行，避开 hero metric 反模式
  //   ③ 详细技术数据 —— <details> inline disclosure，开发者排错入口

  import { onMount } from "svelte";
  import { getTelemetrySnapshot, type TelemetrySnapshot } from "../../lib/api";
  import SettingsGroup from "../../lib/components/SettingsGroup.svelte";
  import SettingsButton from "../../lib/components/SettingsButton.svelte";
  import SkeletonList from "../SkeletonList.svelte";
  import { CHECK_CIRCLE_SVG, ALERT_CIRCLE_SVG } from "../../lib/icons";

  let snapshot = $state<TelemetrySnapshot | null>(null);
  let loading = $state(true);
  let refreshing = $state(false);
  let error: string | null = $state(null);
  let copyToast: string | null = $state(null);

  onMount(() => {
    void load();
  });

  async function load() {
    loading = true;
    error = null;
    try {
      snapshot = await getTelemetrySnapshot();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    error = null;
    try {
      // silent reload：保留旧 snapshot 直到新数据到达，避免闪烁
      const next = await getTelemetrySnapshot();
      snapshot = next;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      refreshing = false;
    }
  }

  async function copySnapshot() {
    if (!snapshot) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2));
      copyToast = "已复制完整 snapshot 到剪贴板";
    } catch (e) {
      copyToast = "复制失败";
      console.warn("[DiagnosticsTab] copy failed", e);
    }
    setTimeout(() => (copyToast = null), 2000);
  }

  // ===== counter helpers =====
  function counterValue(name: string): number {
    return snapshot?.counters[name] ?? 0;
  }

  const cacheHits = $derived(counterValue("metadata.cache.hit"));
  const cacheMiss = $derived(counterValue("metadata.cache.miss"));
  const cacheSigSkew = $derived(counterValue("metadata.cache.sig_mismatch"));
  const cacheStatErr = $derived(counterValue("metadata.cache.stat_err"));
  const cacheTotal = $derived(cacheHits + cacheMiss + cacheSigSkew + cacheStatErr);
  const cacheHitRate = $derived(cacheTotal > 0 ? cacheHits / cacheTotal : null);
  const cacheHitRateText = $derived(
    cacheHitRate == null ? "—" : `${(cacheHitRate * 100).toFixed(1)}%`,
  );

  const ipcErrorTotal = $derived(
    counterValue("cdt_api.error") + counterValue("cdt_api.warn"),
  );
  const panicCount = $derived(counterValue("panic.recovered"));
  const sshReconnectCount = $derived(counterValue("ssh.reconnect"));

  // ===== health derivation =====
  // 阈值：panic > 0 → red；ipc.error+warn > 0 或 cache 命中率 < 70% → amber；
  //       否则 → green。ssh.reconnect 不参与判定（远端工作区天然偶尔重连）。
  type HealthLevel = "green" | "amber" | "red";
  const health = $derived.by((): { level: HealthLevel; headline: string; detail: string } => {
    if (panicCount > 0) {
      return {
        level: "red",
        headline: `检测到崩溃恢复 × ${panicCount}`,
        detail: "应用本次运行中遇到内部错误并自动恢复，建议复制下方 snapshot 反馈。",
      };
    }
    if (ipcErrorTotal > 0) {
      return {
        level: "amber",
        headline: `检测到 ${ipcErrorTotal} 次内部调用错误`,
        detail: "可能与卡顿、刷新失败等异常相关，详情见下方技术数据。",
      };
    }
    if (cacheHitRate !== null && cacheHitRate < 0.7) {
      return {
        level: "amber",
        headline: `缓存命中率偏低（${(cacheHitRate * 100).toFixed(1)}%）`,
        detail: "文件加载将更频繁触发磁盘扫描，体感可能稍慢。",
      };
    }
    return {
      level: "green",
      headline: "一切正常",
      detail: "本次运行未发现异常。",
    };
  });

  // ===== histogram config =====
  const histogramConfig: Array<{ name: string; label: string }> = [
    { name: "ipc.list_sessions.duration_ns", label: "会话列表加载耗时" },
    { name: "ipc.get_session_detail.duration_ns", label: "会话详情加载耗时" },
  ];

  function fmtNs(n: number | null | undefined): string {
    if (n == null) return "—";
    if (n < 1000) return `${n} ns`;
    if (n < 1_000_000) return `${(n / 1000).toFixed(1)} μs`;
    if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(1)} ms`;
    return `${(n / 1_000_000_000).toFixed(2)} s`;
  }

  function bucketRect(buckets: number[], i: number): { x: number; y: number; h: number; opacity: number } {
    const v = buckets[i] ?? 0;
    const max = Math.max(...buckets, 1);
    const h = (v / max) * 70;
    return { x: i * 10, y: 80 - h, h, opacity: v > 0 ? 0.85 : 0.15 };
  }
</script>

<div class="diagnostics">
  {#if loading && !snapshot}
    <SkeletonList count={3} rowHeight={64} gap={12} padding="0" label="正在加载 telemetry" />
  {:else}
    {#if snapshot}
      <!-- ① 健康度横幅 -->
      <div class="health-banner health-banner-{health.level}" role="status">
        <span class="health-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            {#if health.level === "green"}
              {@html CHECK_CIRCLE_SVG}
            {:else}
              {@html ALERT_CIRCLE_SVG}
            {/if}
          </svg>
        </span>
        <div class="health-text">
          <div class="health-headline">{health.headline}</div>
          <div class="health-detail">{health.detail}</div>
        </div>
        <div class="health-action">
          <SettingsButton variant="ghost" onClick={refresh} disabled={refreshing}>
            {refreshing ? "刷新中…" : "刷新"}
          </SettingsButton>
        </div>
      </div>
    {/if}

    {#if error}
      <div class="banner-inline-error" role="alert">
        {snapshot ? `刷新失败：${error}` : `无法加载 telemetry：${error}`}
      </div>
    {/if}

    {#if copyToast}
      <div class="copy-toast" role="status" aria-live="polite">{copyToast}</div>
    {/if}

    {#if snapshot}
      <!-- ② 关键指标（list 行列表） -->
      <SettingsGroup title="关键指标">
        <div class="metric-row">
          <div class="metric-line">
            <span class="metric-label">缓存命中率</span>
            <span class="metric-value">{cacheHitRateText}</span>
          </div>
          <div class="metric-hint">越高代表越少重复扫描文件</div>
          <div class="metric-raw">hit {cacheHits} / miss {cacheMiss} / sig {cacheSigSkew} / stat_err {cacheStatErr}</div>
        </div>
        <div class="metric-row">
          <div class="metric-line">
            <span class="metric-label">内部调用错误</span>
            <span class="metric-value">{ipcErrorTotal}</span>
          </div>
          <div class="metric-hint">本次运行 IPC 调用累计错误次数</div>
          <div class="metric-raw">cdt_api.error + cdt_api.warn</div>
        </div>
        <div class="metric-row">
          <div class="metric-line">
            <span class="metric-label">崩溃自愈次数</span>
            <span class="metric-value">{panicCount}</span>
          </div>
          <div class="metric-hint">系统级错误自动恢复，理想为 0</div>
          <div class="metric-raw">panic.recovered</div>
        </div>
        <div class="metric-row">
          <div class="metric-line">
            <span class="metric-label">SSH 远端重连</span>
            <span class="metric-value">{sshReconnectCount}</span>
          </div>
          <div class="metric-hint">仅使用远端工作区时有意义</div>
          <div class="metric-raw">ssh.reconnect</div>
        </div>
      </SettingsGroup>

      <!-- ③ 详细技术数据（默认折叠） -->
      <details class="tech-details">
        <summary class="tech-summary">
          <span class="tech-summary-chevron" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="9 18 15 12 9 6" />
            </svg>
          </span>
          <div class="tech-summary-text">
            <span class="tech-summary-title">详细技术数据</span>
            <span class="tech-summary-subtitle">开发者排错用</span>
          </div>
        </summary>

        <div class="tech-body">
          <SettingsGroup title="延迟分布">
            {#each histogramConfig as cfg (cfg.name)}
              {@const h = snapshot.histograms[cfg.name]}
              {#if h}
                <div class="hist-row">
                  <div class="hist-line">
                    <span class="hist-label">{cfg.label}</span>
                    <span class="hist-name">{cfg.name}</span>
                  </div>
                  <div class="hist-meta">
                    count={h.count} · p50≤{fmtNs(h.p50Ns)} · p95≤{fmtNs(h.p95Ns)} · p99≤{fmtNs(h.p99Ns)}
                  </div>
                  <div class="hist-caveat">power-of-2 bucket（实际值 ≤ 此值，最坏 2× 偏差）</div>
                  <svg class="hist-svg" viewBox="0 0 320 80" preserveAspectRatio="none" aria-hidden="true">
                    {#each h.buckets as _, i (i)}
                      {@const r = bucketRect(h.buckets, i)}
                      <rect x={r.x} y={r.y} width="9" height={r.h} fill="currentColor" opacity={r.opacity} />
                    {/each}
                  </svg>
                  <div class="hist-axis">
                    <span>1 ns</span>
                    <span>1 μs</span>
                    <span>1 ms</span>
                    <span>1 s</span>
                  </div>
                </div>
              {/if}
            {/each}
          </SettingsGroup>

          <SettingsGroup title="最近 50 条事件">
            {#if snapshot.recentEvents.length === 0}
              <div class="events-empty">尚无 event。</div>
            {:else}
              <div class="events-table-wrap">
                <table class="events-table">
                  <thead>
                    <tr>
                      <th class="events-col-time">时间</th>
                      <th class="events-col-kind">kind</th>
                      <th class="events-col-fields">fields</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each snapshot.recentEvents.slice(-50).reverse() as ev (ev.tsUnixMs + ev.kind)}
                      <tr>
                        <td>{new Date(ev.tsUnixMs).toLocaleTimeString()}</td>
                        <td><code>{ev.kind}</code></td>
                        <td class="events-fields">
                          {Object.entries(ev.fields).map(([k, v]) => `${k}=${v}`).join(", ")}
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
          </SettingsGroup>

          <div class="tech-actions">
            <SettingsButton variant="primary" onClick={copySnapshot} disabled={!snapshot}>
              复制完整 snapshot 给作者
            </SettingsButton>
          </div>
        </div>
      </details>
    {/if}
  {/if}
</div>

<style>
  .diagnostics {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  /* ===== ① health banner ===== */
  .health-banner {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 14px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface-raised);
    color: var(--color-text-secondary);
    font-size: 13px;
    line-height: 1.5;
  }
  .health-icon {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    margin-top: 1px;
  }
  .health-icon :global(svg) {
    width: 18px;
    height: 18px;
  }
  .health-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .health-headline {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
    line-height: 1.35;
  }
  .health-detail {
    font-size: 12px;
    color: var(--color-text-secondary);
    line-height: 1.55;
  }
  .health-action {
    flex-shrink: 0;
  }

  /* green：复用 SettingsView .banner-success 同模式（color-mix） */
  .health-banner-green {
    border-color: color-mix(in oklch, var(--color-success-bright) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-success-bright) 8%, var(--color-surface));
  }
  .health-banner-green .health-icon,
  .health-banner-green .health-headline {
    color: var(--color-success);
  }

  /* amber：用预制 --color-warning-* 三件套（已 paired light/dark） */
  .health-banner-amber {
    border-color: var(--color-warning-border);
    background: var(--color-warning-bg);
  }
  .health-banner-amber .health-icon,
  .health-banner-amber .health-headline {
    color: var(--color-warning-text);
  }

  /* red：复用 SettingsView .banner-error 同模式 */
  .health-banner-red {
    border-color: color-mix(in oklch, var(--color-danger-bright) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-danger-bright) 8%, var(--color-surface));
  }
  .health-banner-red .health-icon,
  .health-banner-red .health-headline {
    color: var(--color-danger);
  }

  /* ===== inline banner（refresh / load 失败） ===== */
  .banner-inline-error {
    padding: 10px 14px;
    border: 1px solid color-mix(in oklch, var(--color-danger-bright) 35%, var(--color-border));
    border-radius: 8px;
    background: color-mix(in oklch, var(--color-danger-bright) 8%, var(--color-surface));
    color: var(--tool-result-error-text);
    font-size: 13px;
    line-height: 1.5;
  }

  .copy-toast {
    align-self: flex-start;
    padding: 6px 12px;
    border: 1px solid color-mix(in oklch, var(--color-success-bright) 35%, var(--color-border));
    border-radius: 9999px;
    background: color-mix(in oklch, var(--color-success-bright) 10%, var(--color-surface));
    color: var(--color-success);
    font-size: 12px;
  }

  /* ===== ② metric list rows ===== */
  .metric-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 16px;
    background: var(--color-surface);
  }
  .metric-line {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 16px;
  }
  .metric-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text);
  }
  .metric-value {
    font-family: var(--font-mono);
    font-size: 18px;
    font-weight: 600;
    color: var(--color-text);
    line-height: 1.2;
    letter-spacing: -0.01em;
  }
  .metric-hint {
    font-size: 12px;
    color: var(--color-text-secondary);
    line-height: 1.5;
  }
  .metric-raw {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    line-height: 1.45;
    word-break: break-all;
  }

  /* ===== ③ details disclosure ===== */
  .tech-details {
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface);
  }
  .tech-summary {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    cursor: pointer;
    list-style: none;
    user-select: none;
  }
  .tech-summary::-webkit-details-marker {
    display: none;
  }
  .tech-summary:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: -2px;
    border-radius: 8px;
  }
  .tech-summary-chevron {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    color: var(--color-text-muted);
    transition: transform 0.15s ease;
  }
  .tech-summary-chevron :global(svg) {
    width: 14px;
    height: 14px;
  }
  .tech-details[open] .tech-summary-chevron {
    transform: rotate(90deg);
  }
  @media (prefers-reduced-motion: reduce) {
    .tech-summary-chevron {
      transition: none;
    }
  }
  .tech-summary-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .tech-summary-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text);
  }
  .tech-summary-subtitle {
    font-size: 11px;
    color: var(--color-text-muted);
  }
  .tech-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 4px 16px 16px;
    border-top: 1px solid var(--color-border-subtle);
  }

  /* ===== histogram rows ===== */
  .hist-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 16px;
    background: var(--color-surface);
  }
  .hist-line {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .hist-label {
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text);
  }
  .hist-name {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .hist-meta {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
  }
  .hist-caveat {
    font-size: 10px;
    font-style: italic;
    color: var(--color-text-muted);
    line-height: 1.4;
  }
  .hist-svg {
    width: 100%;
    height: 80px;
    color: var(--color-switch-on);
    margin-top: 6px;
  }
  .hist-axis {
    display: flex;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--color-text-muted);
    margin-top: 2px;
  }

  /* ===== events table ===== */
  .events-table-wrap {
    background: var(--color-surface);
    overflow-x: auto;
  }
  .events-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  .events-table th {
    text-align: left;
    padding: 8px 16px;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    border-bottom: 1px solid var(--color-border);
  }
  .events-table td {
    padding: 6px 16px;
    border-bottom: 1px solid var(--color-border-subtle);
    vertical-align: top;
  }
  .events-table tbody tr:last-child td {
    border-bottom: none;
  }
  .events-col-time {
    width: 96px;
  }
  .events-col-kind {
    width: 38%;
  }
  .events-table td:first-child {
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
  }
  .events-table code {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text);
  }
  .events-fields {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    word-break: break-all;
  }
  .events-empty {
    padding: 14px 16px;
    color: var(--color-text-muted);
    font-size: 12px;
    background: var(--color-surface);
  }

  .tech-actions {
    display: flex;
    justify-content: flex-end;
    padding-top: 4px;
  }
</style>
