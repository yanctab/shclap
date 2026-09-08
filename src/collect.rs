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

/// Detect archive format from file extension.
fn detect_format_from_extension(path: &str) -> anyhow::Result<ArchiveFormat> {
    if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
        Ok(ArchiveFormat::TarGz)
    } else if path.ends_with(".tar") {
        Ok(ArchiveFormat::Tar)
    } else if path.ends_with(".zip") {
        Ok(ArchiveFormat::Zip)
    } else {
        Err(anyhow::anyhow!(
            "unknown archive format: {}; use --archive-format to specify",
            path
        ))
    }
}

/// Write an archive with the collected file pairs.
fn write_archive(
    format: ArchiveFormat,
    out_path: &str,
    pairs: &[(String, String)],
) -> anyhow::Result<()> {
    use anyhow::Context;
    use std::fs::File;
    use std::io::{self, Write};

    match format {
        ArchiveFormat::Tar => {
            if out_path == "-" {
                let stdout = io::stdout();
                let mut builder = tar::Builder::new(stdout.lock());
                for (src_path, dest_path) in pairs {
                    builder.append_file(dest_path, &mut File::open(src_path)?)?;
                }
                builder.finish().context("failed to finalize tar archive")?;
            } else {
                let file = File::create(out_path).context("failed to create tar archive")?;
                let mut builder = tar::Builder::new(file);
                for (src_path, dest_path) in pairs {
                    builder.append_file(dest_path, &mut File::open(src_path)?)?;
                }
                builder.finish().context("failed to finalize tar archive")?;
            }
        }
        ArchiveFormat::TarGz => {
            if out_path == "-" {
                let stdout = io::stdout();
                let encoder =
                    flate2::write::GzEncoder::new(stdout.lock(), flate2::Compression::default());
                let mut builder = tar::Builder::new(encoder);
                for (src_path, dest_path) in pairs {
                    builder.append_file(dest_path, &mut File::open(src_path)?)?;
                }
                builder
                    .finish()
                    .context("failed to finalize tar.gz archive")?;
            } else {
                let file = File::create(out_path).context("failed to create tar.gz archive")?;
                let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
                let mut builder = tar::Builder::new(encoder);
                for (src_path, dest_path) in pairs {
                    builder.append_file(dest_path, &mut File::open(src_path)?)?;
                }
                builder
                    .finish()
                    .context("failed to finalize tar.gz archive")?;
            }
        }
        ArchiveFormat::Zip => {
            if out_path == "-" {
                let cursor = io::Cursor::new(Vec::new());
                let mut writer = zip::ZipWriter::new(cursor);
                for (src_path, dest_path) in pairs {
                    let file_content = std::fs::read(src_path)
                        .context(format!("failed to read file: {}", src_path))?;
                    writer.start_file(
                        dest_path.clone(),
                        zip::write::FileOptions::default()
                            .compression_method(zip::CompressionMethod::Stored),
                    )?;
                    writer.write_all(&file_content)?;
                }
                let buffer = writer.finish().context("failed to finalize zip archive")?;
                io::stdout().write_all(buffer.get_ref())?;
            } else {
                let file = File::create(out_path).context("failed to create zip archive")?;
                let mut writer = zip::ZipWriter::new(file);
                for (src_path, dest_path) in pairs {
                    let file_content = std::fs::read(src_path)
                        .context(format!("failed to read file: {}", src_path))?;
                    writer.start_file(
                        dest_path.clone(),
                        zip::write::FileOptions::default()
                            .compression_method(zip::CompressionMethod::Stored),
                    )?;
                    writer.write_all(&file_content)?;
                }
                writer.finish().context("failed to finalize zip archive")?;
            }
        }
    }

    Ok(())
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

    #[test]
    fn test_archive_tar_format_detection() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        // Use tempdir for output archive
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.tar");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed: {}",
            result.unwrap_err()
        );

        // Verify archive exists and is readable as tar
        assert!(out_archive.exists(), "tar archive should exist");

        // Use std command to list contents
        let output = std::process::Command::new("tar")
            .args(&["-tf", out_archive.to_str().unwrap()])
            .output()
            .expect("Failed to run tar -tf");

        assert!(
            output.status.success(),
            "tar should be able to read archive"
        );
        let contents = String::from_utf8_lossy(&output.stdout);
        assert!(
            contents.contains("test.txt"),
            "archive should contain test.txt"
        );
    }

    #[test]
    fn test_archive_tar_gz_format_detection() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        // Use tempdir for output archive
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.tar.gz");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed: {}",
            result.unwrap_err()
        );

        // Verify archive exists and is readable as tar.gz
        assert!(out_archive.exists(), "tar.gz archive should exist");

        // Use std command to list contents
        let output = std::process::Command::new("tar")
            .args(&["-tzf", out_archive.to_str().unwrap()])
            .output()
            .expect("Failed to run tar -tzf");

        assert!(
            output.status.success(),
            "tar should be able to read tar.gz archive"
        );
        let contents = String::from_utf8_lossy(&output.stdout);
        assert!(
            contents.contains("test.txt"),
            "archive should contain test.txt"
        );
    }

    #[test]
    fn test_archive_tgz_format_detection() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        // Use tempdir for output archive
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.tgz");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed: {}",
            result.unwrap_err()
        );

        // Verify archive exists and is readable as tgz
        assert!(out_archive.exists(), "tgz archive should exist");

        // Use std command to list contents
        let output = std::process::Command::new("tar")
            .args(&["-tzf", out_archive.to_str().unwrap()])
            .output()
            .expect("Failed to run tar -tzf");

        assert!(
            output.status.success(),
            "tar should be able to read tgz archive"
        );
        let contents = String::from_utf8_lossy(&output.stdout);
        assert!(
            contents.contains("test.txt"),
            "archive should contain test.txt"
        );
    }

    #[test]
    fn test_archive_zip_format_detection() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        // Use tempdir for output archive
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.zip");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed: {}",
            result.unwrap_err()
        );

        // Verify archive exists and is readable as zip
        assert!(out_archive.exists(), "zip archive should exist");

        // Use std command to list contents
        let output = std::process::Command::new("unzip")
            .args(&["-l", out_archive.to_str().unwrap()])
            .output()
            .expect("Failed to run unzip -l");

        assert!(
            output.status.success(),
            "unzip should be able to read archive"
        );
        let contents = String::from_utf8_lossy(&output.stdout);
        assert!(
            contents.contains("test.txt"),
            "archive should contain test.txt"
        );
    }

    #[test]
    fn test_archive_format_override() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        // Use tempdir for output archive
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.custom");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            Some(ArchiveFormat::Tar),
            &["test".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed: {}",
            result.unwrap_err()
        );

        // Verify archive exists and is readable as tar
        assert!(out_archive.exists(), "tar archive should exist");

        // Use std command to list contents
        let output = std::process::Command::new("tar")
            .args(&["-tf", out_archive.to_str().unwrap()])
            .output()
            .expect("Failed to run tar -tf");

        assert!(
            output.status.success(),
            "tar should be able to read archive"
        );
    }

    #[test]
    fn test_archive_unknown_extension_error() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        // Use tempdir for output archive with unknown extension
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.unknown");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );

        assert!(result.is_err(), "run() should fail with unknown extension");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("unknown archive format"),
            "Error should mention unknown format: {}",
            err_msg
        );
    }

    #[test]
    fn test_archive_stdout_without_format_error() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            "-",
            OutputType::Archive,
            None,
            &["test".to_string()],
        );

        assert!(
            result.is_err(),
            "run() should fail when using --out - without format"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("unknown archive format"),
            "Error should mention unknown format: {}",
            err_msg
        );
    }

    #[test]
    fn test_archive_stdout_with_format() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        // This test is limited since we can't easily capture stdout in unit tests
        // Just verify that specifying format doesn't cause an error in the detection phase
        let result = run(
            config,
            "-",
            OutputType::Archive,
            Some(ArchiveFormat::Tar),
            &["test".to_string()],
        );

        // The test succeeds if no format detection error occurs
        // (stdout redirection limitations prevent verifying actual tar output)
        assert!(
            result.is_ok() || result.unwrap_err().to_string().contains("stdout"),
            "Should not fail on format detection when format is specified"
        );
    }

    #[test]
    fn test_archive_file_overwrite() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test file
        fs::write(temp_path.join("test.txt"), "content").expect("Failed to write file");

        // Use tempdir for output archive
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.tar");

        // Create initial archive
        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("test.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result1 = run(
            config.clone(),
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );
        assert!(result1.is_ok(), "First run should succeed");

        let metadata1 = fs::metadata(out_archive.as_path()).expect("File should exist");
        let modified1 = metadata1.modified().expect("Should get modified time");

        // Wait a bit and create second archive
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result2 = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );
        assert!(result2.is_ok(), "Second run should succeed");

        // File should exist and be updated
        assert!(out_archive.exists(), "Archive should still exist");
        let metadata2 = fs::metadata(out_archive.as_path()).expect("File should exist");
        let modified2 = metadata2.modified().expect("Should get modified time");

        assert!(
            modified2 > modified1,
            "Archive should be overwritten (updated)"
        );
    }

    #[test]
    fn test_archive_symlink_handling() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create original file
        fs::write(temp_path.join("original.txt"), "original content")
            .expect("Failed to write original file");

        // Create symlink
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            temp_path.join("original.txt"),
            temp_path.join("symlink.txt"),
        )
        .expect("Failed to create symlink");

        #[cfg(windows)]
        std::os::windows::fs::symlink_file(
            temp_path.join("original.txt"),
            temp_path.join("symlink.txt"),
        )
        .expect("Failed to create symlink");

        // Use tempdir for output archive
        let out_dir = TempDir::new().expect("Failed to create output dir");
        let out_archive = out_dir.path().join("out.tar");

        let config = CollectConfig {
            schema_version: 1,
            bundles: {
                let mut bundles = IndexMap::new();
                bundles.insert(
                    "test".to_string(),
                    vec![Entry::Object(EntryObject {
                        from: temp_path.join("symlink.txt").to_str().unwrap().to_string(),
                        to: None,
                        optional: false,
                    })],
                );
                bundles
            },
        };

        let result = run(
            config,
            out_archive.to_str().unwrap(),
            OutputType::Archive,
            None,
            &["test".to_string()],
        );

        assert!(
            result.is_ok(),
            "run() should succeed: {}",
            result.unwrap_err()
        );

        // Verify archive exists and contains symlink content
        assert!(out_archive.exists(), "tar archive should exist");

        // Use std command to list contents
        let output = std::process::Command::new("tar")
            .args(&["-tf", out_archive.to_str().unwrap()])
            .output()
            .expect("Failed to run tar -tf");

        assert!(
            output.status.success(),
            "tar should be able to read archive"
        );
        let contents = String::from_utf8_lossy(&output.stdout);
        assert!(
            contents.contains("symlink.txt"),
            "archive should contain symlink.txt"
        );
    }
}

