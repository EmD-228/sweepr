<script lang="ts">
  import type { Snippet } from "svelte";
  import { X } from "@lucide/svelte";

  let {
    title,
    subtitle,
    onclose,
    closable = true,
    children,
    footer,
  }: {
    title: string;
    subtitle?: string;
    onclose: () => void;
    closable?: boolean;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  let dialog: HTMLDialogElement;

  $effect(() => {
    dialog.showModal();
    return () => dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  class="m-auto flex max-h-[85vh] w-[min(640px,calc(100vw-48px))] flex-col overflow-hidden rounded-2xl border border-neutral-200 bg-white p-0 text-neutral-900 shadow-2xl backdrop:bg-black/40 dark:border-neutral-800 dark:bg-neutral-900 dark:text-neutral-100"
  oncancel={(e) => {
    e.preventDefault();
    if (closable) onclose();
  }}
>
  <header class="flex items-start gap-4 border-b border-neutral-200 px-6 py-4 dark:border-neutral-800">
    <div class="min-w-0 flex-1">
      <h2 class="text-base font-semibold">{title}</h2>
      {#if subtitle}
        <p class="mt-0.5 text-sm text-neutral-500 dark:text-neutral-400">{subtitle}</p>
      {/if}
    </div>
    {#if closable}
      <button
        class="-mr-2 rounded-md p-1.5 text-neutral-500 hover:bg-neutral-100 hover:text-neutral-800 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
        onclick={onclose}
        aria-label="Fermer"
      >
        <X class="size-4" />
      </button>
    {/if}
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto px-6 py-4">
    {@render children()}
  </div>

  {#if footer}
    <footer class="flex items-center justify-end gap-2 border-t border-neutral-200 px-6 py-3 dark:border-neutral-800">
      {@render footer()}
    </footer>
  {/if}
</dialog>
