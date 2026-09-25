<script lang="ts">
  import { Circle, CircleAlert, CircleCheck, CircleMinus, CircleX, LoaderCircle } from "@lucide/svelte";
  import { app, type Execution } from "$lib/app.svelte";
  import { formatBytes } from "$lib/format";
  import Modal from "./Modal.svelte";

  let { execution }: { execution: Execution } = $props();

  const done = $derived(execution.finished !== null);
  const gain = $derived(execution.gain);
  const estimate = $derived(execution.finished ? execution.finished.estimate - execution.finished.trashed : 0);

  const statusLabel = {
    done: "Fait",
    partial: "En partie",
    skipped: "Ignoré",
    failed: "Erreur",
  } as const;
</script>

<Modal
  title={done ? "Nettoyage terminé" : "Nettoyage en cours"}
  subtitle={done ? undefined : "Chaque élément est vérifié une dernière fois avant d'être supprimé."}
  closable={done}
  onclose={() => app.closeExecution()}
>
  {#if done}
    <div class="mb-5 rounded-xl bg-neutral-100 px-5 py-4 dark:bg-neutral-800">
      {#if gain}
        <div class="flex items-baseline gap-3">
          <span class="text-3xl font-semibold tabular-nums">{formatBytes(Math.max(gain.gain, 0))}</span>
          <span class="text-sm text-neutral-500 dark:text-neutral-400">récupérés, mesurés sur le disque</span>
        </div>
        <div class="mt-1 text-sm text-neutral-600 dark:text-neutral-400">
          Estimation : {formatBytes(estimate)}.
          {#if gain.state === "recovering"}
            <span class="inline-flex items-center gap-1.5">
              <LoaderCircle class="size-3.5 animate-spin" /> Récupération en cours : macOS libère parfois l'espace avec quelques minutes de retard.
            </span>
          {:else if gain.state === "timed_out"}
            L'espace libre bougeait encore : le chiffre peut encore évoluer.
          {:else if gain.gain < estimate * 0.5 && estimate > 0}
            L'écart vient des fichiers partagés entre plusieurs dossiers, comptés une seule fois sur le disque.
          {/if}
        </div>
      {:else}
        <div class="flex items-center gap-2 text-sm">
          <LoaderCircle class="size-4 animate-spin" /> Mesure de l'espace libéré…
        </div>
      {/if}
      {#if execution.finished && execution.finished.trashed > 0}
        <p class="mt-2 text-sm text-neutral-600 dark:text-neutral-400">
          {formatBytes(execution.finished.trashed)} ont été déplacés dans la corbeille. Videz-la pour libérer cet espace.
        </p>
      {/if}
    </div>
  {/if}

  <ul class="flex flex-col gap-1.5">
    {#each execution.items as item (item.id)}
      {@const run = execution.runs[item.id]}
      <li class="flex items-start gap-2.5 text-sm">
        {#if run === "pending"}
          <Circle class="mt-0.5 size-4 shrink-0 text-neutral-300 dark:text-neutral-600" aria-label="En attente" />
        {:else if run === "running"}
          <LoaderCircle class="mt-0.5 size-4 shrink-0 animate-spin text-neutral-500" aria-label="En cours" />
        {:else if run.status === "done"}
          <CircleCheck class="mt-0.5 size-4 shrink-0 text-emerald-600" aria-label="Fait" />
        {:else if run.status === "partial"}
          <CircleAlert class="mt-0.5 size-4 shrink-0 text-amber-600" aria-label="En partie" />
        {:else if run.status === "skipped"}
          <CircleMinus class="mt-0.5 size-4 shrink-0 text-neutral-400" aria-label="Ignoré" />
        {:else}
          <CircleX class="mt-0.5 size-4 shrink-0 text-red-600" aria-label="Erreur" />
        {/if}
        <div class="min-w-0 flex-1">
          <div class="flex gap-2">
            <span class="flex-1 truncate">{item.title}</span>
            {#if typeof run === "object"}
              <span class="text-neutral-500 dark:text-neutral-400">{statusLabel[run.status]}</span>
            {/if}
          </div>
          {#if typeof run === "object"}
            {#each run.messages as message}
              <p class="selectable text-xs break-all text-neutral-500 dark:text-neutral-400">{message}</p>
            {/each}
          {/if}
        </div>
      </li>
    {/each}
  </ul>

  {#snippet footer()}
    {#if done}
      <button class="btn btn-ghost" onclick={() => app.closeExecution()}>Fermer</button>
      <button
        class="btn btn-primary"
        onclick={() => {
          app.closeExecution();
          app.scan();
        }}
      >
        Analyser à nouveau
      </button>
    {/if}
  {/snippet}
</Modal>
