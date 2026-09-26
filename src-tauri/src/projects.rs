//! Finding development projects and what can be cleaned in them (SPEC sections 5.4 and 9).
//!
//! A project is the outermost folder holding a manifest (`package.json`, `pubspec.yaml`,
//! `Cargo.toml`...). Manifests deeper inside it (monorepo packages, `src-tauri`) are members of
//! that project, and their artifacts belong to it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use serde::Serialize;

use crate::catalog::Ecosystem;
use crate::guards::{self, RepoStatus};
use crate::size::{self, Seen, Usage};

/// Folders never entered while looking for projects or for recent activity.
const NEVER_ENTER: &[&str] = &["node_modules", ".git", "Pods", ".Trash"];
/// Maximum depth searched below each scan root, and below a project root for members.
const MAX_DEPTH: usize = 8;
const MEMBER_DEPTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    pub fn name(self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
        }
    }
}

/// Lock files and the package manager each one belongs to.
const LOCKFILES: &[(&str, PackageManager)] = &[
    ("package-lock.json", PackageManager::Npm),
    ("npm-shrinkwrap.json", PackageManager::Npm),
    ("pnpm-lock.yaml", PackageManager::Pnpm),
    ("yarn.lock", PackageManager::Yarn),
    ("bun.lockb", PackageManager::Bun),
    ("bun.lock", PackageManager::Bun),
];

#[derive(Debug, Clone, Serialize)]
pub struct Artifact {
    pub path: PathBuf,
    /// Id of the ecosystem that declared it.
    pub ecosystem: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    /// Ids of the ecosystems found in the project, in catalog order.
    pub ecosystems: Vec<String>,
    /// Folders holding a manifest, the root included.
    #[serde(skip)]
    pub members: Vec<PathBuf>,
    /// Sent to the interface as items (`scan::project_items`), not a second time here.
    #[serde(skip)]
    pub artifacts: Vec<Artifact>,
    /// From the lock files. More than one is worth a warning (real case).
    pub package_managers: Vec<PackageManager>,
    /// From `.nvmrc`, `.node-version` or the `engines` field.
    pub node_version: Option<String>,
    /// Unix time of the most recent file change, artifacts excluded.
    pub last_modified: Option<i64>,
    /// Filled by [`attach_git`].
    pub git: Option<RepoStatus>,
    /// Steps to bring the project back after cleaning.
    pub regenerate: Vec<String>,
}

impl Project {
    pub fn reclaimable(&self) -> u64 {
        self.artifacts.iter().map(|a| a.usage.allocated).sum()
    }
}

/// Whether `dir` is a project of `eco`.
pub fn detects(eco: &Ecosystem, dir: &Path) -> bool {
    if !eco.detect.any_file.iter().any(|f| dir.join(f).is_file()) {
        return false;
    }
    match &eco.detect.package_json_dependency {
        None => true,
        Some(dep) => read_package_json(dir).is_some_and(|json| {
            ["dependencies", "devDependencies", "peerDependencies"]
                .iter()
                .any(|section| json.get(section).and_then(|s| s.get(dep)).is_some())
        }),
    }
}

fn read_package_json(dir: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(dir.join("package.json")).ok()?;
    serde_json::from_str(&text).ok()
}

pub(crate) fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Sub-folders of `dir` with their names. Symbolic links are not followed.
pub(crate) fn subdirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect()
}

/// An installed SDK, not a project: the Flutter SDK is a git clone full of `pubspec.yaml`
/// files, and its `build` folders are part of it.
pub fn is_tool_install(dir: &Path) -> bool {
    /// Each SDK is recognized when all of its files are present.
    const SDKS: &[&[&str]] = &[
        &["bin/flutter", "packages/flutter_tools/pubspec.yaml"],
        &["bin/dart", "bin/cache/dart-sdk"],
    ];
    SDKS.iter().any(|markers| markers.iter().all(|m| dir.join(m).exists()))
}

/// Finds the project roots below `roots`. Hidden folders and dependency folders are skipped.
pub fn find_roots(roots: &[PathBuf], ecosystems: &[Ecosystem], cancel: &AtomicBool) -> Vec<PathBuf> {
    let found = Mutex::new(Vec::new());
    roots
        .par_iter()
        .for_each(|root| visit(root, 0, ecosystems, cancel, &found));
    let mut found = found.into_inner().unwrap_or_default();
    found.sort();
    found.dedup();
    found
}

