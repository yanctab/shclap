//! Configuration schema and collection engine for the `collect` subcommand.

use clap::ValueEnum;
use indexmap::IndexMap;
use serde::Deserialize;

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
    /// Bundles mapping: bundle name to list of entries (preserves insertion order).
    pub bundles: IndexMap<String, Vec<Entry>>,
}

fn default_schema_version() -> u32 {
    1
}

/// Check if a path string contains glob metacharacters.
fn has_glob_metacharacters(path: &str) -> bool {
    path.contains('*') || path.contains('?') || path.contains('[')
}

/// Validate that a destination path is safe (relative, no .., no absolute).
fn validate_to_path(to_path: &str) -> anyhow::Result<()> {
    use std::path::Path;

    // Check if path is absolute
    if Path::new(to_path).is_absolute() {
        return Err(anyhow::anyhow!("'to' path must be relative: {}", to_path));
    }

    // Check for .. segments
    for component in Path::new(to_path).components() {
        if component.as_os_str() == ".." {
            return Err(anyhow::anyhow!(
                "'to' path cannot contain '..': {}",
                to_path
            ));
        }
    }

    Ok(())
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
                let mut bundles = IndexMap::new();
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
                let mut bundles = IndexMap::new();
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
                let mut bundles = IndexMap::new();
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
                let mut bundles = IndexMap::new();
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
                let mut bundles = IndexMap::new();
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
                let mut bundles = IndexMap::new();
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
                let mut bundles = IndexMap::new();
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

        // Test glob pattern with trailing-slash to (use relative path)
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let pattern = format!("{}/*.log", temp_path.display());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: pattern,
                        to: Some("dest/".to_string()),
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

        // Verify files were copied with basenames under dest/
        let files: Vec<_> = fs::read_dir(out_dir.path().join("dest"))
            .expect("Failed to read dest dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 2, "Should have copied 2 files into dest/");
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
                let mut bundles = IndexMap::new();
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
                let mut bundles = IndexMap::new();
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

    #[test]
    fn test_env_expansion_happy_path() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create a source file
        let src_file = temp_path.join("source.txt");
        fs::write(&src_file, "content").expect("Failed to write source file");

        // Set environment variable
        let var_name = "TEST_SRC_PATH";
        std::env::set_var(var_name, src_file.to_str().unwrap());

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: format!("${}", var_name),
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
        assert!(result.is_ok(), "run() should succeed with env expansion");

        // Verify file was copied
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 1, "Should have copied 1 file");
    }

    #[test]
    fn test_undefined_variable_error() {
        use tempfile::TempDir;

        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Clear the variable to ensure it's undefined
        std::env::remove_var("UNDEFINED_TEST_VAR_12345");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: "$UNDEFINED_TEST_VAR_12345".to_string(),
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

        assert!(result.is_err(), "run() should fail with undefined variable");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("undefined variable"),
            "Error should mention undefined variable: {}",
            err_msg
        );
    }

    #[test]
    fn test_to_safety_rejects_absolute_path() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create source file
        let src_file = temp_path.join("source.txt");
        fs::write(&src_file, "content").expect("Failed to write source file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: src_file.to_str().unwrap().to_string(),
                        to: Some("/absolute/path/to/dest.txt".to_string()),
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

        assert!(result.is_err(), "run() should reject absolute path in to");
    }

    #[test]
    fn test_to_safety_rejects_parent_directory() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create source file
        let src_file = temp_path.join("source.txt");
        fs::write(&src_file, "content").expect("Failed to write source file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: src_file.to_str().unwrap().to_string(),
                        to: Some("../escaped.txt".to_string()),
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

        assert!(result.is_err(), "run() should reject .. in to path");
    }

    #[test]
    fn test_optional_entry_skips_missing_source() {
        use tempfile::TempDir;

        let out_dir = TempDir::new().expect("Failed to create output dir");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: "/nonexistent/file.txt".to_string(),
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
            "run() should succeed with optional missing entry"
        );
    }

    #[test]
    fn test_last_wins_collision() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create two source files with different content
        let src_file1 = temp_path.join("file1.txt");
        let src_file2 = temp_path.join("file2.txt");
        fs::write(&src_file1, "content1").expect("Failed to write file1");
        fs::write(&src_file2, "content2").expect("Failed to write file2");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![
                        Entry::Object(EntryObject {
                            from: src_file1.to_str().unwrap().to_string(),
                            to: Some("dest.txt".to_string()),
                            optional: false,
                        }),
                        Entry::Object(EntryObject {
                            from: src_file2.to_str().unwrap().to_string(),
                            to: Some("dest.txt".to_string()),
                            optional: false,
                        }),
                    ],
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

        assert!(result.is_ok(), "run() should succeed with collision");

        // Verify that the later file (content2) won
        let dest_file = out_dir.path().join("dest.txt");
        let content = fs::read_to_string(&dest_file).expect("Failed to read dest file");
        assert_eq!(content, "content2", "Later entry should win in collision");
    }

    #[test]
    fn test_directory_source_rejection() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create a subdirectory
        let src_dir = temp_path.join("subdir");
        fs::create_dir(&src_dir).expect("Failed to create subdirectory");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: src_dir.to_str().unwrap().to_string(),
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

        assert!(result.is_err(), "run() should reject directory source");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("is a directory"),
            "Error should mention directory: {}",
            err_msg
        );
    }

    #[test]
    fn test_bundle_selector_single_bundle() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create source files for two bundles
        fs::write(temp_path.join("binaries.txt"), "binary content")
            .expect("Failed to write binaries file");
        fs::write(temp_path.join("logs.txt"), "log content").expect("Failed to write logs file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "binaries".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("binaries.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles.insert(
                    "logs".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("logs.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        // Request only the "binaries" bundle
        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["binaries".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed when requesting a valid bundle"
        );

        // Verify only binaries.txt was copied, not logs.txt
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();

        assert_eq!(files.len(), 1, "Should have copied only 1 file");
        assert!(
            files.contains(&"binaries.txt".to_string()),
            "Should have copied binaries.txt"
        );
        assert!(
            !files.contains(&"logs.txt".to_string()),
            "Should not have copied logs.txt"
        );
    }

    #[test]
    fn test_bundle_selector_multi_bundle() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create source files for three bundles
        fs::write(temp_path.join("binaries.txt"), "binary content")
            .expect("Failed to write binaries file");
        fs::write(temp_path.join("logs.txt"), "log content").expect("Failed to write logs file");
        fs::write(temp_path.join("config.txt"), "config content")
            .expect("Failed to write config file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "binaries".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("binaries.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles.insert(
                    "logs".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("logs.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles.insert(
                    "config".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("config.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        // Request both "binaries" and "logs" bundles
        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["binaries".to_string(), "logs".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed when requesting valid bundles"
        );

        // Verify both requested files were copied, but not config.txt
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();

        assert_eq!(files.len(), 2, "Should have copied exactly 2 files");
        assert!(
            files.contains(&"binaries.txt".to_string()),
            "Should have copied binaries.txt"
        );
        assert!(
            files.contains(&"logs.txt".to_string()),
            "Should have copied logs.txt"
        );
        assert!(
            !files.contains(&"config.txt".to_string()),
            "Should not have copied config.txt"
        );
    }

    #[test]
    fn test_bundle_selector_omitted_processes_all() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create source files for two bundles
        fs::write(temp_path.join("binaries.txt"), "binary content")
            .expect("Failed to write binaries file");
        fs::write(temp_path.join("logs.txt"), "log content").expect("Failed to write logs file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "binaries".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("binaries.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles.insert(
                    "logs".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("logs.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        // Omit --bundle entirely (pass empty selector)
        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &[],
        );

        assert!(
            result.is_ok(),
            "run() should succeed when no bundle selector provided"
        );

        // Verify all files were copied
        let files: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();

        assert_eq!(files.len(), 2, "Should have copied all 2 files");
        assert!(
            files.contains(&"binaries.txt".to_string()),
            "Should have copied binaries.txt"
        );
        assert!(
            files.contains(&"logs.txt".to_string()),
            "Should have copied logs.txt"
        );
    }

    #[test]
    fn test_bundle_selector_unknown_bundle_errors() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create source files
        fs::write(temp_path.join("binaries.txt"), "binary content")
            .expect("Failed to write binaries file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "binaries".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("binaries.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        // Request an unknown bundle
        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["unknown_bundle".to_string()],
        );

        assert!(
            result.is_err(),
            "run() should fail when requesting unknown bundle"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("unknown bundle name"),
            "Error should mention unknown bundle: {}",
            err_msg
        );
        assert!(
            err_msg.contains("unknown_bundle"),
            "Error should name the unknown bundle: {}",
            err_msg
        );
    }

    #[test]
    fn test_bundle_selector_skipped_bundles_produce_no_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let out_dir = TempDir::new().expect("Failed to create output dir");

        // Create source files for two bundles
        fs::write(temp_path.join("binaries.txt"), "binary content")
            .expect("Failed to write binaries file");
        fs::write(temp_path.join("logs.txt"), "log content").expect("Failed to write logs file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "binaries".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("binaries.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles.insert(
                    "logs".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("logs.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        // Request only "binaries" bundle
        let result = run(
            config,
            out_dir.path().to_str().unwrap(),
            OutputType::Dir,
            None,
            &["binaries".to_string()],
        );

        assert!(result.is_ok(), "run() should succeed");

        // Verify output directory contains only binaries.txt
        let entries: Vec<_> = fs::read_dir(out_dir.path())
            .expect("Failed to read output dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();

        assert_eq!(entries.len(), 1, "Should have exactly 1 file");
        assert_eq!(entries[0], "binaries.txt", "Should only have binaries.txt");
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

    // If bundles selector is non-empty, validate all names exist in config
    if !bundles.is_empty() {
        for bundle_name in bundles {
            if !config.bundles.contains_key(bundle_name) {
                return Err(anyhow::anyhow!("unknown bundle name: {}", bundle_name));
            }
        }
    }

    // Determine which bundles to process
    let bundles_to_process: Vec<&String> = if bundles.is_empty() {
        // If no selector, process all bundles in config
        config.bundles.keys().collect()
    } else {
        // If selector provided, only process those bundles (in config order)
        config
            .bundles
            .keys()
            .filter(|k| bundles.contains(k))
            .collect()
    };

    // Process each requested bundle
    for bundle_name in bundles_to_process {
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
                        // Check if source is a directory
                        if Path::new(&matched_file).is_dir() {
                            return Err(anyhow::anyhow!(
                                "'{}' is a directory; use '{}/**/*' to include contents",
                                matched_file,
                                matched_file
                            ));
                        }

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

                        // Validate destination path safety
                        validate_to_path(&to_path)?;

                        let dest_full_path = Path::new(out).join(&to_path);

                        // Create parent directories if needed
                        if let Some(parent) = dest_full_path.parent() {
                            fs::create_dir_all(parent)
                                .context("failed to create parent directory")?;
                        }

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

                    // Check if source is a directory
                    if Path::new(&from).is_dir() {
                        return Err(anyhow::anyhow!(
                            "'{}' is a directory; use '{}/**/*' to include contents",
                            from,
                            from
                        ));
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

                    // Validate destination path safety
                    validate_to_path(&to_path)?;

                    let dest_full_path = Path::new(out).join(&to_path);

                    // Create parent directories if needed
                    if let Some(parent) = dest_full_path.parent() {
                        fs::create_dir_all(parent).context("failed to create parent directory")?;
                    }

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
