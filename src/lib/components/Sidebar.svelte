<script lang="ts">
  import { FolderGit2, HardDrive, LayoutGrid, LoaderCircle, RefreshCw, Wrench } from "@lucide/svelte";
  import { app, type View } from "$lib/app.svelte";
  import { formatBytes } from "$lib/format";
  import Logo from "./Logo.svelte";

  const entries = $derived([
    { view: "overview" as View, label: "Vue d'ensemble", icon: LayoutGrid, size: app.total("caches", "trash", "temporary", "personal") },
    ...(app.result?.developer
      ? [
          { view: "projects" as View, label: "Projets", icon: FolderGit2, size: app.total("projects") },
          { view: "tools" as View, label: "Outils développeur", icon: Wrench, size: app.total("developer") },
        ]
      : []),
  ]);
</script>

<aside class="flex w-56 shrink-0 flex-col border-r border-neutral-200 bg-neutral-100/60 dark:border-neutral-800 dark:bg-neutral-900/60">
  <div class="flex items-center gap-2.5 px-5 pt-6 pb-4">
    <Logo size={26} />
    <div class="text-lg font-semibold tracking-tight">Sweepr</div>
  </div>

  <nav class="flex flex-col gap-0.5 px-3" aria-label="Sections">
    {#each entries as entry (entry.view)}
      <button
        class="flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-sm transition-colors
          {app.view === entry.view
          ? 'bg-white font-medium shadow-sm ring-1 ring-neutral-200 dark:bg-neutral-800 dark:ring-neutral-700'
          : 'text-neutral-600 hover:bg-neutral-200/60 dark:text-neutral-400 dark:hover:bg-neutral-800/60'}"
        onclick={() => (app.view = entry.view)}
        aria-current={app.view === entry.view ? "page" : undefined}
      >
        <entry.icon class="size-4 shrink-0" />
        <span class="flex-1 truncate">{entry.label}</span>
        {#if app.result && entry.size > 0}
          <span class="text-xs text-neutral-500 tabular-nums dark:text-neutral-400">{formatBytes(entry.size)}</span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="mt-auto flex flex-col gap-3 p-4">
    {#if app.result?.free_space != null}
      <div class="flex items-center gap-2 text-sm text-neutral-600 dark:text-neutral-400">
        <HardDrive class="size-4 shrink-0" />
        <span><span class="font-medium text-neutral-900 dark:text-neutral-100">{formatBytes(app.result.free_space)}</span> libres</span>
      </div>
    {/if}
    {#if app.scanning}
      <button class="btn btn-outline py-2" onclick={() => app.cancelScan()}>
        <LoaderCircle class="size-4 animate-spin" />
        Arrêter l'analyse
      </button>
    {:else}
      <button class="btn btn-outline py-2" onclick={() => app.scan()} disabled={app.execution !== null}>
        <RefreshCw class="size-4" />
        {app.result ? "Analyser à nouveau" : "Analyser le disque"}
      </button>
    {/if}
  </div>
</aside>
