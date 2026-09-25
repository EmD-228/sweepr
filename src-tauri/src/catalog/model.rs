//! Data model of the cleanup catalog. Rules are written in TOML files under
//! `src-tauri/catalog/` and deserialized into these types.

use serde::{Deserialize, Serialize};

/// Risk scale from SPEC section 6. Stored as an integer (0 to 3) in the TOML files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum Risk {
    /// Pure cache, recreated automatically.
    None = 0,
    /// Recreated, but costs time, network or a small action.
    Low = 1,
    /// May contain work: the user ticks each item.
    Review = 2,
    /// Cannot be recreated: temporary trash and double confirmation.
    Irreplaceable = 3,
}

impl TryFrom<u8> for Risk {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Risk::None),
            1 => Ok(Risk::Low),
            2 => Ok(Risk::Review),
            3 => Ok(Risk::Irreplaceable),
            other => Err(format!("risk must be between 0 and 3, got {other}")),
        }
    }
}

impl From<Risk> for u8 {
    fn from(risk: Risk) -> u8 {
        risk as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Macos,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Macos
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// Which audience sees the rule: the default simple mode, or the developer module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    General,
    Developer,
}

/// How the matched paths are presented to the user.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// One line for the whole rule.
    #[default]
    None,
    /// One line per application, named after it (`com.spotify.client` → "Spotify").
    App,
    /// One line per Xcode project (`Runner-<hash>` → "Runner").
    XcodeProject,
}

/// What a rule frees, as the storage chart groups it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Caches,
    Trash,
    /// Logs, old installers, downloaded attachments.
    Temporary,
    /// The user's own files: backups, attachments, large files, duplicates.
    Personal,
    Developer,
}

/// A fixed command line. Never passed through a shell.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Argv {
    /// Bare program name, resolved through `PATH`.
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Items that only code can list: one simulator, one Docker volume, one NDK version...
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    LargeFiles,
    Duplicates,
    IosBackups,
    IosSimulators,
    IosRuntimes,
    AndroidEmulators,
    AndroidSystemImages,
    AndroidNdk,
    DockerProjects,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    /// Delete the files and folders matched by these glob patterns.
    Paths {
        paths: Vec<String>,
        /// Patterns never deleted, even when matched.
        #[serde(default)]
        exclude: Vec<String>,
    },
    /// Run an official cleanup command, always from the home folder
    /// (inside a project, Corepack may refuse a package manager the project does not use).
    Command {
        /// Bare program name, resolved through `PATH`.
        program: String,
        #[serde(default)]
        args: Vec<String>,
        /// Paths whose size estimates what the command frees. Empty when the gain cannot be known in advance.
        #[serde(default)]
        measure: Vec<String>,
    },
    /// Items listed by a dedicated provider in Rust.
    Provider { provider: ProviderId },
}

/// A cleanup rule. Sent to the interface as is, without the fields only the engine needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub profile: Profile,
    pub category: Category,
    #[serde(skip_serializing)]
    pub platforms: Vec<Os>,
    /// Short name shown in the list.
    pub title: String,
    /// One plain-language sentence: what this is.
    pub summary: String,
    pub risk: Risk,
    /// What the user loses by running the action.
    pub loses: String,
    /// How the deleted data comes back.
    pub regenerate: String,
    /// Programs that must be installed for the rule to apply (for example `npm`).
    #[serde(default, skip_serializing)]
    pub requires: Vec<String>,
    #[serde(default, skip_serializing)]
    pub group_by: GroupBy,
    /// Extra caveat shown before confirmation.
    #[serde(default)]
    pub note: Option<String>,
    #[serde(skip_serializing)]
    pub target: Target,
}

/// How a project of an ecosystem is recognized.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    /// The project root contains at least one of these files.
    pub any_file: Vec<String>,
    /// `package.json` must also declare this dependency.
    #[serde(default)]
    pub package_json_dependency: Option<String>,
}

/// Build and dependency folders of one project ecosystem (SPEC section 5.4).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecosystem {
    pub id: String,
    pub name: String,
    pub detect: Detect,
    /// Folders relative to the project root. A `**/` prefix matches the name at any depth.
    pub artifacts: Vec<String>,
    pub risk: Risk,
    #[serde(default)]
    pub official_command: Option<Argv>,
    /// How to bring the project back to a working state.
    pub regenerate: String,
    #[serde(default)]
    pub note: Option<String>,
}

/// Content of one catalog file.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFile {
    #[serde(default)]
    pub rule: Vec<Rule>,
    #[serde(default)]
    pub ecosystem: Vec<Ecosystem>,
}
