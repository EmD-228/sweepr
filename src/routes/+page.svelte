<script lang="ts">
  import { onMount } from "svelte";
  import { CircleX, HardDrive, LoaderCircle } from "@lucide/svelte";
  import { app } from "$lib/app.svelte";
  import { listenEngine } from "$lib/api";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import DevTools from "$lib/components/DevTools.svelte";
  import ExecutionDialog from "$lib/components/ExecutionDialog.svelte";
  import Overview from "$lib/components/Overview.svelte";
  import Projects from "$lib/components/Projects.svelte";
  import SelectionBar from "$lib/components/SelectionBar.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import SimulationDialog from "$lib/components/SimulationDialog.svelte";

  onMount(() => {
    const ready = listenEngine(app);
    // The first scan starts right away: it only reads the disk.
    ready.then(() => app.scan());
    return () => {
      ready.then((unlisten) => unlisten());
    };
  });
</script>

<div class="flex h-screen">
  <Sidebar />

  <main class="flex min-w-0 flex-1 flex-col">
    {#if app.error}
      <div class="flex items-start gap-2 border-b border-red-200 bg-red-50 px-8 py-2.5 text-sm text-red-800 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-300">
        <CircleX class="mt-0.5 size-4 shrink-0" />
        <span class="flex-1">{app.error}</span>
        <button class="font-medium hover:underline" onclick={() => (app.error = null)}>Fermer</button>
      </div>
    {/if}

    <div class="min-h-0 flex-1 overflow-y-auto">
      {#if !app.result}
        <div class="flex h-full flex-col items-center justify-center gap-3 px-8 text-center">
          {#if app.scanning}
            <LoaderCircle class="size-8 animate-spin text-neutral-400" />
            <h1 class="text-lg font-semibold">Analyse du disque</h1>
            <p class="max-w-sm text-neutral-500 dark:text-neutral-400">Sweepr lit votre disque sans rien modifier. Les premiers résultats arrivent dans quelques secondes.</p>
          {:else}
            <HardDrive class="size-8 text-neutral-400" />
            <h1 class="text-lg font-semibold">Voyons ce qui occupe votre disque</h1>
            <p class="max-w-sm text-neutral-500 dark:text-neutral-400">L'analyse ne supprime rien. Vous choisirez ensuite quoi nettoyer, avec le détail de chaque action.</p>
            <button class="btn btn-primary mt-2 py-2" onclick={() => app.scan()}>
              Analyser le disque
            </button>
          {/if}
        </div>
      {:else if app.view === "projects"}
        <Projects />
      {:else if app.view === "tools"}
        <DevTools />
      {:else}
        <Overview />
      {/if}
    </div>

    <SelectionBar />
  </main>
</div>

{#if app.confirming}
  <ConfirmDialog />
{/if}
{#if app.previews}
  <SimulationDialog previews={app.previews} />
{/if}
{#if app.execution}
  <ExecutionDialog execution={app.execution} />
{/if}
