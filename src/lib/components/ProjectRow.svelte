<script lang="ts">
  import { ChevronRight, FolderOpen, GitBranch, RotateCcw, TriangleAlert } from "@lucide/svelte";
  import { app } from "$lib/app.svelte";
  import { reveal } from "$lib/api";
  import { formatAge, formatBytes, formatRegenerate, lastActivity, plural } from "$lib/format";
  import type { Item, Project } from "$lib/types";
  import GroupCheckbox from "./GroupCheckbox.svelte";
  import ItemRow from "./ItemRow.svelte";

  /** `size`: cleanable bytes, computed by the projects view. */
  let { project, items, size }: { project: Project; items: Item[]; size: number } = $props();

  let open = $state(false);

  // Real case: a project with both package-lock.json and yarn.lock.
  const lockWarning = $derived(
    project.package_managers.length > 1
      ? `Plusieurs gestionnaires de paquets (${project.package_managers.join(", ")}) : vérifiez lequel réinstaller.`
      : null,
  );

  const gitLine = $derived.by(() => {
    const git = project.git;
    if (!git) return null;
    const parts = [];
    if (git.changes.length > 0) parts.push(plural(git.changes.length, "modification non commitée", "modifications non commitées"));
    if (git.has_remote && git.unpushed > 0) parts.push(plural(git.unpushed, "commit non poussé", "commits non poussés"));
    if (!git.has_remote) parts.push("aucun dépôt distant");
    return parts.length ? parts.join(" · ") : null;
  });
</script>

<section class="card">
  <div class="flex items-center gap-3 px-4 py-3">
    <GroupCheckbox {items} label={project.name} />
    <button class="flex min-w-0 flex-1 items-center gap-3 text-left" onclick={() => (open = !open)} aria-expanded={open}>
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <h3 class="truncate font-semibold">{project.name}</h3>
          {#each project.ecosystems as eco (eco)}
            <span class="rounded-md bg-neutral-100 px-1.5 py-0.5 text-[11px] font-medium text-neutral-600 dark:bg-neutral-800 dark:text-neutral-400">
              {app.result?.ecosystem_names[eco] ?? eco}
            </span>
          {/each}
          {#if lockWarning}
            <TriangleAlert class="size-3.5 shrink-0 text-amber-600 dark:text-amber-400" aria-label="Avertissement" />
          {/if}
        </div>
        <p class="truncate text-sm text-neutral-500 dark:text-neutral-400">
          Actif {formatAge(lastActivity(project))}
          {#if gitLine}· {gitLine}{/if}
        </p>
      </div>
      <div class="shrink-0 text-right font-semibold tabular-nums">{formatBytes(size)}</div>
      <ChevronRight class="size-4 shrink-0 text-neutral-400 transition-transform {open ? 'rotate-90' : ''}" />
    </button>
  </div>

  {#if open}
    <div class="border-t border-neutral-100 px-4 pt-3 pb-2 dark:border-neutral-800">
      <div class="mb-2 grid gap-1.5 pl-7 text-sm">
        <div class="flex items-center gap-2">
          <span class="selectable min-w-0 flex-1 truncate font-mono text-xs text-neutral-500 dark:text-neutral-400">{project.root}</span>
          <button
            class="flex shrink-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-xs text-neutral-500 hover:bg-neutral-100 hover:text-neutral-800 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
            onclick={() => reveal(project.root)}
          >
            <FolderOpen class="size-3.5" /> Afficher
          </button>
        </div>
        {#if project.regenerate.length > 0}
          <div class="flex gap-2">
            <RotateCcw class="mt-0.5 size-4 shrink-0 text-neutral-400" />
            <span>Pour le relancer : {formatRegenerate(project)}</span>
          </div>
        {/if}
        {#if project.git && gitLine}
          <div class="flex gap-2 text-neutral-600 dark:text-neutral-400">
            <GitBranch class="mt-0.5 size-4 shrink-0 text-neutral-400" />
            <span>{gitLine}. Le nettoyage ne touche pas au code : seuls les dossiers régénérables sont supprimés.</span>
          </div>
        {/if}
        {#if lockWarning}
          <div class="flex gap-2 text-amber-800 dark:text-amber-400">
            <TriangleAlert class="mt-0.5 size-4 shrink-0" />
            <span>{lockWarning}</span>
          </div>
        {/if}
      </div>
      <div class="-mx-1">
        {#each items as item (item.id)}
          <ItemRow {item} showDetail={false} />
        {/each}
      </div>
    </div>
  {/if}
</section>
