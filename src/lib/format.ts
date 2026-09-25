import type { Project, Risk } from "./types";

const decimal = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });
const whole = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 0 });

/** Sizes in decimal units, like the Finder and the macOS storage panel. */
export function formatBytes(bytes: number): string {
  const abs = Math.abs(bytes);
  if (abs >= 1e9) return `${decimal.format(bytes / 1e9)} Go`;
  if (abs >= 1e6) return `${whole.format(bytes / 1e6)} Mo`;
  if (abs >= 1e3) return `${whole.format(bytes / 1e3)} Ko`;
  return `${whole.format(bytes)} octets`;
}

export const sumSize = (items: readonly { size: number }[]) => items.reduce((sum, i) => sum + i.size, 0);

export const bySizeDesc = (a: { size: number }, b: { size: number }) => b.size - a.size;

const relative = new Intl.RelativeTimeFormat("fr-FR", { numeric: "auto" });

/** "il y a 3 jours", "il y a 2 mois"... from a Unix time in seconds. */
export function formatAge(unixSeconds: number | null, now = Date.now()): string {
  if (unixSeconds === null) return "date inconnue";
  const days = Math.round((now / 1000 - unixSeconds) / 86400);
  if (days < 1) return "aujourd'hui";
  if (days < 30) return relative.format(-days, "day");
  if (days < 365) return relative.format(-Math.round(days / 30), "month");
  return relative.format(-Math.round(days / 365), "year");
}

export function plural(count: number, singular: string, pluralForm = `${singular}s`): string {
  return `${count} ${count > 1 ? pluralForm : singular}`;
}

export const RISK_LABELS: Record<Risk, string> = {
  0: "Sans risque",
  1: "Faible",
  2: "À vérifier",
  3: "Irremplaçable",
};

/** Steps to bring a project back after cleaning, as one sentence. */
export function formatRegenerate(project: Project | undefined): string {
  return project?.regenerate.length ? `${project.regenerate.join(", puis ")}.` : "Réinstaller et recompiler le projet.";
}

/** Latest sign of life of a project: a file change or a commit. */
export function lastActivity(project: Project): number | null {
  const commit = project.git?.last_commit ?? null;
  if (project.last_modified === null) return commit;
  if (commit === null) return project.last_modified;
  return Math.max(project.last_modified, commit);
}

const ACTIVE_WINDOW_DAYS = 30;

export function isActive(project: Project, now = Date.now()): boolean {
  const last = lastActivity(project);
  return last !== null && now / 1000 - last <= ACTIVE_WINDOW_DAYS * 86400;
}
