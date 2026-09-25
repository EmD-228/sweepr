<script lang="ts" module>
  export interface Segment {
    key: string;
    label: string;
    value: number;
    /** CSS color, usually a `var(--series-n)` token. */
    color: string;
  }
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    segments,
    size = 176,
    thickness = 16,
    label,
    active = $bindable(null),
    center,
  }: {
    segments: Segment[];
    size?: number;
    thickness?: number;
    /** Accessible description of the whole chart. */
    label: string;
    /** Key of the hovered segment, shared with the legend. */
    active?: string | null;
    center?: Snippet;
  } = $props();

  // Surface-colored gap between segments, in pixels along the ring.
  const GAP = 2;

  const radius = $derived((size - thickness) / 2);
  const circumference = $derived(2 * Math.PI * radius);
  const total = $derived(segments.reduce((s, x) => s + Math.max(x.value, 0), 0));

  const arcs = $derived.by(() => {
    const visible = segments.filter((s) => s.value > 0);
    const gap = visible.length > 1 ? GAP : 0;
    let offset = 0;
    return visible.map((segment) => {
      const length = total > 0 ? (segment.value / total) * circumference : 0;
      const drawn = Math.max(length - gap, 0.5);
      const arc = { segment, dash: `${drawn} ${circumference - drawn}`, offset: -offset };
      offset += length;
      return arc;
    });
  });
</script>

<div class="relative shrink-0" style="width: {size}px; height: {size}px">
  <svg width={size} height={size} viewBox="0 0 {size} {size}" role="img" aria-label={label} class="-rotate-90">
    <circle cx={size / 2} cy={size / 2} r={radius} fill="none" stroke="var(--chart-track)" stroke-width={thickness} />
    {#each arcs as arc (arc.segment.key)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke={arc.segment.color}
        stroke-width={active === arc.segment.key ? thickness + 4 : thickness}
        stroke-dasharray={arc.dash}
        stroke-dashoffset={arc.offset}
        opacity={active && active !== arc.segment.key ? 0.35 : 1}
        class="cursor-default transition-[opacity,stroke-width] duration-150"
        onmouseenter={() => (active = arc.segment.key)}
        onmouseleave={() => (active = null)}
      />
    {/each}
  </svg>
  {#if center}
    <div class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
      {@render center()}
    </div>
  {/if}
</div>
