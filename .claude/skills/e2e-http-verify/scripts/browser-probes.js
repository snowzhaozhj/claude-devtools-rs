/**
 * 浏览器侧 e2e 探针模板集合。
 *
 * 用法：在 chrome-devtools mcp `evaluate_script` 里粘对应模板（async wrapper 已带）。
 * 所有模板都 JIT 从真返回 keys 读字段名，避免 schema 漂移。
 *
 * Templates:
 *   T1 sseOpenLatency()       - 测 SSE OPEN 时间（应 < 100ms，回归判定）
 *   T2 listGroupsLatency()    - 测 list_group_sessions IPC P50 / P95（n=5）
 *   T3 clickProjectLatency()  - 测点击 dashboard project card → fetch 完成 wall time
 *   T4 networkOverFetch()     - 列 windowSec（默认 5s）内按 {path, cursor/pageSize} 维度分组 fetch
 *                                count > threshold（默认 3）标 suspect 让人判断（不直接 fail）
 *   T5 sessionDetailReady()   - 点开 session → 等 chunks DOM 出现 → 报 chunk 数 / role 分布
 *   T6 collectConsoleErrors() - 装 console.error 监听 N 秒，返回所有错误
 *   T7 openSessionViaTest()   - 用 window.__cdtTest.openTab 绕过 sidebar virtualization
 *
 * 设计原则：
 *   - 每个模板独立、可粘可跑、不依赖前一个的 state
 *   - 不写死字段名，先 fetch 一次拿真 schema 再做断言
 *   - DOM selector 先 take_snapshot 看真 class，再 evaluate_script 用稳定 attr
 *     （sidebar session button 已加 `data-session-id` / `data-project-id`）
 */

// ===== T1 sseOpenLatency =====
async () => {
  const t0 = performance.now();
  return new Promise((resolve) => {
    const es = new EventSource('/api/events');
    es.addEventListener('open', () => {
      const ms = Math.round(performance.now() - t0);
      es.close();
      resolve({ sse_open_ms: ms, verdict: ms < 100 ? 'PASS' : 'FAIL (vite proxy 缓冲？检查 sse.rs prelude)' });
    }, { once: true });
    setTimeout(() => { es.close(); resolve({ timeout: true, elapsed_ms: 3000 }); }, 3000);
  });
}

// ===== T2 listGroupsLatency =====
async ({ groupName = null, n = 5, pageSize = 50 } = {}) => {
  const groups = await fetch('/api/repository-groups').then((r) => r.json());
  const g = groupName ? groups.find((x) => (x.name || '').includes(groupName)) : groups[0];
  if (!g) return { error: `group not found: ${groupName}`, available: groups.map((x) => x.name) };
  const id = encodeURIComponent(g.id);
  const samples = [];
  for (let i = 0; i < n; i++) {
    const t0 = performance.now();
    const j = await fetch(`/api/repository-groups/${id}/sessions?pageSize=${pageSize}`).then((r) => r.json());
    samples.push({
      ms: Math.round(performance.now() - t0),
      n: j.sessions?.length,
      titled: (j.sessions || []).filter((s) => s.title).length,
    });
  }
  const sorted = samples.map((s) => s.ms).sort((a, b) => a - b);
  return {
    target: `${g.name} (${g.totalSessions} sessions, ${g.worktrees?.length} worktrees)`,
    samples,
    p50_ms: sorted[Math.floor(n / 2)],
    p95_ms: sorted[Math.min(n - 1, Math.floor(n * 0.95))],
  };
}

// ===== T3 clickProjectLatency =====
async ({ projectNameContains = null } = {}) => {
  const cards = Array.from(document.querySelectorAll('button')).filter((b) =>
    /💬/.test(b.textContent || '') && !/当前/.test(b.textContent || ''),
  );
  const target = projectNameContains
    ? cards.find((c) => (c.textContent || '').includes(projectNameContains))
    : cards[0];
  if (!target) return { error: 'no clickable project card', candidates: cards.length };
  const fetches = [];
  const obs = new PerformanceObserver((list) => {
    for (const e of list.getEntries()) {
      if (e.initiatorType === 'fetch' && e.name.includes('/api/')) {
        fetches.push({ url: e.name.replace(location.origin, ''), duration_ms: Math.round(e.duration) });
      }
    }
  });
  obs.observe({ type: 'resource', buffered: false });
  const t0 = performance.now();
  target.click();
  await new Promise((r) => setTimeout(r, 2000));
  obs.disconnect();
  return {
    clicked: target.textContent?.replace(/\s+/g, ' ').slice(0, 50),
    wall_total_ms: 2000,
    total_fetches: fetches.length,
    total_ipc_ms: fetches.reduce((s, f) => s + f.duration_ms, 0),
    by_fetch: fetches,
  };
}

