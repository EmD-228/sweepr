//! Tauri commands. This layer only translates between the interface and the engine.
//!
//! The interface sends item ids, never paths or commands: every action is looked up in the
//! last scan result, built by the engine from the catalog. Slow work runs on blocking threads
//! and reports through events.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::catalog::{Catalog, Os, Risk};
use crate::exec::{self, Preview, Report, Status};
use crate::measure::{self, StabilityConfig};
use crate::names::Namer;
use crate::paths::PathEnv;
use crate::platform;
use crate::projects;
use crate::scan::{self, Action, Disposal, Item, ScanResult};
use crate::size::Seen;

pub struct AppState {
    catalog: Catalog,
    env: PathEnv,
    /// Application names, read from `/Applications` on the first scan.
    namer: OnceLock<Namer>,
    result: Mutex<Option<ScanResult>>,
    cancel: Arc<AtomicBool>,
    /// A scan or an execution is running.
    busy: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let catalog = Catalog::embedded().map_err(|e| e.to_string())?;
        let env = PathEnv::from_system().ok_or("home folder not found")?;
        Ok(AppState {
            catalog,
            env,
            namer: OnceLock::new(),
            result: Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            busy: Arc::new(AtomicBool::new(false)),
        })
    }

    fn try_start(&self) -> Result<BusyGuard, String> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err("Une analyse ou un nettoyage est déjà en cours.".into());
        }
        self.cancel.store(false, Ordering::SeqCst);
        Ok(BusyGuard(self.busy.clone()))
    }

    fn with_result<T>(&self, f: impl FnOnce(&ScanResult) -> Result<T, String>) -> Result<T, String> {
        let guard = self.result.lock().map_err(|e| e.to_string())?;
        f(guard.as_ref().ok_or("Aucune analyse. Lancez d'abord une analyse.")?)
    }
}

/// Clears the busy flag when the work ends, even on panic.
struct BusyGuard(Arc<AtomicBool>);

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    /// Caches, trash, installers and developer tools are listed.
    Overview,
    /// Projects are listed with their artifacts.
    Projects,
    /// Git checks are done: the result is final.
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
struct ScanUpdate<'a> {
    phase: ScanPhase,
    result: &'a ScanResult,
}

fn publish(app: &AppHandle, state: &AppState, phase: ScanPhase, result: &ScanResult) {
    let _ = app.emit("scan:update", ScanUpdate { phase, result });
    if let Ok(mut slot) = state.result.lock() {
        *slot = Some(result.clone());
    }
}

/// Scans the disk in three steps, each published as a `scan:update` event so the
/// interface shows the big picture first and fills in the details.
#[tauri::command]
pub async fn start_scan(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let busy = state.try_start()?;
    blocking(move || {
        let _busy = busy;
        run_scan(&app);
    })
    .await
}

