//! Plain names for folders whose real name is technical: `com.spotify.client` is shown as
//! "Spotify", `BraveSoftware` and `com.brave.Browser` both as "Brave".

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::catalog::GroupBy;

/// Folder names that are not bundle identifiers, with the name people know.
const ALIASES: &[(&str, &str)] = &[
    ("Google", "Google Chrome"),
    ("BraveSoftware", "Brave Browser"),
    ("com.microsoft.VSCode", "Visual Studio Code"),
    ("Mozilla", "Firefox"),
    ("company.thebrowser.Browser", "Arc"),
    ("Homebrew", "Homebrew"),
    ("pip", "Python (pip)"),
    ("typescript", "TypeScript"),
    ("ms-playwright", "Playwright"),
    ("ms-playwright-go", "Playwright"),
    ("go-build", "Go"),
    ("next-swc", "Next.js"),
    ("org.swift.swiftpm", "Swift Package Manager"),
    ("JetBrains", "JetBrains"),
];

/// Suffixes of cache folders that belong to an application's updater.
const UPDATER_SUFFIXES: &[&str] = &[".ShipIt", "-updater", ".Sparkle"];

#[derive(Debug, Default)]
pub struct Namer {
    /// Lowercased bundle identifier → application name.
    bundles: HashMap<String, String>,
}

impl Namer {
    /// Reads the names of the installed applications.
    pub fn load(home: &Path) -> Namer {
        let mut namer = Namer::default();
        if cfg!(target_os = "macos") {
            let folders = [
                Path::new("/Applications").to_path_buf(),
                Path::new("/Applications/Utilities").to_path_buf(),
                Path::new("/System/Applications").to_path_buf(),
                home.join("Applications"),
            ];
            for folder in folders {
                namer.read_folder(&folder);
            }
        }
        namer
    }

    fn read_folder(&mut self, folder: &Path) {
        let Ok(entries) = fs::read_dir(folder) else { return };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "app") {
                if let Some((id, name)) = read_bundle(&path) {
                    self.bundles.insert(id.to_lowercase(), name);
                }
            }
        }
    }

    pub fn with_bundle(mut self, id: &str, name: &str) -> Namer {
        self.bundles.insert(id.to_lowercase(), name.to_string());
        self
    }

    /// Plain name of a folder listed by a rule, following how the rule groups its folders.
    pub fn child_title(&self, group_by: GroupBy, path: &Path) -> String {
        let folder = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match group_by {
            GroupBy::App => self.app_name(&folder).unwrap_or(folder),
            GroupBy::XcodeProject => strip_derived_data_hash(&folder).to_string(),
            GroupBy::None => folder,
        }
    }

    fn app_name(&self, folder: &str) -> Option<String> {
        if let Some((_, name)) = ALIASES.iter().find(|(alias, _)| *alias == folder) {
            return Some(name.to_string());
        }
        if let Some(name) = self.bundles.get(&folder.to_lowercase()) {
            return Some(name.clone());
        }
        let base = UPDATER_SUFFIXES.iter().find_map(|s| folder.strip_suffix(s))?;
        self.app_name(base).or_else(|| Some(base.to_string()))
    }
}

/// Bundle identifier and display name of an application.
fn read_bundle(app: &Path) -> Option<(String, String)> {
    let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
    let dict = info.as_dictionary()?;
    let id = dict.get("CFBundleIdentifier")?.as_string()?.to_string();
    let name = ["CFBundleDisplayName", "CFBundleName"]
        .iter()
        .find_map(|key| dict.get(key).and_then(|v| v.as_string()).filter(|s| !s.trim().is_empty()))
        .map(str::to_string)
        .or_else(|| app.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
    Some((id, name))
}

/// Xcode names DerivedData folders `<Project>-<28 lowercase letters>`.
fn strip_derived_data_hash(folder: &str) -> &str {
    match folder.rsplit_once('-') {
        Some((name, hash)) if hash.len() == 28 && hash.chars().all(|c| c.is_ascii_lowercase()) => name,
        _ => folder,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_caches_after_their_application() {
        let namer = Namer::default().with_bundle("com.spotify.client", "Spotify").with_bundle("com.brave.Browser", "Brave Browser");
        let title = |folder: &str| namer.child_title(GroupBy::App, &Path::new("/c").join(folder));
        assert_eq!(title("com.spotify.client"), "Spotify");
        assert_eq!(title("com.microsoft.VSCode.ShipIt"), "Visual Studio Code");
        assert_eq!(title("maestro-studio-updater"), "maestro-studio");
        assert_eq!(title("BraveSoftware"), title("com.brave.Browser"));
        assert_eq!(title("unknown.thing"), "unknown.thing");
    }

    #[test]
    fn strips_the_derived_data_hash() {
        let path = Path::new("/d/Runner-abcdefghijklmnopqrstuvwxyzab");
        assert_eq!(Namer::default().child_title(GroupBy::XcodeProject, path), "Runner");
        assert_eq!(strip_derived_data_hash("my-app"), "my-app");
    }
}
