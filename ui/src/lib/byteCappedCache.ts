/**
 * 带 byte cap 的 LRU 缓存：count 与 byte 双闸门，任一超限都从最旧端淘汰。
 *
 * 动机见 `.claude/rules/perf.md`「cache 仅设 count cap 不设 byte cap」反模式——
 * key/value 含完整源码或 diff 内容时，单条可达数 MB，只设 count cap 会内存爆涨。
 * `OutputBlock` / `DiffViewer` 共用本实现，避免各写一份易漂移的淘汰逻辑。
 */
export interface ByteCappedCacheOptions<V> {
  /** 最大条目数。 */
  maxEntries: number;
  /** 最大累计字节数（估算值）。 */
  maxBytes: number;
  /** 估算单条 entry（key + value）的字节数。 */
  sizeOf: (key: string, value: V) => number;
}

export class ByteCappedCache<V> {
  private readonly map = new Map<string, V>();
  // 每个 key 入账时实际累加的字节数。覆写 / 淘汰一律扣这个记录值，
  // 不重算 sizeOf——否则 sizeOf 非纯（读外部可变态）时 byteSize 会与真实内容漂移。
  private readonly sizes = new Map<string, number>();
  private bytes = 0;

  constructor(private readonly opts: ByteCappedCacheOptions<V>) {}

  /** 命中则 LRU touch（移到最新端）并返回；未命中返回 undefined。 */
  get(key: string): V | undefined {
    const hit = this.map.get(key);
    if (hit === undefined) return undefined;
    this.map.delete(key);
    this.map.set(key, hit);
    return hit;
  }

  /** 写入并按双闸门淘汰；单条超 maxBytes 时会清空其余条目后仍存入该条。 */
  set(key: string, value: V): void {
    const existingSize = this.sizes.get(key);
    if (existingSize !== undefined) {
      this.bytes -= existingSize;
      this.map.delete(key);
      this.sizes.delete(key);
    }
    const size = this.opts.sizeOf(key, value);
    while (
      this.map.size > 0 &&
      (this.map.size >= this.opts.maxEntries || this.bytes + size > this.opts.maxBytes)
    ) {
      this.evictOldest();
    }
    this.map.set(key, value);
    this.sizes.set(key, size);
    this.bytes += size;
  }

  private evictOldest(): void {
    const first = this.map.keys().next().value;
    if (first === undefined) return;
    this.map.delete(first);
    const evictedSize = this.sizes.get(first);
    this.sizes.delete(first);
    if (evictedSize !== undefined) this.bytes -= evictedSize;
  }

  /** 当前条目数。 */
  get size(): number {
    return this.map.size;
  }

  /** 当前累计字节数。 */
  get byteSize(): number {
    return this.bytes;
  }
}