fn run_scan(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (env, catalog, cancel) = (&state.env, &state.catalog, &state.cancel);
    let os = Os::current();
    let developer = scan::developer_detected(env);
    let seen = Seen::default();

    let disk = platform::disk_space(env.home()).ok();
    let rules = scan::active_rules(catalog, os, developer);
    let namer = state.namer.get_or_init(|| Namer::load(env.home()));
    let mut result = ScanResult {
        developer,
        free_space: disk.map(|d| d.free),
        total_space: disk.map(|d| d.total),
        rules: rules.iter().map(|r| (*r).clone()).collect(),
        ecosystem_names: catalog
            .ecosystems
            .iter()
            .map(|e| (e.id.clone(), e.name.clone()))
            .collect(),
        items: scan::rule_items(env, &rules, namer, &seen, cancel),
        projects: Vec::new(),
        full_disk_access: platform::full_disk_access(env.home()),
    };
    if cancel.load(Ordering::Relaxed) {
        return publish(app, &state, ScanPhase::Cancelled, &result);
    }
    publish(app, &state, ScanPhase::Overview, &result);
    if !developer {
        return publish(app, &state, ScanPhase::Done, &result);
    }

    let found = scan::scan_projects(env, &catalog.ecosystems, &seen, cancel);
    result.items.extend(scan::project_items(&found, &catalog.ecosystems));
    result.projects = found;
    if cancel.load(Ordering::Relaxed) {
        return publish(app, &state, ScanPhase::Cancelled, &result);
    }
    publish(app, &state, ScanPhase::Projects, &result);

    projects::attach_git(&mut result.projects);
    scan::apply_project_guards(&mut result.items);
    publish(app, &state, ScanPhase::Done, &result);
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

/// Lists exactly what cleaning the selected items would touch, without touching anything.
#[tauri::command]
pub async fn simulate(ids: Vec<String>, app: AppHandle, state: State<'_, AppState>) -> Result<Vec<Preview>, String> {
    let items = state.with_result(|result| result.items_with_ids(&ids))?;
    blocking(move || {
        let state = app.state::<AppState>();
        items.iter().map(|item| exec::preview(item, state.env.home())).collect()
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
struct ExecFinished {
    reports: Vec<Report>,
    /// Estimated bytes freed by the items that were cleaned.
    estimate: u64,
    /// Estimated bytes moved to the trash: freed only once the trash is emptied.
    trashed: u64,
}

/// Cleans the selected items one by one (`exec:item` events), then follows free space until it
/// settles (`gain:update` events). Irreplaceable items need `confirm_irreplaceable`, set by the
/// second confirmation of the interface.
#[tauri::command]
pub async fn execute(
    ids: Vec<String>,
    confirm_irreplaceable: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let items = state.with_result(|result| result.items_with_ids(&ids))?;
    if !confirm_irreplaceable && items.iter().any(|i| i.risk == Risk::Irreplaceable) {
        return Err("Certains éléments sont irremplaçables et demandent une double confirmation.".into());
    }
    let busy = state.try_start()?;
    blocking(move || run_execution(&app, &items, busy)).await
}

fn run_execution(app: &AppHandle, items: &[Item], busy: BusyGuard) {
    let state = app.state::<AppState>();
    let home = state.env.home().to_path_buf();
    let free_before = platform::free_space(&home).ok();

    let mut reports = Vec::with_capacity(items.len());
    for item in items {
        let _ = app.emit("exec:start", &item.id);
        let report = exec::execute(item, &home);
        let _ = app.emit("exec:item", &report);
        reports.push(report);
    }

    // `reports[i]` is the report of `items[i]`.
    let (mut estimate, mut trashed) = (0, 0);
    for (item, report) in items.iter().zip(&reports) {
        if matches!(report.status, Status::Done | Status::Partial) {
            estimate += item.size;
            if item.action.disposal() == Some(Disposal::Trash) {
                trashed += item.size;
            }
        }
    }

    // Cleaned items leave the result, so they cannot be run twice. The interface gets the
    // updated result like after a scan.
    let done: HashSet<&str> = reports
        .iter()
        .filter(|r| r.status == Status::Done)
        .map(|r| r.id.as_str())
        .collect();
    let updated = state.result.lock().ok().and_then(|mut guard| {
        let result = guard.as_mut()?;
        result.items.retain(|i| !done.contains(i.id.as_str()));
        Some(result.clone())
    });
    if let Some(result) = updated {
        let _ = app.emit(
            "scan:update",
            ScanUpdate {
                phase: ScanPhase::Done,
                result: &result,
            },
        );
    }
    let _ = app.emit(
        "exec:finished",
        ExecFinished {
            reports,
            estimate,
            trashed,
        },
    );
    // Measuring can take minutes: a new scan may start meanwhile.
    drop(busy);

    if let Some(baseline) = free_before {
        // Each step is reported, the last one with a final state (`stable` or `timed_out`).
        measure::follow(
            baseline,
            estimate.saturating_sub(trashed),
            StabilityConfig::default(),
            || platform::free_space(&home),
            std::thread::sleep,
            |progress| {
                let _ = app.emit("gain:update", progress);
            },
        );
    }
}

/// Shows an item, or a project given by its root, in the Finder or the file explorer.
#[tauri::command]
pub fn reveal(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let path: PathBuf = state.with_result(|result| match result.item(&id).map(|i| &i.action) {
        Some(Action::Delete { paths, .. }) if !paths.is_empty() => Ok(paths[0].clone()),
        _ => result
            .projects
            .iter()
            .find(|p| p.root.to_string_lossy() == id)
            .map(|p| p.root.clone())
            .ok_or_else(|| "Rien à afficher pour cet élément.".to_string()),
    })?;
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_full_disk_access_settings() -> Result<(), String> {
    tauri_plugin_opener::open_url(platform::FULL_DISK_ACCESS_SETTINGS, None::<&str>).map_err(|e| e.to_string())
}
