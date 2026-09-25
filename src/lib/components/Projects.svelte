<script lang="ts">
  import { app, cleanable } from "$lib/app.svelte";
  import { bySizeDesc, formatBytes, isActive, lastActivity, plural, sumSize } from "$lib/format";
  import type { Item, Project } from "$lib/types";
  import ProjectRow from "./ProjectRow.svelte";

  interface Entry {
    project: Project;
    items: Item[];
    size: number;
  }

  const entries = $derived.by(() => {
    const byRoot = new Map<string, Item[]>();
    for (const item of app.items) {
      if (!item.project) continue;
      const list = byRoot.get(item.project);
      if (list) list.push(item);
      else byRoot.set(item.project, [item]);
    }
    return (app.result?.projects ?? []).map((project): Entry => {
      const items = (byRoot.get(project.root) ?? []).sort(bySizeDesc);
      return { project, items, size: sumSize(cleanable(items)) };
    });
  });

  const withSomething = $derived(entries.filter((e) => e.size > 0));
  const byActivity = (a: Entry, b: Entry) => (lastActivity(b.project) ?? 0) - (lastActivity(a.project) ?? 0);
  const active = $derived(withSomething.filter((e) => isActive(e.project)).sort(byActivity));
  const inactive = $derived(withSomething.filter((e) => !isActive(e.project)).sort(bySizeDesc));
  const clean = $derived(entries.filter((e) => e.size === 0));

  const inactiveItems = $derived(cleanable(inactive.flatMap((e) => e.items)));
  const allInactiveSelected = $derived(app.allSelected(inactiveItems));
</script>

<div class="view">
  <header class="mb-6 flex flex-col items-start gap-3 @2xl:flex-row @2xl:items-end @2xl:justify-between @2xl:gap-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Projets</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        {#if app.phase === "overview"}
          Recherche des projets dans votre dossier personnel.
        {:else}
          {plural(entries.length, "projet")} trouvé{entries.length > 1 ? "s" : ""}, {formatBytes(sumSize(withSomething))} de dépendances et de builds régénérables. Le code n'est jamais touché.
        {/if}
      </p>
    </div>
    {#if inactiveItems.length > 0}
      <button class="btn btn-outline shrink-0" onclick={() => app.setAll(inactiveItems, !allInactiveSelected)}>
        {allInactiveSelected ? "Désélectionner les inactifs" : `Sélectionner les inactifs (${formatBytes(sumSize(inactive))})`}
      </button>
    {/if}
  </header>

  {#if app.phase === "projects"}
    <p class="mb-4 text-sm text-neutral-500 dark:text-neutral-400">Vérification des dépôts git en cours…</p>
  {/if}

  {#each [
    { key: "active", list: active, title: "Actifs ce dernier mois", text: "Vous y travaillez : les nettoyer vous obligera à tout réinstaller." },
    { key: "inactive", list: inactive, title: "Inactifs", text: "Aucune modification ni commit depuis plus d'un mois. Triés par espace récupérable." },
  ] as section (section.key)}
    {#if section.list.length > 0}
      <h2 class="mb-1 text-base font-semibold">{section.title}</h2>
      <p class="mb-3 text-sm text-neutral-500 dark:text-neutral-400">{section.text}</p>
      <div class="mb-8 flex flex-col gap-3">
        {#each section.list as entry (entry.project.root)}
          <ProjectRow project={entry.project} items={entry.items} size={entry.size} />
        {/each}
      </div>
    {/if}
  {/each}

  {#if clean.length > 0}
    <p class="text-sm text-neutral-500 dark:text-neutral-400">
      {plural(clean.length, "projet")} déjà propre{clean.length > 1 ? "s" : ""} : {clean.map((e) => e.project.name).join(", ")}.
    </p>
  {/if}
</div>
