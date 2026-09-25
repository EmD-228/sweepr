<script lang="ts">
  import { RISK_LABELS } from "$lib/format";
  import type { Risk } from "$lib/types";

  let { risk }: { risk: Risk } = $props();

  const styles: Record<Risk, string> = {
    0: "bg-emerald-50 text-emerald-700 ring-emerald-600/20 dark:bg-emerald-500/10 dark:text-emerald-400 dark:ring-emerald-400/20",
    1: "bg-sky-50 text-sky-700 ring-sky-600/20 dark:bg-sky-500/10 dark:text-sky-400 dark:ring-sky-400/20",
    2: "bg-amber-50 text-amber-800 ring-amber-600/25 dark:bg-amber-500/10 dark:text-amber-400 dark:ring-amber-400/20",
    3: "bg-red-50 text-red-700 ring-red-600/20 dark:bg-red-500/10 dark:text-red-400 dark:ring-red-400/20",
  };
</script>

<span class="inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-[11px] font-medium ring-1 ring-inset {styles[risk]}">
  {RISK_LABELS[risk]}
</span>
