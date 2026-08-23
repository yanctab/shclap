//! Container detection module.
//!
//! Detects whether the current process is running inside a container by checking
//! four independent signals in priority order.

use std::path::Path;

/// Signals that indicate the process is running inside a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerSignal {
    /// The `SHCLAP_IN_CONTAINER` environment variable is set (highest priority).
    ShclapInContainer,
    /// The `/.dockerenv` marker file exists.
    DockerEnv,
    /// The `/run/.containerenv` marker file exists.
    ContainerEnv,
    /// The `$container` environment variable is set (lowest priority).
    OciContainerEnv,
}

impl ContainerSignal {
    /// Returns the user-visible string for this signal.
    pub fn signal_name(&self) -> &'static str {
        match self {
            ContainerSignal::ShclapInContainer => "SHCLAP_IN_CONTAINER",
            ContainerSignal::DockerEnv => "/.dockerenv",
            ContainerSignal::ContainerEnv => "/run/.containerenv",
            ContainerSignal::OciContainerEnv => "$container",
        }
    }
}

/// Detects whether the process is running inside a container.
///
/// Checks four signals in priority order and returns the first match.
/// Uses `/` as the root for marker files and `std::env::var` for environment variables.
pub fn detect_container() -> Option<ContainerSignal> {
    detect_container_with(Path::new("/"), |name| std::env::var(name).ok())
}

/// Test seam for container detection.
///
/// Allows injecting a custom marker root and environment variable lookup function
/// for testing purposes.
pub fn detect_container_with(
    marker_root: &Path,
    env_lookup: impl Fn(&str) -> Option<String>,
) -> Option<ContainerSignal> {
    // Check 1: SHCLAP_IN_CONTAINER environment variable
    if env_lookup("SHCLAP_IN_CONTAINER").is_some() {
        return Some(ContainerSignal::ShclapInContainer);
    }

    // Check 2: /.dockerenv marker file
    if marker_root.join(".dockerenv").exists() {
        return Some(ContainerSignal::DockerEnv);
    }

    // Check 3: /run/.containerenv marker file
    if marker_root.join("run/.containerenv").exists() {
        return Some(ContainerSignal::ContainerEnv);
    }

    // Check 4: $container environment variable
    if env_lookup("container").is_some() {
        return Some(ContainerSignal::OciContainerEnv);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test environment that returns no signals.
    fn no_env_lookup(_name: &str) -> Option<String> {
        None
    }

    #[test]
    fn test_no_signal_present() {
        let result = detect_container_with(Path::new("/nonexistent"), no_env_lookup);
        assert_eq!(result, None);
    }

    #[test]
    fn test_shclap_in_container_detected() {
        let env_lookup = |name: &str| {
            if name == "SHCLAP_IN_CONTAINER" {
                Some("1".to_string())
            } else {
                None
            }
        };
        let result = detect_container_with(Path::new("/nonexistent"), env_lookup);
        assert_eq!(result, Some(ContainerSignal::ShclapInContainer));
    }

    #[test]
    fn test_docker_env_detected() {
        let base_tmpdir = std::env::temp_dir();
        let test_dir = base_tmpdir.join(format!("shclap_test_docker_{}", std::process::id()));
        std::fs::create_dir_all(&test_dir).ok();

        let dockerenv_path = test_dir.join(".dockerenv");
        std::fs::write(&dockerenv_path, "").ok();

        let result = detect_container_with(&test_dir, no_env_lookup);

        std::fs::remove_file(&dockerenv_path).ok();
        std::fs::remove_dir(&test_dir).ok();

        assert_eq!(result, Some(ContainerSignal::DockerEnv));
    }

    #[test]
    fn test_container_env_detected() {
        let base_tmpdir = std::env::temp_dir();
        let test_dir = base_tmpdir.join(format!("shclap_test_container_{}", std::process::id()));
        std::fs::create_dir_all(&test_dir).ok();

        let run_dir = test_dir.join("run");
        std::fs::create_dir_all(&run_dir).ok();
        let containerenv_path = run_dir.join(".containerenv");
        std::fs::write(&containerenv_path, "").ok();

        let result = detect_container_with(&test_dir, no_env_lookup);

        std::fs::remove_file(&containerenv_path).ok();
        std::fs::remove_dir(&run_dir).ok();
        std::fs::remove_dir(&test_dir).ok();

        assert_eq!(result, Some(ContainerSignal::ContainerEnv));
    }

    #[test]
    fn test_oci_container_env_detected() {
        let env_lookup = |name: &str| {
            if name == "container" {
                Some("podman".to_string())
            } else {
                None
            }
        };
        let result = detect_container_with(Path::new("/nonexistent"), env_lookup);
        assert_eq!(result, Some(ContainerSignal::OciContainerEnv));
    }

    #[test]
    fn test_priority_shclap_in_container_wins() {
        let env_lookup = |name: &str| match name {
            "SHCLAP_IN_CONTAINER" => Some("1".to_string()),
            "container" => Some("podman".to_string()),
            _ => None,
        };
        let result = detect_container_with(Path::new("/nonexistent"), env_lookup);
        assert_eq!(result, Some(ContainerSignal::ShclapInContainer));
    }

    #[test]
    fn test_priority_docker_env_wins_over_lower() {
        let base_tmpdir = std::env::temp_dir();
        let test_dir = base_tmpdir.join(format!("shclap_test_priority_{}", std::process::id()));
        std::fs::create_dir_all(&test_dir).ok();

        let env_lookup = |name: &str| {
            if name == "container" {
                Some("podman".to_string())
            } else {
                None
            }
        };
        // Create a .dockerenv file in test_dir for testing
        let dockerenv_path = test_dir.join(".dockerenv");
        std::fs::write(&dockerenv_path, "").ok();

        let result = detect_container_with(&test_dir, env_lookup);

        // Clean up
        std::fs::remove_file(&dockerenv_path).ok();
        std::fs::remove_dir(&test_dir).ok();

        assert_eq!(result, Some(ContainerSignal::DockerEnv));
    }

    #[test]
    fn test_signal_name_shclap_in_container() {
        assert_eq!(
            ContainerSignal::ShclapInContainer.signal_name(),
            "SHCLAP_IN_CONTAINER"
        );
    }

    #[test]
    fn test_signal_name_docker_env() {
        assert_eq!(ContainerSignal::DockerEnv.signal_name(), "/.dockerenv");
    }

    #[test]
    fn test_signal_name_container_env() {
        assert_eq!(
            ContainerSignal::ContainerEnv.signal_name(),
            "/run/.containerenv"
        );
    }

    #[test]
    fn test_signal_name_oci_container_env() {
        assert_eq!(ContainerSignal::OciContainerEnv.signal_name(), "$container");
    }
}
