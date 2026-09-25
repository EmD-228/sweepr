<script lang="ts">
  import { ArrowRight, FolderGit2, ShieldAlert, Wrench } from "@lucide/svelte";
  import { app, cleanable } from "$lib/app.svelte";
  import { openFullDiskAccessSettings } from "$lib/api";
  import { formatBytes, sumSize } from "$lib/format";
  import RuleCard from "./RuleCard.svelte";
  import StorageStats from "./StorageStats.svelte";

  const groups = $derived(app.groupsFor("general"));
  const safeItems = $derived(cleanable(groups.flatMap((g) => g.items)).filter((i) => i.risk === 0));
  const allSafeSelected = $derived(app.allSelected(safeItems));
</script>

<div class="view">
  <header class="mb-6 flex flex-col items-start gap-3 @2xl:flex-row @2xl:items-end @2xl:justify-between @2xl:gap-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Vue d'ensemble</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        {#if app.phase === "overview" || (app.scanning && !app.result)}
          Analyse en cours. Les premiers résultats s'affichent au fur et à mesure.
        {:else}
          {formatBytes(sumSize(groups))} récupérables, triés par gain possible. Chaque action est expliquée avant d'être lancée.
        {/if}
      </p>
    </div>
    {#if safeItems.length > 0}
      <button class="btn btn-outline shrink-0" onclick={() => app.setAll(safeItems, !allSafeSelected)}>
        {allSafeSelected ? "Tout désélectionner" : `Sélectionner le sans-risque (${formatBytes(sumSize(safeItems))})`}
      </button>
    {/if}
  </header>

  <StorageStats />

  {#if app.result?.full_disk_access === false}
    <div class="mb-4 flex items-start gap-3 rounded-xl border border-amber-300/70 bg-amber-50 px-4 py-3 dark:border-amber-500/30 dark:bg-amber-500/10">
      <ShieldAlert class="mt-0.5 size-5 shrink-0 text-amber-700 dark:text-amber-400" />
      <div class="flex-1 text-sm">
        <p class="font-medium text-amber-900 dark:text-amber-200">Sweepr ne voit pas tout votre disque</p>
        <p class="mt-0.5 text-amber-800 dark:text-amber-300/90">
          Sans l'accès complet au disque, la corbeille et les pièces jointes de Mail et Messages restent invisibles. Activez Sweepr dans les réglages, puis relancez l'analyse.
        </p>
      </div>
      <button
        class="shrink-0 rounded-lg bg-amber-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-800 dark:bg-amber-400 dark:text-amber-950 dark:hover:bg-amber-300"
        onclick={() => openFullDiskAccessSettings()}
      >
        Ouvrir les réglages
      </button>
    </div>
  {/if}

  <div class="flex flex-col gap-3">
    {#each groups as group (group.rule.id)}
      <RuleCard {group} />
    {:else}
      {#if app.phase === "done"}
        <p class="rounded-xl border border-dashed border-neutral-300 px-4 py-8 text-center text-neutral-500 dark:border-neutral-700">
          Rien à nettoyer ici pour le moment.
        </p>
      {/if}
    {/each}
  </div>

  {#if app.result?.developer}
    <h2 class="mt-8 mb-3 text-base font-semibold">Pour les développeurs</h2>
    <div class="grid grid-cols-1 gap-3 @xl:grid-cols-2">
      {#each [
        { view: "projects" as const, icon: FolderGit2, title: "Projets", text: app.phase === "overview" ? "Recherche en cours" : `${formatBytes(app.total("projects"))} de dossiers de build` },
        { view: "tools" as const, icon: Wrench, title: "Outils développeur", text: `${formatBytes(app.total("developer"))} de caches et SDK` },
      ] as link (link.view)}
        <button class="card group flex items-center gap-3 px-4 py-3 text-left hover:border-neutral-300 dark:hover:border-neutral-700" onclick={() => (app.view = link.view)}>
          <link.icon class="size-5 shrink-0 text-neutral-500" />
          <div class="flex-1">
            <div class="font-medium">{link.title}</div>
            <div class="text-sm text-neutral-500 dark:text-neutral-400">{link.text}</div>
          </div>
          <ArrowRight class="size-4 text-neutral-400 transition-transform group-hover:translate-x-0.5" />
        </button>
      {/each}
    </div>
  {/if}
</div>
