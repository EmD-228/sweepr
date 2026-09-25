<script lang="ts">
  import { ChevronRight, Info, RotateCcw, TriangleAlert } from "@lucide/svelte";
  import { app, cleanable, type RuleGroup } from "$lib/app.svelte";
  import { formatBytes, plural } from "$lib/format";
  import GroupCheckbox from "./GroupCheckbox.svelte";
  import ItemRow from "./ItemRow.svelte";
  import RiskBadge from "./RiskBadge.svelte";

  let { group }: { group: RuleGroup } = $props();
  const { rule } = $derived(group);

  let open = $state(false);

  const sizeKnown = $derived(group.items.every((i) => i.size_known));
  const selectedCount = $derived(cleanable(group.items).filter((i) => app.selected.has(i.id)).length);
  const single = $derived(group.items.length === 1);
  // Risk 2 and 3: each item is ticked on its own (SPEC section 6).
  const groupSelect = $derived(single || rule.risk <= 1);
</script>

<section class="card">
  <div class="flex items-center gap-3 px-4 py-3">
    {#if groupSelect}
      <GroupCheckbox items={group.items} label={rule.title} />
    {:else}
      <span class="size-4 shrink-0"></span>
    {/if}

    <button class="flex min-w-0 flex-1 items-center gap-3 text-left" onclick={() => (open = !open)} aria-expanded={open}>
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <h3 class="truncate font-semibold">{rule.title}</h3>
          <RiskBadge risk={rule.risk} />
        </div>
        <p class="truncate text-sm text-neutral-500 dark:text-neutral-400">
          {#if single}
            {rule.summary}
          {:else}
            {plural(group.items.length, "élément")} · {rule.summary}
          {/if}
        </p>
      </div>
      <div class="shrink-0 text-right">
        <div class="font-semibold tabular-nums">{sizeKnown ? formatBytes(group.size) : "Gain inconnu"}</div>
        {#if selectedCount > 0 && !single}
          <div class="text-xs text-indigo-700 dark:text-indigo-300">{selectedCount} sélectionné{selectedCount > 1 ? "s" : ""}</div>
        {/if}
      </div>
      <ChevronRight class="size-4 shrink-0 text-neutral-400 transition-transform {open ? 'rotate-90' : ''}" />
    </button>
  </div>

  {#if open}
    <div class="border-t border-neutral-100 px-4 pt-3 pb-2 dark:border-neutral-800">
      <dl class="mb-2 grid gap-1.5 pl-7 text-sm">
        <div class="flex gap-2">
          <dt class="sr-only">Ce que vous perdez</dt>
          <Info class="mt-0.5 size-4 shrink-0 text-neutral-400" />
          <dd>{rule.loses}</dd>
        </div>
        <div class="flex gap-2">
          <dt class="sr-only">Pour revenir en arrière</dt>
          <RotateCcw class="mt-0.5 size-4 shrink-0 text-neutral-400" />
          <dd>{rule.regenerate}</dd>
        </div>
        {#if rule.note}
          <div class="flex gap-2 text-amber-800 dark:text-amber-400">
            <dt class="sr-only">À savoir</dt>
            <TriangleAlert class="mt-0.5 size-4 shrink-0" />
            <dd>{rule.note}</dd>
          </div>
        {/if}
      </dl>
      {#if !groupSelect}
        <p class="mb-1 pl-7 text-xs text-neutral-500 dark:text-neutral-400">Cochez chaque élément après l'avoir vérifié.</p>
      {/if}
      <div class="-mx-1">
        {#each group.items as item (item.id)}
          <ItemRow {item} />
        {/each}
      </div>
    </div>
  {/if}
</section>
