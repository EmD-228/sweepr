<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { formatBytes, sumSize } from "$lib/format";
  import RuleCard from "./RuleCard.svelte";

  const groups = $derived(app.groupsFor("developer"));
</script>

<div class="view">
  <header class="mb-6">
    <h1 class="text-2xl font-semibold tracking-tight">Outils développeur</h1>
    <p class="mt-1 text-neutral-500 dark:text-neutral-400">
      {formatBytes(sumSize(groups))} de caches, SDK et outils. Les commandes officielles sont utilisées quand elles existent.
    </p>
  </header>

  <div class="flex flex-col gap-3">
    {#each groups as group (group.rule.id)}
      <RuleCard {group} />
    {:else}
      <p class="rounded-xl border border-dashed border-neutral-300 px-4 py-8 text-center text-neutral-500 dark:border-neutral-700">
        Aucun cache d'outil de développement trouvé.
      </p>
    {/each}
  </div>
</div>
