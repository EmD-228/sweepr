<script lang="ts">
  import { FlaskConical, Trash2 } from "@lucide/svelte";
  import { app } from "$lib/app.svelte";
  import { formatBytes, plural } from "$lib/format";

  const unknown = $derived(app.selectedItems.some((i) => !i.size_known));
</script>

{#if app.selected.size > 0}
  <div class="border-t border-neutral-200 bg-white/90 px-8 py-3 backdrop-blur dark:border-neutral-800 dark:bg-neutral-900/90">
    <div class="mx-auto flex max-w-4xl items-center gap-3">
      <div class="flex-1">
        <span class="font-semibold">{plural(app.selected.size, "élément")}</span>
        <span class="text-neutral-500 dark:text-neutral-400">
          · environ {formatBytes(app.selectedSize)}{unknown ? " et plus" : ""}
        </span>
      </div>
      <button class="btn btn-ghost" onclick={() => app.selected.clear()}>Désélectionner</button>
      <button
        class="btn btn-outline"
        onclick={() => app.simulate()}
        disabled={app.scanning}
        title="Voir exactement ce qui serait supprimé, sans rien toucher"
      >
        <FlaskConical class="size-4" /> Simuler
      </button>
      <button
        class="btn btn-primary"
        onclick={() => (app.confirming = true)}
        disabled={app.scanning}
      >
        <Trash2 class="size-4" /> Nettoyer…
      </button>
    </div>
  </div>
{/if}
