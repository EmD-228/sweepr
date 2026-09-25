<script lang="ts">
  import { CircleMinus, Terminal, Trash, Trash2 } from "@lucide/svelte";
  import { app } from "$lib/app.svelte";
  import { plural } from "$lib/format";
  import type { Preview } from "$lib/types";
  import Modal from "./Modal.svelte";

  let { previews }: { previews: Preview[] } = $props();

  const title = (id: string) => app.itemsById.get(id)?.title ?? id;
  const actionCount = $derived(previews.reduce((n, p) => n + p.actions.length, 0));
  const skippedCount = $derived(previews.reduce((n, p) => n + p.skipped.length, 0));
</script>

<Modal
  title="Simulation"
  subtitle="Voici exactement ce que le nettoyage toucherait. Rien n'a été supprimé."
  onclose={() => (app.previews = null)}
>
  <p class="mb-4 text-sm">
    {plural(actionCount, "action")}{#if skippedCount > 0}, {plural(skippedCount, "élément ignoré", "éléments ignorés")}{/if}.
  </p>
  <div class="flex flex-col gap-4">
    {#each previews as preview (preview.id)}
      <div>
        <h3 class="mb-1 font-semibold">{title(preview.id)}</h3>
        <ul class="flex flex-col gap-1">
          {#each preview.actions as action}
            <li class="flex items-start gap-2 text-sm">
              {#if preview.disposal === null}
                <Terminal class="mt-0.5 size-4 shrink-0 text-neutral-400" aria-label="Commande" />
              {:else if preview.disposal === "trash"}
                <Trash class="mt-0.5 size-4 shrink-0 text-neutral-400" aria-label="Déplacé dans la corbeille" />
              {:else}
                <Trash2 class="mt-0.5 size-4 shrink-0 text-neutral-400" aria-label="Supprimé" />
              {/if}
              <span class="selectable font-mono text-xs break-all">{action}</span>
            </li>
          {/each}
          {#each preview.skipped as reason}
            <li class="flex items-start gap-2 text-sm text-amber-800 dark:text-amber-400">
              <CircleMinus class="mt-0.5 size-4 shrink-0" aria-label="Ignoré" />
              <span class="selectable text-xs break-all">{reason}</span>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </div>

  {#snippet footer()}
    <button class="btn btn-ghost" onclick={() => (app.previews = null)}>Fermer</button>
    <button
      class="btn btn-primary"
      onclick={() => {
        app.previews = null;
        app.confirming = true;
      }}
    >
      Continuer vers la confirmation
    </button>
  {/snippet}
</Modal>
