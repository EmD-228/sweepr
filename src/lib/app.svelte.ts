import { SvelteSet } from "svelte/reactivity";
import * as api from "./api";
import { bySizeDesc, sumSize } from "./format";
import type {
  Category,
  ExecFinished,
  GainProgress,
  Item,
  Preview,
  Profile,
  Project,
  Report,
  RuleView,
  ScanPhase,
  ScanResult,
  ScanUpdate,
} from "./types";

export type View = "overview" | "projects" | "tools";
export type ItemRun = "pending" | "running" | Report;
/** Where an item is shown: a rule category, or the projects view. */
export type Bucket = Category | "projects";

export interface Execution {
  items: Item[];
  runs: Record<string, ItemRun>;
  finished: ExecFinished | null;
  gain: GainProgress | null;
}

export interface RuleGroup {
  rule: RuleView;
  /** Largest first. */
  items: Item[];
  /** Bytes that can be cleaned: blocked items excluded. */
  size: number;
}

/** What can be cleaned: blocked items never count. */
export const cleanable = (items: readonly Item[]) => items.filter((i) => !i.blocked);

class AppState {
  // Replaced as a whole by each engine update, never mutated: no deep proxy needed.
  result = $state.raw<ScanResult | null>(null);
  phase = $state<ScanPhase | null>(null);
  scanning = $state(false);
  view = $state<View>("overview");
  selected = new SvelteSet<string>();
  error = $state<string | null>(null);

  /** Open dialogs. */
  confirming = $state(false);
  previews = $state<Preview[] | null>(null);
  execution = $state<Execution | null>(null);

  items = $derived(this.result?.items ?? []);
  rules = $derived(new Map((this.result?.rules ?? []).map((r) => [r.id, r] as const)));
  itemsById = $derived(new Map(this.items.map((i) => [i.id, i] as const)));
  projectsByRoot = $derived(new Map((this.result?.projects ?? []).map((p) => [p.root, p] as const)));
  selectedItems = $derived(this.items.filter((i) => this.selected.has(i.id)));
  selectedSize = $derived(sumSize(this.selectedItems));

  /** Items grouped by rule in one pass, each group sorted once. */
  groups = $derived.by(() => {
    const byRule = new Map<string, Item[]>();
    for (const item of this.items) {
      if (item.project) continue;
      const list = byRule.get(item.rule);
      if (list) list.push(item);
      else byRule.set(item.rule, [item]);
    }
    const groups: RuleGroup[] = [];
    for (const [id, items] of byRule) {
      const rule = this.rules.get(id);
      if (rule) groups.push({ rule, items: items.sort(bySizeDesc), size: sumSize(cleanable(items)) });
    }
    return groups.sort(bySizeDesc);
  });

  /** Cleanable bytes per bucket, for the sidebar, the overview and the charts. */
  totals = $derived.by(() => {
    const totals = new Map<Bucket, number>();
    for (const item of cleanable(this.items)) {
      const bucket = this.bucketOf(item);
      if (bucket) totals.set(bucket, (totals.get(bucket) ?? 0) + item.size);
    }
    return totals;
  });

  rule(id: string): RuleView | undefined {
    return this.rules.get(id);
  }

  project(root: string | null): Project | undefined {
    return root ? this.projectsByRoot.get(root) : undefined;
  }

  bucketOf(item: Item): Bucket | undefined {
    return item.project ? "projects" : this.rule(item.rule)?.category;
  }

  total(...buckets: Bucket[]): number {
    return buckets.reduce((sum, b) => sum + (this.totals.get(b) ?? 0), 0);
  }

  groupsFor(profile: Profile): RuleGroup[] {
    return this.groups.filter((g) => g.rule.profile === profile);
  }

  /** Blocked items can never be selected. */
  toggle(item: Item, on = !this.selected.has(item.id)) {
    if (item.blocked) return;
    if (on) this.selected.add(item.id);
    else this.selected.delete(item.id);
  }

  setAll(items: readonly Item[], on: boolean) {
    for (const item of items) this.toggle(item, on);
  }

  allSelected(items: readonly Item[]): boolean {
    const selectable = cleanable(items);
    return selectable.length > 0 && selectable.every((i) => this.selected.has(i.id));
  }

  async scan() {
    if (this.scanning) return;
    this.error = null;
    this.scanning = true;
    this.phase = null;
    try {
      await api.startScan();
    } catch (e) {
      this.error = String(e);
    } finally {
      this.scanning = false;
    }
  }

  cancelScan() {
    api.cancelScan();
  }

  async simulate() {
    this.error = null;
    try {
      this.previews = await api.simulate(this.selectedItems.map((i) => i.id));
    } catch (e) {
      this.error = String(e);
    }
  }

  async execute(confirmIrreplaceable: boolean) {
    const items = this.selectedItems;
    this.confirming = false;
    this.execution = {
      items,
      runs: Object.fromEntries(items.map((i) => [i.id, "pending" as ItemRun])),
      finished: null,
      gain: null,
    };
    try {
      await api.execute(
        items.map((i) => i.id),
        confirmIrreplaceable,
      );
    } catch (e) {
      this.error = String(e);
      this.execution = null;
    }
  }

  closeExecution() {
    if (this.execution?.finished) this.execution = null;
  }

  // Engine events (see `api.listenEngine`).

  onScan({ phase, result }: ScanUpdate) {
    this.phase = phase;
    this.result = result;
    // Items gone from the new result (cleaned, or changed on disk) cannot stay selected.
    const ids = new Set(cleanable(result.items).map((i) => i.id));
    for (const id of [...this.selected]) if (!ids.has(id)) this.selected.delete(id);
  }

  onExecStart(id: string) {
    if (this.execution) this.execution.runs[id] = "running";
  }

  onExecItem(report: Report) {
    if (this.execution) this.execution.runs[report.id] = report;
  }

  onExecFinished(finished: ExecFinished) {
    if (this.execution) this.execution.finished = finished;
  }

  onGain(progress: GainProgress) {
    if (this.execution) this.execution.gain = progress;
  }

  onTrayScan() {
    this.scan();
  }
}

export const app = new AppState();
