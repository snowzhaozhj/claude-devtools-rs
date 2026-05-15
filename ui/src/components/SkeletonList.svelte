<!--
  SkeletonList — 列表型 loading 骨架。
  外层 role="status" + aria-busy="true"，让 screen reader 知道正在加载。
-->

<script lang="ts">
  import Skeleton from "./Skeleton.svelte";

  interface Props {
    count?: number;
    rowHeight?: number;
    gap?: number;
    padding?: string;
    variant?: "row" | "card";
    label?: string;
  }
  let {
    count = 6,
    rowHeight = 32,
    gap = 6,
    padding = "8px",
    variant = "row",
    label = "加载中",
  }: Props = $props();
</script>

<div
  class="skel-list"
  style:gap="{gap}px"
  style:padding
  role="status"
  aria-busy="true"
  aria-label={label}
>
  {#each Array.from({ length: count }) as _, i (i)}
    <Skeleton {variant} height={rowHeight} />
  {/each}
</div>

<style>
  .skel-list {
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
  }
</style>
