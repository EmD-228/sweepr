//! Turns the catalog and the disk into a list of cleanable items.
//!
//! Every item carries the exact action to run, built here from the catalog and validated
//! paths. The interface only ever sends item ids back: it can never name a path or a command.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rayon::prelude::*;
use serde::Serialize;

use crate::catalog::{Catalog, Ecosystem, GroupBy, Os, Profile, Risk, Rule, Target};
use crate::guards;
use crate::names::Namer;
use crate::paths::PathEnv;
use crate::process;
use crate::projects::{self, Project};
use crate::size::{self, Seen};

/// How deleted paths leave the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposal {
    /// Removed for good. Only for data that comes back by itself or with a command.
    Delete,
    /// Moved to the system trash, for data that may not come back (risk 2 and 3).
    Trash,
}

#[derive(Debug, Clone)]
pub enum Action {
    Delete {
        paths: Vec<PathBuf>,
        disposal: Disposal,
        /// Folders outside home that the rule may clean in (for example `/Applications`).
        extra_roots: Vec<PathBuf>,
        /// Run the project guards (tracked files, `.env`, nested repository) before deleting.
        project_guards: bool,
    },
    Command {
        program: String,
        args: Vec<String>,
    },
}

impl Action {
    /// Deletion of `paths`. Data that may not come back (risk 2 and 3) goes to the trash:
    /// this is the only place that decides it.
    pub fn delete(paths: Vec<PathBuf>, risk: Risk, extra_roots: Vec<PathBuf>, project_guards: bool) -> Action {
        let disposal = if risk >= Risk::Review {
            Disposal::Trash
        } else {
            Disposal::Delete
        };
        Action::Delete {
            paths,
            disposal,
            extra_roots,
            project_guards,
        }
    }

    pub fn disposal(&self) -> Option<Disposal> {
        match self {
            Action::Delete { disposal, .. } => Some(*disposal),
            Action::Command { .. } => None,
        }
    }

