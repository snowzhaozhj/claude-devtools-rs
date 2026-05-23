<script lang="ts">
  // Diagnostics tab — pull-based telemetry snapshot dashboard.
  //
  // 详见 OpenSpec change `add-telemetry-signal-bus` D7 + spec settings-ui §
  // Diagnostics tab 暴露 telemetry 快照。
  //
  // 仅读不写：4 仪表盘卡片 + 2 延迟分布柱状图 + 最近 events + 复制按钮。
  // 不做轮询；用户主动点"刷新"按钮触发。

  import { onMount } from "svelte";
  import { getTelemetrySnapshot, type TelemetrySnapshot } from "../../lib/api";

  let snapshot = $state<TelemetrySnapshot | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let copyToast = $state<string | null>(null);

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
    // silent=true 模式：保留旧数据展示，新数据到达后 in-place 替换
    error = null;
    try {
      const next = await getTelemetrySnapshot();
      snapshot = next;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function copySnapshot() {
    if (!snapshot) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2));
      copyToast = "已复制完整 snapshot 到剪贴板";
      setTimeout(() => (copyToast = null), 2000);
    } catch (e) {
      copyToast = "复制失败";
      setTimeout(() => (copyToast = null), 2000);
      console.warn("[DiagnosticsTab] copy failed", e);
    }
  }

  // ---- Counter helpers ----
  function counterValue(name: string): number {
    return snapshot?.counters[name] ?? 0;
  }

  const cacheHitRate = $derived.by(() => {
    if (!snapshot) return null;
    const hit = counterValue("metadata.cache.hit");
    const miss = counterValue("metadata.cache.miss");
    const sigSkew = counterValue("metadata.cache.sig_mismatch");
    const statErr = counterValue("metadata.cache.stat_err");
    const total = hit + miss + sigSkew + statErr;
    return total > 0 ? hit / total : null;
  });

  const ipcErrorTotal = $derived.by(() => {
    if (!snapshot) return 0;
    return counterValue("cdt_api.error") + counterValue("cdt_api.warn");
  });

  const panicCount = $derived(counterValue("panic.recovered"));

  const sshReconnectCount = $derived(counterValue("ssh.reconnect"));

  const histogramNames = [
    "ipc.list_sessions.duration_ns",
    "ipc.get_session_detail.duration_ns",
  ];

  function fmtNs(n: number | null | undefined): string {
    if (n == null) return "—";
    if (n < 1000) return `${n} ns`;
    if (n < 1_000_000) return `${(n / 1000).toFixed(1)} μs`;
    if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(1)} ms`;
    return `${(n / 1_000_000_000).toFixed(2)} s`;
  }

  function maxBucketHeight(buckets: number[]): number {
    return Math.max(...buckets, 1);
  }

  function bucketRect(buckets: number[], i: number): { x: number; y: number; h: number; opacity: number } {
    const v = buckets[i] ?? 0;
    const max = maxBucketHeight(buckets);
    const h = (v / max) * 70;
    return { x: i * 10, y: 80 - h, h, opacity: v > 0 ? 0.85 : 0.15 };
  }
</script>

<div class="diagnostics">
  <header class="diag-header">
    <div>
      <h2 class="diag-title">应用健康度</h2>
      <p class="diag-desc">
        cdt-telemetry Phase 1 — Performance / Reliability / Correctness 信号快照。
      </p>
    </div>
    <div class="diag-actions">
      <button type="button" class="diag-btn" onclick={refresh} disabled={loading}>
        刷新
      </button>
      <button
        type="button"
        class="diag-btn diag-btn-primary"
        onclick={copySnapshot}
        disabled={!snapshot}
      >
        复制完整 snapshot
      </button>
    </div>
  </header>

  {#if copyToast}
    <div class="diag-toast" role="status" aria-live="polite">{copyToast}</div>
  {/if}

  {#if error}
    <div class="diag-error">无法加载 telemetry：{error}</div>
  {/if}

  {#if loading && !snapshot}
    <div class="diag-loading">加载中...</div>
  {/if}

  {#if snapshot}
    <section class="diag-cards">
      <article class="diag-card">
        <div class="diag-card-label">Metadata cache 命中率</div>
        <div class="diag-card-value">
          {cacheHitRate == null ? "—" : `${(cacheHitRate * 100).toFixed(1)}%`}
        </div>
        <div class="diag-card-sub">
          hit {counterValue("metadata.cache.hit")} / miss {counterValue("metadata.cache.miss")} /
          sig {counterValue("metadata.cache.sig_mismatch")} / stat_err {counterValue("metadata.cache.stat_err")}
        </div>
      </article>
      <article class="diag-card">
        <div class="diag-card-label">IPC 错误累计</div>
        <div class="diag-card-value">{ipcErrorTotal}</div>
        <div class="diag-card-sub">cdt_api.error + cdt_api.warn</div>
      </article>
      <article class="diag-card">
        <div class="diag-card-label">Panic 计数</div>
        <div class="diag-card-value">{panicCount}</div>
        <div class="diag-card-sub">本进程启动后累计</div>
      </article>
      <article class="diag-card">
        <div class="diag-card-label">SSH 重连次数</div>
        <div class="diag-card-value">{sshReconnectCount}</div>
        <div class="diag-card-sub">ssh.reconnect 累计</div>
      </article>
    </section>

    <section class="diag-histograms">
      {#each histogramNames as name (name)}
        {@const h = snapshot.histograms[name]}
        {#if h}
          <article class="diag-hist">
            <h3 class="diag-hist-name">{name}</h3>
            <div class="diag-hist-meta">
              count={h.count} ·
              p50≤{fmtNs(h.p50Ns)} · p95≤{fmtNs(h.p95Ns)} · p99≤{fmtNs(h.p99Ns)}
            </div>
            <p class="diag-hist-hint">
              power-of-2 bucket upper bound（实际值 ≤ 此值，最坏 2x 偏差）
            </p>
            <svg class="diag-hist-svg" viewBox="0 0 320 80" preserveAspectRatio="none" aria-hidden="true">
              {#each h.buckets as _, i (i)}
                {@const r = bucketRect(h.buckets, i)}
                <rect x={r.x} y={r.y} width="9" height={r.h} fill="currentColor" opacity={r.opacity} />
              {/each}
            </svg>
            <div class="diag-hist-axis">
              <span>1 ns</span>
              <span>1 μs</span>
              <span>1 ms</span>
              <span>1 s</span>
            </div>
          </article>
        {/if}
      {/each}
    </section>

    <section class="diag-events">
      <h3 class="diag-events-title">最近 events ({snapshot.recentEvents.length})</h3>
      {#if snapshot.recentEvents.length === 0}
        <p class="diag-events-empty">尚无 event。</p>
      {:else}
        <table class="diag-events-table">
          <thead>
            <tr>
              <th>时间</th>
              <th>kind</th>
              <th>fields</th>
            </tr>
          </thead>
          <tbody>
            {#each snapshot.recentEvents.slice(-50).reverse() as ev (ev.tsUnixMs + ev.kind)}
              <tr>
                <td>{new Date(ev.tsUnixMs).toLocaleTimeString()}</td>
                <td><code>{ev.kind}</code></td>
                <td class="diag-events-fields">
                  {Object.entries(ev.fields).map(([k, v]) => `${k}=${v}`).join(", ")}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/if}
</div>

<style>
  .diagnostics {
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }
  .diag-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
  }
  .diag-title {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 600;
  }
  .diag-desc {
    margin: 0.25rem 0 0;
    color: var(--text-secondary, #888);
    font-size: 0.85rem;
  }
  .diag-actions {
    display: flex;
    gap: 0.5rem;
  }
  .diag-btn {
    padding: 0.4rem 0.75rem;
    border: 1px solid var(--border, #ccc);
    background: var(--surface, transparent);
    color: inherit;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .diag-btn:hover:not(:disabled) {
    background: var(--surface-hover, #f5f5f5);
  }
  .diag-btn-primary {
    background: var(--accent, #2563eb);
    color: var(--accent-foreground, #fff);
    border-color: transparent;
  }
  .diag-btn-primary:hover:not(:disabled) {
    background: var(--accent-hover, #1d4ed8);
  }
  .diag-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .diag-toast {
    background: var(--surface-success, #d1fae5);
    border: 1px solid var(--border-success, #6ee7b7);
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    font-size: 0.85rem;
  }
  .diag-error {
    background: var(--surface-error, #fef2f2);
    border: 1px solid var(--border-error, #fca5a5);
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    font-size: 0.85rem;
    color: var(--text-error, #991b1b);
  }
  .diag-loading {
    color: var(--text-secondary, #888);
    font-size: 0.9rem;
  }
  .diag-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }
  .diag-card {
    border: 1px solid var(--border, #ddd);
    border-radius: 6px;
    padding: 0.75rem 1rem;
    background: var(--surface, transparent);
  }
  .diag-card-label {
    font-size: 0.8rem;
    color: var(--text-secondary, #888);
  }
  .diag-card-value {
    font-size: 1.5rem;
    font-weight: 600;
    margin-top: 0.25rem;
  }
  .diag-card-sub {
    font-size: 0.75rem;
    color: var(--text-secondary, #888);
    margin-top: 0.25rem;
  }
  .diag-histograms {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .diag-hist {
    border: 1px solid var(--border, #ddd);
    border-radius: 6px;
    padding: 0.75rem 1rem;
    background: var(--surface, transparent);
  }
  .diag-hist-name {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 600;
    font-family: var(--font-mono, monospace);
  }
  .diag-hist-meta {
    margin-top: 0.25rem;
    font-size: 0.8rem;
    color: var(--text-secondary, #888);
  }
  .diag-hist-hint {
    margin: 0.25rem 0;
    font-size: 0.75rem;
    color: var(--text-tertiary, #aaa);
    font-style: italic;
  }
  .diag-hist-svg {
    width: 100%;
    height: 80px;
    color: var(--accent, #2563eb);
    margin-top: 0.5rem;
  }
  .diag-hist-axis {
    display: flex;
    justify-content: space-between;
    font-size: 0.7rem;
    color: var(--text-secondary, #888);
    margin-top: 0.25rem;
  }
  .diag-events-title {
    margin: 0 0 0.5rem;
    font-size: 0.9rem;
    font-weight: 600;
  }
  .diag-events-empty {
    color: var(--text-secondary, #888);
    font-size: 0.85rem;
    margin: 0;
  }
  .diag-events-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }
  .diag-events-table th,
  .diag-events-table td {
    text-align: left;
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid var(--border, #eee);
  }
  .diag-events-fields {
    font-family: var(--font-mono, monospace);
    font-size: 0.78rem;
    color: var(--text-secondary, #555);
  }
</style>