/// Run the collection engine: copy matched files from config to destination.
pub fn run(
    config: CollectConfig,
    out: &str,
    output_type: OutputType,
    archive_format: Option<ArchiveFormat>,
    bundles: &[String],
) -> anyhow::Result<()> {
    use crate::expand::expand;
    use anyhow::Context;
    use std::fs;
    use std::path::Path;

    // Initialize logging
    crate::logging::init_once();

    // For archive output, prepare file pairs. For directory output, create output directory.
    let mut file_pairs: Vec<(String, String)> = Vec::new();

    if output_type == OutputType::Archive && out != "-" {
        // For archive output to a file, we'll collect pairs and write at the end
    } else if output_type == OutputType::Dir {
        // Create output directory for dir output
        fs::create_dir_all(out).context("failed to create output directory")?;
    }
    // For archive output to stdout, file pairs will be used directly at the end

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

                        if output_type == OutputType::Archive {
                            // For archive output, collect the pair
                            file_pairs.push((matched_file.clone(), to_path.clone()));
                            log::info!("collected {} -> {}", matched_file, to_path);
                        } else {
                            // For directory output, copy the file
                            let dest_full_path = Path::new(out).join(&to_path);

                            // Create parent directories if needed
                            if let Some(parent) = dest_full_path.parent() {
                                fs::create_dir_all(parent)
                                    .context("failed to create parent directory")?;
                            }

                            // Copy the file
                            fs::copy(&matched_file, &dest_full_path)
                                .context("failed to copy file")?;

                            // Log the collection
                            log::info!("collected {} -> {}", matched_file, to_path);
                        }
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

                    if output_type == OutputType::Archive {
                        // For archive output, collect the pair
                        file_pairs.push((from.clone(), to_path.clone()));
                        log::info!("collected {} -> {}", from, to_path);
                    } else {
                        // For directory output, copy the file
                        let dest_full_path = Path::new(out).join(&to_path);

                        // Create parent directories if needed
                        if let Some(parent) = dest_full_path.parent() {
                            fs::create_dir_all(parent)
                                .context("failed to create parent directory")?;
                        }

                        // Copy the file
                        fs::copy(&from, &dest_full_path).context("failed to copy file")?;

                        // Log the collection
                        log::info!("collected {} -> {}", from, to_path);
                    }
                }
            }
        }
    }

    // Handle archive output
    if output_type == OutputType::Archive {
        // Detect format from extension or use provided format
        let format = if let Some(fmt) = archive_format {
            fmt
        } else {
            detect_format_from_extension(out)?
        };

        write_archive(format, out, &file_pairs)?;
    }

    // Report where the output landed. With `--out -` the archive itself is the
    // stdout stream, so echoing the path there would append "-\n" to the archive
    // bytes; `gzip -t` rejects the result as trailing garbage. Name the
    // destination on stderr instead, keeping stdout byte-exact.
    if out == "-" {
        log::info!("wrote archive to stdout");
    } else {
        println!("{}", out);
    }

    Ok(())
}
