//! The cleanup catalog: rules and project ecosystems, loaded from the TOML
//! files embedded in the binary.

mod model;
mod validate;

pub use model::*;

use thiserror::Error;

/// Catalog files shipped with the app. Adding a file here is enough to load it.
const EMBEDDED: &[(&str, &str)] = &[
    ("macos.toml", include_str!("../../catalog/macos.toml")),
    ("dev-tools.toml", include_str!("../../catalog/dev-tools.toml")),
    ("ecosystems.toml", include_str!("../../catalog/ecosystems.toml")),
];

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("{file}: {source}")]
    Parse { file: String, source: toml::de::Error },
    #[error("invalid catalog:\n{}", .0.join("\n"))]
    Invalid(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub rules: Vec<Rule>,
    pub ecosystems: Vec<Ecosystem>,
}

impl Catalog {
    /// Loads and validates the catalog shipped with the app.
    pub fn embedded() -> Result<Catalog, CatalogError> {
        Catalog::from_sources(EMBEDDED)
    }

    pub fn from_sources(sources: &[(&str, &str)]) -> Result<Catalog, CatalogError> {
        let mut files = Vec::with_capacity(sources.len());
        for (name, text) in sources {
            let file: CatalogFile =
                toml::from_str(text).map_err(|source| CatalogError::Parse { file: name.to_string(), source })?;
            files.push((*name, file));
        }
        let refs: Vec<(&str, &CatalogFile)> = files.iter().map(|(n, f)| (*n, f)).collect();
        let errors = validate::validate_files(&refs);
        if !errors.is_empty() {
            return Err(CatalogError::Invalid(errors));
        }
        let mut catalog = Catalog::default();
        for (_, file) in files {
            catalog.rules.extend(file.rule);
            catalog.ecosystems.extend(file.ecosystem);
        }
        Ok(catalog)
    }

    /// Rules that apply to this operating system.
    pub fn rules_for(&self, os: Os) -> impl Iterator<Item = &Rule> {
        self.rules.iter().filter(move |r| r.platforms.contains(&os))
    }

    pub fn rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_valid() {
        let catalog = Catalog::embedded().unwrap_or_else(|e| panic!("{e}"));
        assert!(catalog.rules_for(Os::Macos).count() > 10);
        assert!(!catalog.ecosystems.is_empty());
    }
}