    /// What the action will do, in a form the user can read.
    pub fn describe(&self) -> Vec<String> {
        match self {
            Action::Delete { paths, .. } => paths.iter().map(|p| p.display().to_string()).collect(),
            Action::Command { program, args } => vec![std::iter::once(program.as_str())
                .chain(args.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" ")],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    /// Rule id, or `project` for project artifacts.
    pub rule: String,
    pub title: String,
    /// Secondary line: a path, a command or a short description.
    pub detail: Option<String>,
    /// Project root, for project artifacts.
    pub project: Option<PathBuf>,
    pub risk: Risk,
    /// Estimated bytes freed.
    pub size: u64,
    /// False when the gain cannot be known before running (for example `pnpm store prune`).
    pub size_known: bool,
    /// Why the item cannot be cleaned, if so. Shown, never selectable.
    pub blocked: Option<String>,
    /// The interface only sees how paths leave the disk (`disposal`), never the paths or commands.
    #[serde(rename = "disposal", serialize_with = "serialize_disposal")]
    pub action: Action,
}

fn serialize_disposal<S: serde::Serializer>(action: &Action, serializer: S) -> Result<S::Ok, S::Error> {
    action.disposal().serialize(serializer)
}

/// Whether developer tools are present, which turns on the developer module.
pub fn developer_detected(env: &PathEnv) -> bool {
    let home = env.home();
    let folders = [
        "Library/Developer",
        ".android",
        ".cargo",
        ".pub-cache",
        ".npm",
        ".gradle",
        ".docker",
    ];
    folders.iter().any(|f| home.join(f).exists())
        || ["node", "docker", "flutter", "cargo", "xcrun"]
            .iter()
            .any(|p| process::has_program(p))
}

/// Rules shown on this system: right platform, right profile, required programs installed.
pub fn active_rules(catalog: &Catalog, os: Os, developer: bool) -> Vec<&Rule> {
    catalog
        .rules_for(os)
        .filter(|r| developer || r.profile == Profile::General)
        .filter(|r| r.requires.iter().all(|p| process::has_program(p)))
        .collect()
}

/// Paths a rule targets or measures.
fn rule_paths(env: &PathEnv, rule: &Rule) -> Vec<PathBuf> {
    match &rule.target {
        Target::Paths { paths, .. } => env.resolve_all(paths),
        Target::Command { measure, .. } => env.resolve_all(measure),
        Target::Provider { .. } => Vec::new(),
    }
}

/// Builds the items of every `paths` and `command` rule in `rules`.
/// Provider rules are handled by `providers`.
pub fn rule_items(env: &PathEnv, rules: &[&Rule], namer: &Namer, seen: &Seen, cancel: &AtomicBool) -> Vec<Item> {
    // Developer rules claim their folders first, so `~/Library/Caches/Yarn` shows as
    // "Cache Yarn" and not a second time among application caches.
    let resolved: Vec<Vec<PathBuf>> = rules.par_iter().map(|rule| rule_paths(env, rule)).collect();
    let claimed: Vec<&PathBuf> = rules
        .iter()
        .zip(&resolved)
        .filter(|(rule, _)| rule.profile == Profile::Developer)
        .flat_map(|(_, paths)| paths)
        .collect();

    rules
        .par_iter()
        .zip(resolved.par_iter())
        .flat_map_iter(|(rule, matched)| {
            let claimed = if rule.profile == Profile::General {
                claimed.as_slice()
            } else {
                &[]
            };
            items_for_rule(env, rule, matched.clone(), claimed, namer, seen, cancel)
        })
        .collect()
}

fn items_for_rule(
    env: &PathEnv,
    rule: &Rule,
    mut matched: Vec<PathBuf>,
    claimed: &[&PathBuf],
    namer: &Namer,
    seen: &Seen,
    cancel: &AtomicBool,
) -> Vec<Item> {
    match &rule.target {
        Target::Paths { paths, exclude } => {
            let mut extra_roots = Vec::new();
            for pattern in paths {
                if let Ok(expanded) = env.expand(pattern) {
                    if !expanded.base.starts_with(env.home()) && !extra_roots.contains(&expanded.base) {
                        extra_roots.push(expanded.base);
                    }
                }
            }
            // A pattern that cannot be compiled excludes nothing it could have matched: drop the path.
            let exclusions: Vec<Option<glob::Pattern>> = exclude.iter().map(|p| env.compile(p).ok()).collect();
            matched.retain(|path| {
                let excluded = exclusions
                    .iter()
                    .any(|p| p.as_ref().is_none_or(|p| p.matches_path(path)));
                let is_claimed = claimed.iter().any(|c| path.starts_with(c));
                !excluded && !is_claimed
            });
            let make = |id: String, title: String, detail: Option<String>, paths: Vec<PathBuf>| Item {
                id,
                rule: rule.id.clone(),
                title,
                detail,
                project: None,
                risk: rule.risk,
                size: size::total_allocated(&paths, seen, cancel),
                size_known: true,
                blocked: None,
                action: Action::delete(paths, rule.risk, extra_roots.clone(), false),
            };
            match rule.group_by {
                GroupBy::App | GroupBy::XcodeProject => {
                    // Folders of the same application (`BraveSoftware`, `com.brave.Browser`) form one item.
                    let mut groups: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
                    for path in matched {
                        groups
                            .entry(namer.child_title(rule.group_by, &path))
                            .or_default()
                            .push(path);
                    }
                    groups
                        .into_iter()
                        .map(|(title, paths)| {
                            let detail = match paths.as_slice() {
                                [one] => one.display().to_string(),
                                many => format!("{} dossiers", many.len()),
                            };
                            make(format!("{}:{title}", rule.id), title, Some(detail), paths)
                        })
                        .filter(|item| item.size > 0)
                        .collect()
                }
                GroupBy::None if matched.is_empty() => Vec::new(),
                GroupBy::None => {
                    let detail = match matched.len() {
                        1 => matched[0].display().to_string(),
                        n => format!("{n} éléments"),
                    };
                    let item = make(rule.id.clone(), rule.title.clone(), Some(detail), matched);
                    if item.size > 0 {
                        vec![item]
                    } else {
                        Vec::new()
                    }
                }
            }
        }
        Target::Command { program, args, measure } => {
            let action = Action::Command {
                program: program.clone(),
                args: args.clone(),
            };
            vec![Item {
                id: rule.id.clone(),
                rule: rule.id.clone(),
                title: rule.title.clone(),
                detail: action.describe().into_iter().next(),
                project: None,
                risk: rule.risk,
                // `matched` holds the resolved `measure` folders.
                size: size::total_allocated(&matched, seen, cancel),
                size_known: !measure.is_empty(),
                blocked: None,
                action,
            }]
        }
        Target::Provider { .. } => Vec::new(),
    }
}

/// Folders searched for projects: the home folder, without system and media folders.
pub fn project_roots(env: &PathEnv) -> Vec<PathBuf> {
    const SKIP: &[&str] = &[
        "Library",
        "Applications",
        "Pictures",
        "Movies",
        "Music",
        "Public",
        "AppData",
    ];
    let mut roots: Vec<PathBuf> = projects::subdirs(env.home())
        .into_iter()
        .filter(|(name, _)| !projects::is_hidden(name) && !SKIP.contains(&name.as_str()))
        .map(|(_, path)| path)
        .collect();
    roots.sort();
    roots
}

/// Projects below the home folder, largest reclaimable space first.
pub fn scan_projects(env: &PathEnv, ecosystems: &[Ecosystem], seen: &Seen, cancel: &AtomicBool) -> Vec<Project> {
    // A project folder directly in a root (for example `~/Developer/app`) must be found too,
    // so the roots themselves are searched from their parent's point of view.
    let roots = project_roots(env);
    let mut found = Vec::new();
    for root in &roots {
        if projects::is_tool_install(root) {
            continue;
        }
        if ecosystems.iter().any(|eco| projects::detects(eco, root)) {
            found.push(root.clone());
        } else {
            found.extend(projects::find_roots(std::slice::from_ref(root), ecosystems, cancel));
        }
    }
    let mut projects: Vec<Project> = found
        .par_iter()
        .map(|root| projects::analyze(root, ecosystems, seen, cancel))
        .collect();
    projects.sort_by_key(|p| std::cmp::Reverse(p.reclaimable()));
    projects
}

/// One item per project artifact.
pub fn project_items(projects: &[Project], ecosystems: &[Ecosystem]) -> Vec<Item> {
    let risks: HashMap<&str, Risk> = ecosystems.iter().map(|e| (e.id.as_str(), e.risk)).collect();
    let risks = &risks;
    projects
        .iter()
        .flat_map(|project| {
            project.artifacts.iter().map(move |artifact| {
                let relative = artifact.path.strip_prefix(&project.root).unwrap_or(&artifact.path);
                let risk = risks.get(artifact.ecosystem.as_str()).copied().unwrap_or(Risk::Low);
                Item {
                    id: format!("artifact:{}", artifact.path.display()),
                    rule: "project".into(),
                    title: relative.display().to_string(),
                    detail: Some(project.name.clone()),
                    project: Some(project.root.clone()),
                    risk,
                    size: artifact.usage.allocated,
                    size_known: true,
                    blocked: None,
                    action: Action::delete(vec![artifact.path.clone()], risk, Vec::new(), true),
                }
            })
        })
        .collect()
}

/// Marks artifacts that the project guards refuse (tracked files, `.env`, nested repository).
/// The same checks run again right before deletion.
pub fn apply_project_guards(items: &mut [Item]) {
    let git = guards::git_available();
    items.par_iter_mut().for_each(|item| {
        if let Action::Delete {
            paths,
            project_guards: true,
            ..
        } = &item.action
        {
            for path in paths {
                let result = if git {
                    guards::check_artifact(path)
                } else {
                    Err(guards::Blocked::GitError {
                        message: "git n'est pas disponible".into(),
                    })
                };
                if let Err(blocked) = result {
                    item.blocked = Some(describe_blocked(&blocked));
                    break;
                }
            }
        }
    });
}

pub fn describe_blocked(blocked: &guards::Blocked) -> String {
    match blocked {
        guards::Blocked::TrackedFiles { count } => {
            format!(
                "Contient {count} fichier{} suivi{} par git : ce dossier fait partie du code.",
                plural(*count),
                plural(*count)
            )
        }
        guards::Blocked::ContainsRepository => "Contient son propre dépôt git.".into(),
        guards::Blocked::EnvFile { path } => format!(
            "Contient un fichier de configuration ({}).",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
        guards::Blocked::GitError { message } => format!("Impossible de vérifier avec git : {message}"),
    }
}

fn plural(count: usize) -> &'static str {
    if count > 1 {
        "s"
    } else {
        ""
    }
}

/// Everything found, grouped for the interface.
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub developer: bool,
    pub free_space: Option<u64>,
    /// Capacity of the volume holding the home folder.
    pub total_space: Option<u64>,
    pub rules: Vec<Rule>,
    /// Ecosystem id → name shown to the user, from the catalog.
    pub ecosystem_names: BTreeMap<String, String>,
    pub items: Vec<Item>,
    pub projects: Vec<Project>,
    /// macOS: false when Sweepr cannot read protected folders such as the Trash.
    pub full_disk_access: Option<bool>,
}

impl ScanResult {
    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    /// The items with these ids, in scan order, in one pass over the items.
    pub fn items_with_ids(&self, ids: &[String]) -> Result<Vec<Item>, String> {
        let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let found: Vec<Item> = self
            .items
            .iter()
            .filter(|i| wanted.contains(i.id.as_str()))
            .cloned()
            .collect();
        if found.len() != wanted.len() {
            return Err("Certains éléments ont changé depuis l'analyse. Relancez l'analyse.".into());
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn builds_items_from_rules() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        write(&home.join("Library/Caches/com.spotify.client/data"), 40_000);
        write(&home.join("Library/Caches/Yarn/v6/pkg"), 20_000);
        write(&home.join("Library/Logs/app.log"), 10_000);
        write(&home.join("Downloads/setup.dmg"), 30_000);
        write(&home.join("Downloads/photo.jpg"), 30_000);
        let env = PathEnv::new(&home);
        let catalog = Catalog::embedded().unwrap();
        let cancel = AtomicBool::new(false);

        let rules = active_rules(&catalog, Os::Macos, false);
        let items = rule_items(&env, &rules, &Namer::default(), &Seen::default(), &cancel);
        let caches: Vec<_> = items.iter().filter(|i| i.rule == "macos.app-caches").collect();
        assert_eq!(
            caches.len(),
            2,
            "without the developer module, Yarn is an ordinary cache"
        );
        let spotify = home.join("Library/Caches/com.spotify.client").display().to_string();
        assert!(caches.iter().any(|i| i.detail.as_deref() == Some(spotify.as_str())));

        let installers = items.iter().find(|i| i.rule == "macos.old-installers").unwrap();
        assert_eq!(
            installers.action.describe(),
            vec![home.join("Downloads/setup.dmg").display().to_string()]
        );
        assert!(items.iter().any(|i| i.rule == "macos.logs" && i.size >= 10_000));
        assert!(items
            .iter()
            .all(|i| catalog.rule(&i.rule).unwrap().profile == Profile::General));
    }

    #[test]
    fn project_artifacts_become_guarded_items() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        write(&home.join("Documents/site/package.json"), 2);
        write(&home.join("Documents/site/node_modules/react/index.js"), 5_000);
        let env = PathEnv::new(&home);
        let catalog = Catalog::embedded().unwrap();
        let cancel = AtomicBool::new(false);

        let projects = scan_projects(&env, &catalog.ecosystems, &Seen::default(), &cancel);
        assert_eq!(projects.len(), 1);
        let mut items = project_items(&projects, &catalog.ecosystems);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "node_modules");
        assert_eq!(items[0].risk, Risk::Low);
        apply_project_guards(&mut items);
        assert_eq!(items[0].blocked, None);
    }
}
