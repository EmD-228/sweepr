import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ExecFinished, GainProgress, Preview, Report, ScanUpdate } from "./types";

// The interface only sends item ids: paths and commands stay in the engine.

export const startScan = () => invoke<void>("start_scan");
export const cancelScan = () => invoke<void>("cancel_scan");
export const simulate = (ids: string[]) => invoke<Preview[]>("simulate", { ids });
export const execute = (ids: string[], confirmIrreplaceable: boolean) =>
  invoke<void>("execute", { ids, confirmIrreplaceable });
export const reveal = (id: string) => invoke<void>("reveal", { id });
export const openFullDiskAccessSettings = () => invoke<void>("open_full_disk_access_settings");

export interface EngineEvents {
  onScan(update: ScanUpdate): void;
  onExecStart(id: string): void;
  onExecItem(report: Report): void;
  onExecFinished(finished: ExecFinished): void;
  /** Every measure of the real gain; the last one has a final `state`. */
  onGain(progress: GainProgress): void;
  /** "Analyser maintenant" in the menu bar menu. */
  onTrayScan(): void;
}

export async function listenEngine(target: EngineEvents): Promise<UnlistenFn> {
  const unlisten = await Promise.all([
    listen<ScanUpdate>("scan:update", (e) => target.onScan(e.payload)),
    listen<string>("exec:start", (e) => target.onExecStart(e.payload)),
    listen<Report>("exec:item", (e) => target.onExecItem(e.payload)),
    listen<ExecFinished>("exec:finished", (e) => target.onExecFinished(e.payload)),
    listen<GainProgress>("gain:update", (e) => target.onGain(e.payload)),
    listen("tray:scan", () => target.onTrayScan()),
  ]);
  return () => unlisten.forEach((fn) => fn());
}