fn visit(dir: &Path, depth: usize, ecosystems: &[Ecosystem], cancel: &AtomicBool, found: &Mutex<Vec<PathBuf>>) {
    if cancel.load(Ordering::Relaxed) || depth > MAX_DEPTH {
        return;
    }
    if is_tool_install(dir) {
        return;
    }
    // The scan root itself is never a project: it is a folder of projects.
    if depth > 0 && ecosystems.iter().any(|eco| detects(eco, dir)) {
        if let Ok(mut list) = found.lock() {
            list.push(dir.to_path_buf());
        }
        return;
    }
    subdirs(dir)
        .into_par_iter()
        .filter(|(name, _)| !is_hidden(name) && !NEVER_ENTER.contains(&name.as_str()))
        .for_each(|(_, path)| visit(&path, depth + 1, ecosystems, cancel, found));
}

/// Builds the full description of the project at `root`. `seen` is shared across projects so
/// hard links (pnpm store) are counted once.
pub fn analyze(root: &Path, ecosystems: &[Ecosystem], seen: &Seen, cancel: &AtomicBool) -> Project {
    let artifact_names: BTreeSet<&str> = ecosystems
        .iter()
        .flat_map(|e| &e.artifacts)
        .map(|a| a.strip_prefix("**/").unwrap_or(a))
        .filter(|a| !a.contains('/'))
        .collect();

    let mut members = vec![root.to_path_buf()];
    collect_members(root, 0, &artifact_names, ecosystems, &mut members);
    members.sort();

    let mut ecosystem_ids = Vec::new();
    let mut paths: Vec<(PathBuf, String)> = Vec::new();
    for eco in ecosystems {
        let matching: Vec<&PathBuf> = members.iter().filter(|m| detects(eco, m)).collect();
        if matching.is_empty() {
            continue;
        }
        ecosystem_ids.push(eco.id.clone());
        for member in matching {
            for artifact in &eco.artifacts {
                match artifact.strip_prefix("**/") {
                    Some(name) => find_named(member, name, &artifact_names, &mut |p| paths.push((p, eco.id.clone()))),
                    None => {
                        let candidate = member.join(artifact);
                        let is_real_dir = candidate.symlink_metadata().map(|m| m.is_dir()).unwrap_or(false);
                        if is_real_dir {
                            paths.push((candidate, eco.id.clone()));
                        }
                    }
                }
            }
        }
    }
    // Keep each folder once, and drop folders inside another artifact.
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    let mut kept: Vec<(PathBuf, String)> = Vec::new();
    for (path, eco) in paths {
        if !kept.iter().any(|(k, _)| path.starts_with(k)) {
            kept.push((path, eco));
        }
    }
    let artifacts: Vec<Artifact> = kept
        .into_par_iter()
        .map(|(path, ecosystem)| {
            let usage = size::usage(&path, seen, cancel);
            Artifact { path, ecosystem, usage }
        })
        .collect();

    let package_managers = package_managers(&members);
    let node_version = node_version(root);
    let artifact_paths: Vec<&Path> = artifacts.iter().map(|a| a.path.as_path()).collect();
    let last_modified = latest_change(root, &artifact_paths, cancel);
    let regenerate = regenerate_steps(
        &ecosystem_ids,
        &package_managers,
        node_version.as_deref(),
        &artifacts,
        ecosystems,
    );

    Project {
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        root: root.to_path_buf(),
        ecosystems: ecosystem_ids,
        members,
        artifacts,
        package_managers,
        node_version,
        last_modified,
        git: None,
        regenerate,
    }
}

fn collect_members(
    dir: &Path,
    depth: usize,
    artifact_names: &BTreeSet<&str>,
    ecosystems: &[Ecosystem],
    members: &mut Vec<PathBuf>,
) {
    if depth >= MEMBER_DEPTH {
        return;
    }
    for (name, path) in subdirs(dir) {
        if is_hidden(&name) || NEVER_ENTER.contains(&name.as_str()) || artifact_names.contains(name.as_str()) {
            continue;
        }
        if ecosystems.iter().any(|eco| detects(eco, &path)) {
            members.push(path.clone());
        }
        collect_members(&path, depth + 1, artifact_names, ecosystems, members);
    }
}

