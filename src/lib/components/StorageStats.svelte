<script lang="ts">
  import { app, type Bucket } from "$lib/app.svelte";
  import { formatBytes } from "$lib/format";
  import Donut, { type Segment } from "./Donut.svelte";

  /** Buckets in a fixed order: each keeps its color whatever its size. */
  const BUCKETS: { key: Bucket; label: string; color: string }[] = [
    { key: "caches", label: "Caches des applications", color: "var(--series-1)" },
    { key: "projects", label: "Projets", color: "var(--series-2)" },
    { key: "developer", label: "Outils développeur", color: "var(--series-3)" },
    { key: "trash", label: "Corbeille", color: "var(--series-4)" },
    { key: "personal", label: "Fichiers personnels", color: "var(--series-5)" },
    { key: "temporary", label: "Journaux et installateurs", color: "var(--series-6)" },
  ];

  const categories = $derived(BUCKETS.map((b) => ({ ...b, value: app.total(b.key) })).filter((s) => s.value > 0));
  const reclaimable = $derived(categories.reduce((s, c) => s + c.value, 0));

  const total = $derived(app.result?.total_space ?? null);
  const free = $derived(app.result?.free_space ?? null);
  const disk = $derived.by((): Segment[] => {
    if (total === null || free === null) return [];
    const used = Math.max(total - free, 0);
    return [
      { key: "reclaimable", label: "Récupérable par Sweepr", value: Math.min(reclaimable, used), color: "var(--chart-reclaimable)" },
      { key: "used", label: "Autres données", value: Math.max(used - reclaimable, 0), color: "var(--chart-other)" },
      { key: "free", label: "Libre", value: free, color: "var(--chart-track)" },
    ];
  });

  let activeDisk = $state<string | null>(null);
  let activeCategory = $state<string | null>(null);

  const percent = (value: number, of: number) => (of > 0 ? `${Math.round((value / of) * 100)} %` : "");
  const find = (list: Segment[], key: string | null) => list.find((s) => s.key === key) ?? null;
</script>

{#snippet legend(segments: Segment[], of: number, active: string | null, setActive: (key: string | null) => void)}
  <ul class="flex w-full min-w-0 flex-1 flex-col gap-0.5">
    {#each segments as segment (segment.key)}
      <li
        class="flex items-center gap-2.5 rounded-md px-2 py-1 text-sm transition-colors {active === segment.key ? 'bg-neutral-100 dark:bg-neutral-800' : ''}"
        onmouseenter={() => setActive(segment.key)}
        onmouseleave={() => setActive(null)}
      >
        <span class="size-2.5 shrink-0 rounded-full ring-1 ring-black/5 dark:ring-white/10" style="background: {segment.color}"></span>
        <span class="min-w-0 flex-1 truncate text-neutral-600 dark:text-neutral-400">{segment.label}</span>
        <span class="font-medium tabular-nums">{formatBytes(segment.value)}</span>
        <span class="w-10 text-right text-xs text-neutral-500 tabular-nums dark:text-neutral-400">{percent(segment.value, of)}</span>
      </li>
    {/each}
  </ul>
{/snippet}

<!-- Container queries: the layout follows the room the view really has, not the screen. -->
<div class="mb-6 grid grid-cols-1 gap-3 @3xl:grid-cols-2">
  {#if total !== null && free !== null}
    <section class="card @container p-5">
      <h2 class="font-semibold">Votre disque</h2>
      <p class="mb-4 text-sm text-neutral-500 dark:text-neutral-400">{formatBytes(total)} au total</p>
      <div class="flex flex-col items-center gap-5 @sm:flex-row">
        <Donut segments={disk} bind:active={activeDisk} size={148} thickness={14} label="Répartition de l'espace du disque">
          {#snippet center()}
            {@const hovered = find(disk, activeDisk)}
            <span class="text-xl font-semibold tabular-nums">{formatBytes(hovered?.value ?? free)}</span>
            <span class="max-w-24 text-xs leading-tight text-neutral-500 dark:text-neutral-400">{hovered?.label ?? "libres"}</span>
          {/snippet}
        </Donut>
        {@render legend(disk, total, activeDisk, (key) => (activeDisk = key))}
      </div>
    </section>
  {/if}

  <section class="card @container p-5">
    <h2 class="font-semibold">Ce que Sweepr peut récupérer</h2>
    <p class="mb-4 text-sm text-neutral-500 dark:text-neutral-400">
      {app.phase === "done" ? "Analyse complète" : "Se complète pendant l'analyse"}
    </p>
    <div class="flex flex-col items-center gap-5 @sm:flex-row">
      <Donut segments={categories} bind:active={activeCategory} size={148} thickness={14} label="Répartition de l'espace récupérable par catégorie">
        {#snippet center()}
          {@const hovered = find(categories, activeCategory)}
          <span class="text-xl font-semibold tabular-nums">{formatBytes(hovered?.value ?? reclaimable)}</span>
          <span class="max-w-24 text-xs leading-tight text-neutral-500 dark:text-neutral-400">{hovered?.label ?? "récupérables"}</span>
        {/snippet}
      </Donut>
      {#if categories.length > 0}
        {@render legend(categories, reclaimable, activeCategory, (key) => (activeCategory = key))}
      {:else}
        <p class="text-sm text-neutral-500 dark:text-neutral-400">Rien à récupérer pour le moment.</p>
      {/if}
    </div>
  </section>
</div>
