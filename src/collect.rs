//! Configuration schema and collection engine for the `collect` subcommand.

use clap::ValueEnum;
use serde::Deserialize;
use std::collections::HashMap;

/// Archive format for collected bundles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    /// TAR archive (uncompressed)
    Tar,
    /// TAR archive compressed with gzip
    #[serde(rename = "tar.gz")]
    #[value(name = "tar.gz")]
    TarGz,
    /// ZIP archive
    Zip,
}

/// Output type for the collect operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
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

/// Check if a path string contains glob metacharacters.
fn has_glob_metacharacters(path: &str) -> bool {
    path.contains('*') || path.contains('?') || path.contains('[')
}

/// Process a single glob pattern and return matched files.
fn resolve_glob_pattern(pattern: &str) -> anyhow::Result<Vec<String>> {
    use glob::glob_with;
    use glob::MatchOptions;

    let options = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let mut matches = Vec::new();
    match glob_with(pattern, options) {
        Ok(paths) => {
            for path_result in paths {
                match path_result {
                    Ok(path) => {
                        if let Some(path_str) = path.to_str() {
                            matches.push(path_str.to_string());
                        }
                    }
                    Err(e) => return Err(anyhow::anyhow!("glob error: {}", e)),
                }
            }
        }
        Err(e) => return Err(anyhow::anyhow!("glob compile error: {}", e)),
    }

    Ok(matches)
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

    #[test]
    fn test_glob_pattern_asterisk_matches_non_subdirectories() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test files
        fs::write(temp_path.join("test1.log"), "content").expect("Failed to write file");
        fs::write(temp_path.join("test2.log"), "content").expect("Failed to write file");
        fs::write(temp_path.join("other.txt"), "content").expect("Failed to write file");

        // Create a subdirectory with a .log file
        fs::create_dir(temp_path.join("subdir")).expect("Failed to create subdir");
        fs::write(temp_path.join("subdir/nested.log"), "content").expect("Failed to write file");

        // Test glob pattern
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/*.log", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(result.is_ok(), "run() should succeed");

        // Verify only top-level .log files were copied (not the one in subdir)
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 2, "Should have copied exactly 2 .log files");
    }

    #[test]
    fn test_glob_pattern_recursive_descent() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create nested test files
        fs::write(temp_path.join("test1.log"), "content").expect("Failed to write file");
        fs::create_dir(temp_path.join("logs")).expect("Failed to create logs dir");
        fs::write(temp_path.join("logs/test2.log"), "content").expect("Failed to write file");
        fs::create_dir(temp_path.join("logs/nested")).expect("Failed to create nested dir");
        fs::write(temp_path.join("logs/nested/test3.log"), "content")
            .expect("Failed to write file");

        // Test glob pattern with **/
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/**/*.log", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(result.is_ok(), "run() should succeed");

        // Verify all .log files were copied
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(
            files.len(),
            3,
            "Should have copied all 3 .log files (test1.log, test2.log, test3.log)"
        );
    }

    #[test]
    fn test_glob_pattern_question_mark() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test files
        fs::write(temp_path.join("a.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("ab.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("abc.txt"), "content").expect("Failed to write file");

        // Test glob pattern with ?
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/??.txt", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(result.is_ok(), "run() should succeed");

        // Verify exactly one file matched (ab.txt)
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 1, "Should have copied exactly 1 file (ab.txt)");
    }

    #[test]
    fn test_glob_pattern_character_class() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test files
        fs::write(temp_path.join("a.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("b.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("c.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("d.txt"), "content").expect("Failed to write file");

        // Test glob pattern with character class
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/[abc].txt", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(result.is_ok(), "run() should succeed");

        // Verify exactly 3 files matched (a.txt, b.txt, c.txt)
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(
            files.len(),
            3,
            "Should have copied 3 files (a.txt, b.txt, c.txt)"
        );
    }

    #[test]
    fn test_glob_pattern_range_class() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test files
        fs::write(temp_path.join("a.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("b.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("c.txt"), "content").expect("Failed to write file");
        fs::write(temp_path.join("z.txt"), "content").expect("Failed to write file");

        // Test glob pattern with range class
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/[a-c].txt", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(result.is_ok(), "run() should succeed");

        // Verify exactly 3 files matched (a.txt, b.txt, c.txt)
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(
            files.len(),
            3,
            "Should have copied 3 files (a.txt, b.txt, c.txt)"
        );
    }

    #[test]
    fn test_glob_zero_matches_required_fails() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create a file that won't match
        fs::write(temp_path.join("other.txt"), "content").expect("Failed to write file");

        // Test glob pattern that matches zero files (required)
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/*.log", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(
            result.is_err(),
            "run() should fail for required glob with zero matches"
        );
    }

    #[test]
    fn test_glob_zero_matches_optional_skips() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create a file that won't match
        fs::write(temp_path.join("other.txt"), "content").expect("Failed to write file");

        // Test glob pattern that matches zero files (optional)
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/*.log", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: None,
                        optional: true,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(
            result.is_ok(),
            "run() should succeed for optional glob with zero matches"
        );

        // Verify output is empty
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 0, "Should have copied no files");
    }

    #[test]
    fn test_glob_trailing_slash_to_multi_match() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create multiple files
        fs::write(temp_path.join("file1.log"), "content").expect("Failed to write file");
        fs::write(temp_path.join("file2.log"), "content").expect("Failed to write file");

        // Test glob pattern with trailing-slash to
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/*.log", temp_path.display());
        let to_dir = format!("{}/", out_dir.path().display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: Some(to_dir),
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(result.is_ok(), "run() should succeed");

        // Verify files were copied with basenames
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 2, "Should have copied 2 files");
    }

    #[test]
    fn test_glob_non_trailing_slash_to_multi_match_fails() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create multiple files
        fs::write(temp_path.join("file1.log"), "content").expect("Failed to write file");
        fs::write(temp_path.join("file2.log"), "content").expect("Failed to write file");

        // Test glob pattern with non-trailing-slash to (should fail)
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/*.log", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: Some(format!("{}/output.log", out_dir.path().display())),
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(
            result.is_err(),
            "run() should fail with multi-match and non-trailing-slash to"
        );
    }

    #[test]
    fn test_glob_bare_entry_multi_match() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create multiple files
        fs::write(temp_path.join("file1.log"), "content").expect("Failed to write file");
        fs::write(temp_path.join("file2.log"), "content").expect("Failed to write file");

        // Test bare entry with glob pattern (no to specified)
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/*.log", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = HashMap::new();
                bundles.insert("test".to_string(), vec![Entry::Bare(pattern)]);
                bundles
            },
        };

        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["test".to_string()],
        );
        assert!(result.is_ok(), "run() should succeed");

        // Verify files were copied with basenames
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 2, "Should have copied 2 files with basenames");
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

                // Check if this is a glob pattern
                if has_glob_metacharacters(&from) {
                    // Resolve glob pattern
                    let matches = match resolve_glob_pattern(&from) {
                        Ok(m) => m,
                        Err(e) => {
                            if optional {
                                continue;
                            }
                            return Err(e);
                        }
                    };

                    // Handle zero matches
                    if matches.is_empty() {
                        if optional {
                            continue;
                        }
                        return Err(anyhow::anyhow!("glob pattern matched zero files: {}", from));
                    }

                    // Handle multi-match with non-trailing-slash to
                    if matches.len() > 1 {
                        if let Some(to) = &to_opt {
                            if !to.ends_with('/') {
                                return Err(anyhow::anyhow!(
                                    "multiple matches for glob pattern with non-trailing-slash 'to': {}",
                                    from
                                ));
                            }
                        }
                    }

                    // Process each match
                    for matched_file in matches {
                        let to_path = if let Some(to) = &to_opt {
                            // Expand environment variables in to
                            let expanded_to = expand(to, |name| std::env::var(name).ok())
                                .context("failed to expand 'to' path")?;

                            if expanded_to.ends_with('/') {
                                // Use basename under directory
                                let basename = Path::new(&matched_file)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .ok_or_else(|| {
                                        anyhow::anyhow!(
                                            "cannot determine basename: {}",
                                            matched_file
                                        )
                                    })?;
                                format!("{}{}", expanded_to, basename)
                            } else {
                                // Use exact path (should only be single match)
                                expanded_to
                            }
                        } else {
                            // Use source filename as destination
                            Path::new(&matched_file)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string())
                                .ok_or_else(|| {
                                    anyhow::anyhow!("cannot determine destination filename")
                                })?
                        };

                        let dest_full_path = Path::new(out).join(&to_path);

                        // Copy the file
                        fs::copy(&matched_file, &dest_full_path).context("failed to copy file")?;

                        // Log the collection
                        log::info!("collected {} -> {}", matched_file, to_path);
                    }
                } else {
                    // Non-glob path: use existing logic
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
                            .ok_or_else(|| {
                                anyhow::anyhow!("cannot determine destination filename")
                            })?
                    };

                    let dest_full_path = Path::new(out).join(&to_path);

                    // Copy the file
                    fs::copy(&from, &dest_full_path).context("failed to copy file")?;

                    // Log the collection
                    log::info!("collected {} -> {}", from, to_path);
                }
            }
        }
    }

    // Print output path to stdout
    println!("{}", out);

    Ok(())
}
