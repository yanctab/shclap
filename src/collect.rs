//! Configuration schema and collection engine for the `collect` subcommand.

use serde::Deserialize;
use std::collections::HashMap;

/// Archive format for collected bundles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    /// TAR archive (uncompressed)
    Tar,
    /// TAR archive compressed with gzip
    #[serde(rename = "tar.gz")]
    TarGz,
    /// ZIP archive
    Zip,
}

/// Output type for the collect operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputType {
    /// Output as a directory
    Dir,
    /// Output as an archive
    Archive,
}

/// A single entry in a bundle: either a bare string or an object with metadata.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    /// Bare string form: treated as `from` with default `to` and `optional: false`.
    Bare(String),
    /// Object form with explicit fields.
    Object(EntryObject),
}

/// Object form of an entry with explicit fields.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryObject {
    /// Source path (required)
    pub from: String,
    /// Destination path (optional)
    pub to: Option<String>,
    /// Whether this entry is optional (defaults to false)
    #[serde(default)]
    pub optional: bool,
}

/// Top-level configuration for the `collect` subcommand.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectConfig {
    /// Schema version (defaults to 1 if omitted).
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Bundles mapping: bundle name to list of entries.
    pub bundles: HashMap<String, Vec<Entry>>,
}

fn default_schema_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version_omitted_defaults_to_1() {
        let json = r#"{"bundles": {}}"#;
        let config: CollectConfig = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(config.schema_version, 1);
    }

    #[test]
    fn test_schema_version_1_accepted() {
        let json = r#"{"schema_version": 1, "bundles": {}}"#;
        let config: CollectConfig = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(config.schema_version, 1);
    }

    #[test]
    fn test_unknown_top_level_field_rejected() {
        let json = r#"{"schema_version": 1, "bundles": {}, "unknown_field": true}"#;
        let result: Result<CollectConfig, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Should reject unknown top-level field");
    }

    #[test]
    fn test_bare_string_entry_deserialises() {
        let json = r#"{"bundles": {"test": ["/path/to/file"]}}"#;
        let config: CollectConfig = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(config.bundles.len(), 1);
        assert_eq!(config.bundles["test"].len(), 1);
        match &config.bundles["test"][0] {
            Entry::Bare(s) => assert_eq!(s, "/path/to/file"),
            _ => panic!("Expected bare string entry"),
        }
    }

    #[test]
    fn test_object_entry_with_all_fields_deserialises() {
        let json = r#"{"bundles": {"test": [{"from": "/src", "to": "/dst", "optional": true}]}}"#;
        let config: CollectConfig = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(config.bundles["test"].len(), 1);
        match &config.bundles["test"][0] {
            Entry::Object(obj) => {
                assert_eq!(obj.from, "/src");
                assert_eq!(obj.to, Some("/dst".to_string()));
                assert_eq!(obj.optional, true);
            }
            _ => panic!("Expected object entry"),
        }
    }

    #[test]
    fn test_object_entry_missing_from_rejected() {
        let json = r#"{"bundles": {"test": [{"to": "/dst"}]}}"#;
        let result: Result<CollectConfig, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Should reject entry without 'from' field");
    }

    #[test]
    fn test_unknown_field_inside_entry_object_rejected() {
        let json = r#"{"bundles": {"test": [{"from": "/src", "unknown": true}]}}"#;
        let result: Result<CollectConfig, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "Should reject unknown field inside entry object"
        );
    }

    #[test]
    fn test_optional_defaults_to_false() {
        let json = r#"{"bundles": {"test": [{"from": "/src"}]}}"#;
        let config: CollectConfig = serde_json::from_str(json).expect("Failed to deserialize");
        match &config.bundles["test"][0] {
            Entry::Object(obj) => assert_eq!(obj.optional, false),
            _ => panic!("Expected object entry"),
        }
    }
}

/// Run the collection engine: copy matched files from config to destination.
pub fn run(
    config: CollectConfig,
    out: &str,
    _output_type: OutputType,
    _archive_format: Option<ArchiveFormat>,
    bundles: &[String],
) -> anyhow::Result<()> {
    use crate::expand::expand;
    use anyhow::Context;
    use std::fs;
    use std::path::Path;

    // Initialize logging
    crate::logging::init_once();

    // Create output directory
    fs::create_dir_all(out).context("failed to create output directory")?;

    // Process each requested bundle
    for bundle_name in bundles {
        if let Some(entries) = config.bundles.get(bundle_name) {
            for entry in entries {
                // Extract from and to paths
                let (from_raw, to_opt, optional) = match entry {
                    Entry::Bare(path) => (path.clone(), None, false),
                    Entry::Object(obj) => (obj.from.clone(), obj.to.clone(), obj.optional),
                };

                // Expand environment variables in from
                let from = match expand(&from_raw, |name| std::env::var(name).ok()) {
                    Ok(expanded) => expanded,
                    Err(e) => {
                        if optional {
                            continue;
                        }
                        return Err(anyhow::anyhow!("{}", e));
                    }
                };

                // Check if source exists
                if !Path::new(&from).exists() {
                    if optional {
                        continue;
                    }
                    return Err(anyhow::anyhow!("source file not found: {}", from));
                }

                // Determine destination
                let to_path = if let Some(to) = to_opt {
                    // Expand environment variables in to
                    expand(&to, |name| std::env::var(name).ok())
                        .context("failed to expand 'to' path")?
                } else {
                    // Use source filename as destination
                    Path::new(&from)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .ok_or_else(|| anyhow::anyhow!("cannot determine destination filename"))?
                };

                let dest_full_path = Path::new(out).join(&to_path);

                // Copy the file
                fs::copy(&from, &dest_full_path).context("failed to copy file")?;

                // Log the collection
                log::info!("collected {} -> {}", from, to_path);
            }
        }
    }

    // Print output path to stdout
    println!("{}", out);

    Ok(())
}
