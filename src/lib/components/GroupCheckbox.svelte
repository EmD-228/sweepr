<script lang="ts">
  import { app, cleanable } from "$lib/app.svelte";
  import type { Item } from "$lib/types";

  /** Selects or clears every cleanable item of a group at once. */
  let { items, label }: { items: Item[]; label: string } = $props();

  const selectable = $derived(cleanable(items));
  const count = $derived(selectable.filter((i) => app.selected.has(i.id)).length);
</script>

<input
  type="checkbox"
  class="size-4 shrink-0 accent-indigo-600"
  checked={count > 0 && count === selectable.length}
  indeterminate={count > 0 && count < selectable.length}
  disabled={selectable.length === 0}
  onchange={(e) => app.setAll(selectable, e.currentTarget.checked)}
  aria-label="Sélectionner {label}"
/>
