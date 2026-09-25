<script lang="ts">
  import { Info, RotateCcw, ShieldAlert, Trash } from "@lucide/svelte";
  import { app } from "$lib/app.svelte";
  import { formatBytes, formatRegenerate, plural, sumSize } from "$lib/format";
  import type { Item, Risk } from "$lib/types";
  import Modal from "./Modal.svelte";
  import RiskBadge from "./RiskBadge.svelte";

  interface Group {
    key: string;
    title: string;
    items: Item[];
    risk: Risk;
    loses: string;
    regenerate: string;
  }

  /** One group per rule, and one per project for project artifacts. */
  function newGroup(item: Item): Group {
    if (item.project) {
      const project = app.project(item.project);
      return {
        key: `project:${item.project}`,
        title: `Projet ${project?.name ?? ""}`,
        items: [],
        risk: item.risk,
        loses: "Les dépendances et les fichiers de build. Le code, git et les fichiers .env ne sont pas touchés.",
        regenerate: formatRegenerate(project),
      };
    }
    const rule = app.rule(item.rule);
    return {
      key: item.rule,
      title: rule?.title ?? item.title,
      items: [],
      risk: rule?.risk ?? item.risk,
      loses: rule?.loses ?? "",
      regenerate: rule?.regenerate ?? "",
    };
  }

  const groups = $derived.by(() => {
    const map = new Map<string, Group>();
    for (const item of app.selectedItems) {
      const key = item.project ? `project:${item.project}` : item.rule;
      const group = map.get(key) ?? newGroup(item);
      group.items.push(item);
      map.set(key, group);
    }
    return [...map.values()].sort((a, b) => b.risk - a.risk);
  });

  const irreplaceable = $derived(app.selectedItems.filter((i) => i.risk === 3));
  const trashed = $derived(app.selectedItems.filter((i) => i.disposal === "trash"));

  let step = $state<1 | 2>(1);
  let understood = $state(false);

  function next() {
    if (irreplaceable.length > 0 && step === 1) step = 2;
    else app.execute(irreplaceable.length > 0 && understood);
  }
</script>

<Modal
  title={step === 1 ? "Confirmer le nettoyage" : "Données irremplaçables"}
  subtitle={step === 1
    ? `${plural(app.selectedItems.length, "élément")}, environ ${formatBytes(app.selectedSize)} récupérés.`
    : "Deuxième confirmation, obligatoire pour ces éléments."}
  onclose={() => (app.confirming = false)}
>
  {#if step === 1}
    <div class="flex flex-col gap-4">
      {#each groups as group (group.key)}
        <div>
          <div class="flex items-center gap-2">
            <h3 class="flex-1 truncate font-semibold">{group.title}</h3>
            <RiskBadge risk={group.risk} />
            <span class="w-20 text-right font-semibold tabular-nums">{formatBytes(sumSize(group.items))}</span>
          </div>
          {#if group.items.length > 1 || group.key.startsWith("project:")}
            <p class="mt-0.5 truncate text-sm text-neutral-500 dark:text-neutral-400">{group.items.map((i) => i.title).join(", ")}</p>
          {/if}
          <div class="mt-1.5 grid gap-1 text-sm">
            <div class="flex gap-2"><Info class="mt-0.5 size-4 shrink-0 text-neutral-400" /><span>{group.loses}</span></div>
            <div class="flex gap-2"><RotateCcw class="mt-0.5 size-4 shrink-0 text-neutral-400" /><span>{group.regenerate}</span></div>
          </div>
        </div>
      {/each}

      {#if trashed.length > 0}
        <div class="flex gap-2 rounded-lg bg-neutral-100 px-3 py-2 text-sm dark:bg-neutral-800">
          <Trash class="mt-0.5 size-4 shrink-0 text-neutral-500" />
          <span>
            {plural(trashed.length, "élément")} à vérifier ou irremplaçable{trashed.length > 1 ? "s" : ""} sera déplacé dans la corbeille, pas supprimé. L'espace ne sera libéré qu'une fois la corbeille vidée.
          </span>
        </div>
      {/if}
    </div>
  {:else}
    <div class="flex flex-col gap-3">
      <div class="flex gap-3 rounded-lg border border-red-300/70 bg-red-50 px-4 py-3 text-sm text-red-900 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-200">
        <ShieldAlert class="mt-0.5 size-5 shrink-0" />
        <p>Ces données ne se recréent pas. Elles seront déplacées dans la corbeille : tant qu'elle n'est pas vidée, vous pouvez encore les récupérer.</p>
      </div>
      <ul class="flex flex-col gap-1 text-sm">
        {#each irreplaceable as item (item.id)}
          <li class="flex gap-2">
            <span class="flex-1 truncate">{item.title}</span>
            <span class="tabular-nums">{formatBytes(item.size)}</span>
          </li>
        {/each}
      </ul>
      <label class="mt-1 flex items-start gap-2 text-sm font-medium">
        <input type="checkbox" class="mt-0.5 size-4 accent-red-600" bind:checked={understood} />
        Je comprends que ces données ne pourront pas être recréées.
      </label>
    </div>
  {/if}

  {#snippet footer()}
    {#if step === 2}
      <button class="btn btn-ghost" onclick={() => (step = 1)}>Retour</button>
    {:else}
      <button class="btn btn-ghost" onclick={() => (app.confirming = false)}>Annuler</button>
    {/if}
    <button
      class="btn px-4 text-white {step === 2 ? 'bg-red-600 hover:bg-red-700' : 'btn-primary'}"
      disabled={step === 2 && !understood}
      onclick={next}
    >
      {#if step === 1 && irreplaceable.length > 0}
        Continuer
      {:else if step === 2}
        Déplacer dans la corbeille et nettoyer
      {:else}
        Nettoyer {formatBytes(app.selectedSize)}
      {/if}
    </button>
  {/snippet}
</Modal>