// ===== T4 networkOverFetch =====
// 收集窗口内 fetch，按 `{path, cursor/pageSize}` 维度分组——纯按 path 折叠会
// 把不同 page 的 loadMore 都算重复，误报严重（codex PR 二审 issue 5）。
// suspects 是**嫌疑**不是 fail——SSE recovered refresh / 用户滚动 / 初始批量
// 加载可能合理产生 > threshold；结合用户操作判断是否真 over-fetch。
async ({ windowSec = 5, threshold = 3 } = {}) => {
  const seen = new Map();
  const groupKey = (urlNoOrigin) => {
    const [path, query = ''] = urlNoOrigin.split('?');
    const qs = new URLSearchParams(query);
    // 保留区分批次的关键 query；忽略 cache-buster 噪音
    const keep = ['cursor', 'pageSize', 'page', 'offset', 'limit'];
    const parts = keep
      .filter((k) => qs.has(k))
      .map((k) => `${k}=${qs.get(k)}`)
      .sort();
    return parts.length ? `${path}?${parts.join('&')}` : path;
  };
  const obs = new PerformanceObserver((list) => {
    for (const e of list.getEntries()) {
      if (e.initiatorType === 'fetch' && e.name.includes('/api/')) {
        const key = groupKey(e.name.replace(location.origin, ''));
        seen.set(key, (seen.get(key) || 0) + 1);
      }
    }
  });
  obs.observe({ type: 'resource', buffered: false });
  await new Promise((r) => setTimeout(r, windowSec * 1000));
  obs.disconnect();
  const sorted = [...seen.entries()].sort((a, b) => b[1] - a[1]);
  return {
    window_sec: windowSec,
    threshold,
    note: 'suspects 是嫌疑，不是 fail；结合用户操作判断（滚动/初始批量加载可能合理产生 > threshold）',
    suspects: sorted.filter(([, n]) => n > threshold).map(([url, n]) => ({ url, count: n })),
    all: Object.fromEntries(sorted),
  };
}

// ===== T5 sessionDetailReady =====
async ({ sessionId = null, waitMs = 1500 } = {}) => {
  if (sessionId) {
    const btn = document.querySelector(`[data-session-id="${sessionId}"]`);
    if (!btn) return { error: `no sidebar button [data-session-id="${sessionId}"] — 不在可见范围？先 scroll` };
    btn.click();
  }
  await new Promise((r) => setTimeout(r, waitMs));
  const rows = document.querySelectorAll('.msg-row');
  return {
    chunk_count: rows.length,
    by_role: Array.from(rows).reduce((acc, r) => {
      const role = r.classList.contains('msg-row-user')
        ? 'user'
        : r.classList.contains('msg-row-ai')
          ? 'ai'
          : 'other';
      acc[role] = (acc[role] || 0) + 1;
      return acc;
    }, {}),
    sample_preview: Array.from(rows).slice(0, 3).map((r) => r.textContent?.replace(/\s+/g, ' ').slice(0, 60)),
  };
}

// ===== T6 collectConsoleErrors =====
async ({ windowSec = 3 } = {}) => {
  const errs = [];
  const warns = [];
  const origErr = console.error;
  const origWarn = console.warn;
  console.error = (...a) => { errs.push(a.map((x) => String(x)).join(' ').slice(0, 200)); origErr.apply(console, a); };
  console.warn = (...a) => { warns.push(a.map((x) => String(x)).join(' ').slice(0, 200)); origWarn.apply(console, a); };
  await new Promise((r) => setTimeout(r, windowSec * 1000));
  console.error = origErr;
  console.warn = origWarn;
  return { window_sec: windowSec, errors: errs, warnings: warns };
}

// ===== T7 openSessionViaTest =====
async ({ sessionId, projectId, label = 'e2e-probe' }) => {
  if (!window.__cdtTest?.openTab) {
    return { error: '__cdtTest.openTab unavailable — main.ts 修法未生效？check `?http=1` 路径下 helper 注入' };
  }
  window.__cdtTest.openTab(sessionId, projectId, label);
  await new Promise((r) => setTimeout(r, 1200));
  const rows = document.querySelectorAll('.msg-row');
  return { ok: true, chunk_count: rows.length };
}