/// Folders called `name` anywhere below `dir`, without entering other artifacts.
fn find_named(dir: &Path, name: &str, artifact_names: &BTreeSet<&str>, found: &mut dyn FnMut(PathBuf)) {
    for (child_name, path) in subdirs(dir) {
        if child_name == name {
            found(path);
        } else if child_name != ".git"
            && !NEVER_ENTER.contains(&child_name.as_str())
            && !artifact_names.contains(child_name.as_str())
        {
            find_named(&path, name, artifact_names, found);
        }
    }
}

fn package_managers(members: &[PathBuf]) -> Vec<PackageManager> {
    let mut found = BTreeSet::new();
    for member in members {
        for (file, manager) in LOCKFILES {
            if member.join(file).is_file() {
                found.insert(*manager);
            }
        }
    }
    found.into_iter().collect()
}

fn node_version(root: &Path) -> Option<String> {
    for file in [".nvmrc", ".node-version"] {
        if let Ok(text) = fs::read_to_string(root.join(file)) {
            let version = text.trim();
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }
    read_package_json(root)?
        .get("engines")?
        .get("node")?
        .as_str()
        .map(str::to_string)
}

fn unix(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

/// Most recent modification time of a file in the project, artifacts and `.git` excluded.
fn latest_change(root: &Path, artifacts: &[&Path], cancel: &AtomicBool) -> Option<i64> {
    fn walk(dir: &Path, artifacts: &[&Path], cancel: &AtomicBool) -> Option<i64> {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let entries: Vec<_> = fs::read_dir(dir).ok()?.filter_map(Result::ok).collect();
        entries
            .into_par_iter()
            .filter_map(|entry| {
                let path = entry.path();
                let name = entry.file_name();
                let meta = path.symlink_metadata().ok()?;
                if meta.is_dir() {
                    if NEVER_ENTER.contains(&name.to_string_lossy().as_ref()) || artifacts.contains(&path.as_path()) {
                        return None;
                    }
                    walk(&path, artifacts, cancel)
                } else {
                    meta.modified().ok().and_then(unix)
                }
            })
            .max()
    }
    walk(root, artifacts, cancel)
}

fn regenerate_steps(
    ecosystem_ids: &[String],
    package_managers: &[PackageManager],
    node_version: Option<&str>,
    artifacts: &[Artifact],
    ecosystems: &[Ecosystem],
) -> Vec<String> {
    let has = |id: &str| ecosystem_ids.iter().any(|e| e == id);
    let mut steps = Vec::new();
    if has("node") {
        let pm = package_managers.first().copied().unwrap_or(PackageManager::Npm).name();
        let mut step = format!("`{pm} install`");
        if let Some(version) = node_version {
            step.push_str(&format!(" avec Node {version}"));
        }
        steps.push(step);
    }
    if has("flutter") {
        steps.push("`flutter pub get`".into());
    }
    if artifacts.iter().any(|a| a.path.ends_with("ios/Pods")) {
        steps.push("`pod install` dans `ios`".into());
    }
    // Every other ecosystem adds its catalog text, so a Node + Rust project keeps `cargo build`.
    // The Tauri text already covers the Rust build of `src-tauri`.
    let covered = |id: &str| matches!(id, "node" | "flutter" | "react-native") || (id == "rust" && has("tauri"));
    steps.extend(
        ecosystems
            .iter()
            .filter(|e| has(&e.id) && !covered(&e.id))
            .map(|e| e.regenerate.clone()),
    );
    steps
}

/// Reads the repository state of each project. Slow (several git calls per project), so done
/// after the first results are shown.
pub fn attach_git(projects: &mut [Project]) {
    if !guards::git_available() {
        return;
    }
    projects.par_iter_mut().for_each(|project| {
        if let Some(repo) = guards::repo_root(&project.root) {
            project.git = guards::repo_status(&repo).ok();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use std::time::Duration;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn analyze_all(root: &Path) -> Vec<Project> {
        let catalog = Catalog::embedded().unwrap();
        let cancel = AtomicBool::new(false);
        let seen = Seen::default();
        find_roots(&[root.to_path_buf()], &catalog.ecosystems, &cancel)
            .iter()
            .map(|r| analyze(r, &catalog.ecosystems, &seen, &cancel))
            .collect()
    }

    #[test]
    fn finds_projects_and_their_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        // A Next.js app with two lock files (real case).
        write(&base.join("web-shop/package.json"), r#"{"engines":{"node":">=20"}}"#);
        write(&base.join("web-shop/pnpm-lock.yaml"), "");
        write(&base.join("web-shop/package-lock.json"), "");
        write(&base.join("web-shop/node_modules/react/index.js"), "x");
        write(&base.join("web-shop/.next/cache/file"), "x");
        write(&base.join("web-shop/src/app.tsx"), "x");
        // A Tauri app: package.json at the root, Cargo.toml in src-tauri.
        write(&base.join("Client Work/client-app/package.json"), "{}");
        write(&base.join("Client Work/client-app/.nvmrc"), "22\n");
        write(&base.join("Client Work/client-app/src-tauri/Cargo.toml"), "");
        write(&base.join("Client Work/client-app/src-tauri/target/debug/app"), "x");
        // A Python project with caches at several depths, and a venv holding its own caches.
        write(&base.join("tools/requirements.txt"), "");
        write(&base.join("tools/__pycache__/a.pyc"), "x");
        write(&base.join("tools/pkg/__pycache__/b.pyc"), "x");
        write(&base.join("tools/.venv/lib/__pycache__/c.pyc"), "x");
        // Not a project.
        write(&base.join("notes/todo.txt"), "x");

        let projects = analyze_all(base);
        let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["client-app", "tools", "web-shop"]);

        let client = &projects[0];
        assert_eq!(client.ecosystems, vec!["node", "rust", "tauri"]);
        assert_eq!(client.node_version.as_deref(), Some("22"));
        let client_artifacts: Vec<_> = client
            .artifacts
            .iter()
            .map(|a| a.path.strip_prefix(&client.root).unwrap().to_path_buf())
            .collect();
        assert_eq!(client_artifacts, vec![PathBuf::from("src-tauri/target")]);

        let tools = &projects[1];
        let tool_artifacts: Vec<_> = tools
            .artifacts
            .iter()
            .map(|a| a.path.strip_prefix(&tools.root).unwrap().to_path_buf())
            .collect();
        assert_eq!(
            tool_artifacts,
            vec![
                PathBuf::from(".venv"),
                PathBuf::from("__pycache__"),
                PathBuf::from("pkg/__pycache__")
            ]
        );

        let website = &projects[2];
        assert_eq!(
            website.package_managers,
            vec![PackageManager::Npm, PackageManager::Pnpm]
        );
        assert_eq!(website.node_version.as_deref(), Some(">=20"));
        assert_eq!(website.artifacts.len(), 2);
        assert!(website.reclaimable() > 0);
        assert_eq!(website.regenerate, vec!["`npm install` avec Node >=20"]);
    }

    #[test]
    fn skips_the_flutter_sdk() {
        let dir = tempfile::tempdir().unwrap();
        let sdk = dir.path().join("dev/flutter");
        write(&sdk.join("bin/flutter"), "#!/bin/sh");
        write(&sdk.join("packages/flutter_tools/pubspec.yaml"), "");
        write(&sdk.join("pubspec.yaml"), "");
        write(&dir.path().join("dev/app/pubspec.yaml"), "");
        let names: Vec<_> = analyze_all(dir.path()).into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["app"]);
    }

    #[test]
    fn react_native_needs_the_dependency() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = Catalog::embedded().unwrap();
        let rn = catalog.ecosystems.iter().find(|e| e.id == "react-native").unwrap();
        write(
            &dir.path().join("a/package.json"),
            r#"{"dependencies":{"react-native":"0.81.0"}}"#,
        );
        write(&dir.path().join("b/package.json"), r#"{"dependencies":{"react":"19"}}"#);
        assert!(detects(rn, &dir.path().join("a")));
        assert!(!detects(rn, &dir.path().join("b")));
    }

    #[test]
    fn activity_ignores_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("app");
        write(&root.join("package.json"), "{}");
        write(&root.join("node_modules/x/index.js"), "x");
        let old = SystemTime::now() - Duration::from_secs(90 * 24 * 3600);
        fs::File::options()
            .write(true)
            .open(root.join("package.json"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let projects = analyze_all(dir.path());
        let project = &projects[0];
        // node_modules was just written, but only package.json counts.
        assert_eq!(project.last_modified, unix(old));
    }
}
