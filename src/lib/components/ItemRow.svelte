<script lang="ts">
  import { FolderOpen, Lock } from "@lucide/svelte";
  import { app } from "$lib/app.svelte";
  import { reveal } from "$lib/api";
  import { formatBytes } from "$lib/format";
  import type { Item } from "$lib/types";

  let { item, showDetail = true }: { item: Item; showDetail?: boolean } = $props();

  const checked = $derived(app.selected.has(item.id));
  const canReveal = $derived(item.disposal !== null);
</script>

<div
  class="group flex items-center gap-3 rounded-lg px-3 py-2 transition-colors
    {item.blocked ? 'opacity-70' : 'hover:bg-neutral-100 dark:hover:bg-neutral-800/60'}"
>
  {#if item.blocked}
    <Lock class="size-4 shrink-0 text-neutral-400" aria-label="Protégé" />
  {:else}
    <input
      type="checkbox"
      class="size-4 shrink-0 accent-indigo-600"
      {checked}
      onchange={(e) => app.toggle(item, e.currentTarget.checked)}
      aria-label="Sélectionner {item.title}"
    />
  {/if}

  <div class="min-w-0 flex-1">
    <div class="truncate font-medium">{item.title}</div>
    {#if item.blocked}
      <div class="text-xs text-neutral-500 dark:text-neutral-400">{item.blocked}</div>
    {:else if showDetail && item.detail}
      <div class="selectable truncate font-mono text-xs text-neutral-500 dark:text-neutral-400" title={item.detail}>
        {item.detail}
      </div>
    {/if}
  </div>

  {#if canReveal}
    <button
      class="rounded-md p-1 text-neutral-400 opacity-0 transition-opacity group-hover:opacity-100 hover:text-neutral-700 focus-visible:opacity-100 dark:hover:text-neutral-200"
      onclick={() => reveal(item.id)}
      title="Afficher dans le Finder"
      aria-label="Afficher {item.title} dans le Finder"
    >
      <FolderOpen class="size-4" />
    </button>
  {/if}

  <div class="w-20 shrink-0 text-right text-sm tabular-nums {item.size_known ? '' : 'text-neutral-400'}">
    {item.size_known ? formatBytes(item.size) : "inconnu"}
  </div>
</div>
