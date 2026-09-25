// Mirrors of the Rust types sent by the engine (src-tauri/src/scan.rs, exec.rs, measure.rs).

export type Risk = 0 | 1 | 2 | 3;
export type Disposal = "delete" | "trash";
export type Profile = "general" | "developer";

export interface Item {
  id: string;
  /** Rule id, or `project` for project artifacts. */
  rule: string;
  title: string;
  detail: string | null;
  /** Project root, for project artifacts. */
  project: string | null;
  risk: Risk;
  size: number;
  size_known: boolean;
  blocked: string | null;
  disposal: Disposal | null;
}

export type Category = "caches" | "trash" | "temporary" | "personal" | "developer";

export interface RuleView {
  id: string;
  profile: Profile;
  category: Category;
  title: string;
  summary: string;
  risk: Risk;
  loses: string;
  regenerate: string;
  note: string | null;
}

export interface RepoStatus {
  root: string;
  changes: string[];
  unpushed: number;
  has_remote: boolean;
  last_commit: number | null;
}

export interface Project {
  root: string;
  name: string;
  ecosystems: string[];
  package_managers: string[];
  node_version: string | null;
  last_modified: number | null;
  git: RepoStatus | null;
  regenerate: string[];
}

export interface ScanResult {
  developer: boolean;
  free_space: number | null;
  total_space: number | null;
  rules: RuleView[];
  /** Ecosystem id → name, from the catalog. */
  ecosystem_names: Record<string, string>;
  items: Item[];
  projects: Project[];
  full_disk_access: boolean | null;
}

export type ScanPhase = "overview" | "projects" | "done" | "cancelled";

export interface ScanUpdate {
  phase: ScanPhase;
  result: ScanResult;
}

export interface Preview {
  id: string;
  actions: string[];
  disposal: Disposal | null;
  skipped: string[];
}

export type ReportStatus = "done" | "partial" | "skipped" | "failed";

export interface Report {
  id: string;
  status: ReportStatus;
  cleaned: number;
  messages: string[];
}

export interface ExecFinished {
  reports: Report[];
  estimate: number;
  trashed: number;
}

export interface GainProgress {
  gain: number;
  estimate: number;
  elapsed_secs: number;
  state: "recovering" | "stable" | "timed_out";
}
