use std::path::{Path, PathBuf};

use crate::platform::Target;

/// Manages the local tool cache at `~/.runx/cache/`.
///
/// Cache directory structure:
/// ```text
/// ~/.runx/cache/<tool>/<version>/<platform>-<arch>/
/// ```
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    /// Create a new cache rooted at `~/.runx/cache/`.
    pub fn new() -> Result<Self, CacheError> {
        let home = dirs::home_dir().ok_or(CacheError::NoHomeDir)?;
        let root = home.join(".runx").join("cache");
        Ok(Self { root })
    }

    /// Create a cache rooted at a custom directory (for testing).
    #[cfg(test)]
    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// Return the root cache directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Return the install path for a specific tool version and target.
    ///
    /// Example: `~/.runx/cache/node/18.19.1/macOS-aarch64/`
    pub fn install_path(&self, tool: &str, version: &semver::Version, target: &Target) -> PathBuf {
        self.root
            .join(tool)
            .join(version.to_string())
            .join(target.to_string())
    }

    /// Check if a tool version is already cached.
    pub fn is_cached(&self, tool: &str, version: &semver::Version, target: &Target) -> bool {
        self.install_path(tool, version, target).exists()
    }

    /// Ensure the cache directory for a tool version exists.
    ///
    /// Returns the path to the install directory.
    #[cfg(test)]
    pub fn prepare_install_dir(
        &self,
        tool: &str,
        version: &semver::Version,
        target: &Target,
    ) -> Result<PathBuf, CacheError> {
        let path = self.install_path(tool, version, target);
        std::fs::create_dir_all(&path).map_err(|e| CacheError::Io {
            path: path.clone(),
            source: e,
        })?;
        Ok(path)
    }

    /// Remove all cached versions of a specific tool.
    pub fn clean_tool(&self, tool: &str) -> Result<u64, CacheError> {
        let tool_dir = self.root.join(tool);
        if !tool_dir.exists() {
            return Ok(0);
        }
        let size = dir_size(&tool_dir);
        std::fs::remove_dir_all(&tool_dir).map_err(|e| CacheError::Io {
            path: tool_dir,
            source: e,
        })?;
        Ok(size)
    }

    /// Remove cached versions of a tool matching a version spec.
    ///
    /// Uses `VersionSpec` matching: `Major(21)` removes all `21.x.x`,
    /// `MajorMinor(21, 0)` removes all `21.0.x`, `Exact` removes one version.
    pub fn clean_version(
        &self,
        tool: &str,
        spec: &crate::version::VersionSpec,
    ) -> Result<u64, CacheError> {
        let tool_dir = self.root.join(tool);
        if !tool_dir.exists() {
            return Ok(0);
        }

        let versions = self.list_versions(tool)?;
        let mut freed = 0u64;

        for ver_str in &versions {
            let Ok(ver) = ver_str.parse::<semver::Version>() else {
                continue;
            };
            if spec.matches(&ver) {
                let ver_dir = tool_dir.join(ver_str);
                freed += dir_size(&ver_dir);
                std::fs::remove_dir_all(&ver_dir).map_err(|e| CacheError::Io {
                    path: ver_dir,
                    source: e,
                })?;
            }
        }

        // Clean up empty tool directory
        if self.list_versions(tool)?.is_empty() && tool_dir.exists() {
            let _ = std::fs::remove_dir(&tool_dir);
        }

        Ok(freed)
    }

    /// List cached versions matching a version spec (for dry-run display).
    pub fn matching_versions(
        &self,
        tool: &str,
        spec: &crate::version::VersionSpec,
    ) -> Result<Vec<String>, CacheError> {
        let versions = self.list_versions(tool)?;
        Ok(versions
            .into_iter()
            .filter(|v| {
                v.parse::<semver::Version>()
                    .map(|ver| spec.matches(&ver))
                    .unwrap_or(false)
            })
            .collect())
    }

    /// Remove all cached tools.
    pub fn clean_all(&self) -> Result<u64, CacheError> {
        if !self.root.exists() {
            return Ok(0);
        }
        let size = dir_size(&self.root);
        std::fs::remove_dir_all(&self.root).map_err(|e| CacheError::Io {
            path: self.root.clone(),
            source: e,
        })?;
        Ok(size)
    }

    /// Find cached versions older than `max_age_days` days.
    ///
    /// Returns candidates with their filesystem paths for optional deletion.
    pub fn find_older_than(
        &self,
        max_age_days: u64,
        tool_filter: Option<&str>,
    ) -> Result<CleanCandidates, CacheError> {
        let mut candidates = CleanCandidates::default();
        if !self.root.exists() {
            return Ok(candidates);
        }

        let cutoff = std::time::SystemTime::now()
            - std::time::Duration::from_secs(max_age_days * 24 * 60 * 60);

        let tool_dirs = self.tool_dirs(tool_filter)?;

        for (tool_name, tool_dir) in &tool_dirs {
            let version_entries = std::fs::read_dir(tool_dir).map_err(|e| CacheError::Io {
                path: tool_dir.clone(),
                source: e,
            })?;

            for entry in version_entries {
                let entry = entry.map_err(|e| CacheError::Io {
                    path: tool_dir.clone(),
                    source: e,
                })?;
                if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    continue;
                }

                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                if modified < cutoff {
                    let path = entry.path();
                    let version_name = entry.file_name().to_string_lossy().to_string();
                    let size = dir_size(&path);
                    candidates.total_bytes += size;
                    candidates.entries.push(CleanEntry {
                        label: format!("{tool_name}@{version_name}"),
                        path,
                        size,
                    });
                }
            }
        }

        Ok(candidates)
    }

    /// Remove the given clean candidates from disk.
    pub fn remove_candidates(&self, candidates: &CleanCandidates) -> Result<u64, CacheError> {
        let mut freed = 0;
        for entry in &candidates.entries {
            if entry.path.exists() {
                std::fs::remove_dir_all(&entry.path).map_err(|e| CacheError::Io {
                    path: entry.path.clone(),
                    source: e,
                })?;
                freed += entry.size;
            }
        }

        // Clean up empty tool directories
        if self.root.exists()
            && let Ok(tool_entries) = std::fs::read_dir(&self.root)
        {
            for tool_entry in tool_entries.flatten() {
                if tool_entry
                    .file_type()
                    .map(|ft| ft.is_dir())
                    .unwrap_or(false)
                    && std::fs::read_dir(tool_entry.path())
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(false)
                {
                    let _ = std::fs::remove_dir(tool_entry.path());
                }
            }
        }

        Ok(freed)
    }

    /// Collect tool directories, optionally filtered to a single tool.
    fn tool_dirs(&self, tool_filter: Option<&str>) -> Result<Vec<(String, PathBuf)>, CacheError> {
        if let Some(name) = tool_filter {
            let dir = self.root.join(name);
            if dir.exists() {
                Ok(vec![(name.to_string(), dir)])
            } else {
                Ok(Vec::new())
            }
        } else {
            let mut dirs = Vec::new();
            let entries = std::fs::read_dir(&self.root).map_err(|e| CacheError::Io {
                path: self.root.clone(),
                source: e,
            })?;
            for entry in entries {
                let entry = entry.map_err(|e| CacheError::Io {
                    path: self.root.clone(),
                    source: e,
                })?;
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    dirs.push((name, entry.path()));
                }
            }
            Ok(dirs)
        }
    }

    /// List all cached tools and their versions.
    pub fn list_cached(&self) -> Result<Vec<CachedTool>, CacheError> {
        let mut tools = Vec::new();
        if !self.root.exists() {
            return Ok(tools);
        }

        let entries = std::fs::read_dir(&self.root).map_err(|e| CacheError::Io {
            path: self.root.clone(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| CacheError::Io {
                path: self.root.clone(),
                source: e,
            })?;
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                let versions = self.list_versions(&name)?;
                let size = dir_size(&entry.path());
                tools.push(CachedTool {
                    name,
                    versions,
                    size_bytes: size,
                });
            }
        }
        Ok(tools)
    }

    /// List cached versions for a specific tool.
    fn list_versions(&self, tool: &str) -> Result<Vec<String>, CacheError> {
        let tool_dir = self.root.join(tool);
        if !tool_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        let entries = std::fs::read_dir(&tool_dir).map_err(|e| CacheError::Io {
            path: tool_dir,
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| CacheError::Io {
                path: self.root.clone(),
                source: e,
            })?;
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                versions.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        Ok(versions)
    }
}

/// Information about a cached tool.
#[derive(Debug, Clone)]
pub struct CachedTool {
    pub name: String,
    pub versions: Vec<String>,
    pub size_bytes: u64,
}

/// Candidates identified for cache cleanup.
#[derive(Debug, Clone, Default)]
pub struct CleanCandidates {
    /// Total bytes of all candidates.
    pub total_bytes: u64,
    /// Individual entries to clean.
    pub entries: Vec<CleanEntry>,
}

/// A single cache entry that can be cleaned.
#[derive(Debug, Clone)]
pub struct CleanEntry {
    /// Display label (e.g., "node@18.0.0").
    pub label: String,
    /// Filesystem path to the version directory.
    pub path: PathBuf,
    /// Size in bytes.
    pub size: u64,
}

/// Errors that occur during cache operations.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("cannot determine home directory")]
    NoHomeDir,

    #[error("cache I/O error at `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Recursively compute the total size of a directory in bytes.
///
/// Silently skips entries that can't be read (permissions, broken symlinks).
fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue; // Skip unreadable entries
            };
            if ft.is_dir() {
                total += dir_size(&entry.path());
            } else {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Arch, Platform, Target};

    fn test_target() -> Target {
        Target {
            platform: Platform::MacOS,
            arch: Arch::Aarch64,
        }
    }

    fn test_version() -> semver::Version {
        semver::Version::new(18, 19, 1)
    }

    #[test]
    fn test_cache_install_path() {
        let cache = Cache::with_root(PathBuf::from("/tmp/test-cache"));
        let path = cache.install_path("node", &test_version(), &test_target());
        assert_eq!(
            path,
            PathBuf::from("/tmp/test-cache/node/18.19.1/macOS-aarch64")
        );
    }

    #[test]
    fn test_cache_is_cached_false_when_missing() {
        let cache = Cache::with_root(PathBuf::from("/tmp/nonexistent-cache-dir"));
        assert!(!cache.is_cached("node", &test_version(), &test_target()));
    }

    #[test]
    fn test_cache_prepare_and_detect() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let target = test_target();
        let version = test_version();

        assert!(!cache.is_cached("node", &version, &target));

        cache
            .prepare_install_dir("node", &version, &target)
            .unwrap();
        assert!(cache.is_cached("node", &version, &target));
    }

    #[test]
    fn test_cache_clean_tool() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let target = test_target();
        let version = test_version();

        cache
            .prepare_install_dir("node", &version, &target)
            .unwrap();
        // Write a small file so there's something to measure
        let install = cache.install_path("node", &version, &target);
        std::fs::write(install.join("test.txt"), "hello").unwrap();

        let freed = cache.clean_tool("node").unwrap();
        assert!(freed > 0);
        assert!(!cache.is_cached("node", &version, &target));
    }

    #[test]
    fn test_cache_clean_nonexistent_tool() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        assert_eq!(cache.clean_tool("nonexistent").unwrap(), 0);
    }

    #[test]
    fn test_cache_clean_all() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());

        cache
            .prepare_install_dir("node", &test_version(), &test_target())
            .unwrap();
        cache
            .prepare_install_dir("python", &semver::Version::new(3, 11, 0), &test_target())
            .unwrap();

        let freed = cache.clean_all().unwrap();
        assert_eq!(freed, 0); // Empty dirs have 0 file bytes
        assert!(!cache.root().exists());
    }

    #[test]
    fn test_cache_list_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let tools = cache.list_cached().unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_cache_list_with_tools() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());

        cache
            .prepare_install_dir("node", &test_version(), &test_target())
            .unwrap();
        cache
            .prepare_install_dir("node", &semver::Version::new(20, 0, 0), &test_target())
            .unwrap();

        let tools = cache.list_cached().unwrap();
        assert_eq!(tools.len(), 1); // One tool (node)
        assert_eq!(tools[0].name, "node");
        assert_eq!(tools[0].versions.len(), 2); // Two versions
    }

    #[test]
    fn test_cache_new_succeeds() {
        // Should succeed on any machine with a home directory
        assert!(Cache::new().is_ok());
    }

    #[test]
    fn test_cache_error_display() {
        let err = CacheError::NoHomeDir;
        assert_eq!(err.to_string(), "cannot determine home directory");

        let err = CacheError::Io {
            path: PathBuf::from("/tmp/test"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        assert!(err.to_string().contains("/tmp/test"));
    }

    // --- remove_candidates: happy path ---

    #[test]
    fn test_remove_candidates_frees_space() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let target = test_target();
        let version = test_version();

        cache
            .prepare_install_dir("node", &version, &target)
            .unwrap();
        let install = cache.install_path("node", &version, &target);
        // Write a real file so dir_size returns > 0
        std::fs::write(install.join("node"), b"fake binary data").unwrap();

        // Build a CleanCandidates entry pointing at the version dir
        let version_dir = install.parent().unwrap().to_path_buf(); // node/18.19.1/
        let size = 16u64; // len of "fake binary data"
        let candidates = CleanCandidates {
            total_bytes: size,
            entries: vec![CleanEntry {
                label: "node@18.19.1".to_string(),
                path: version_dir.clone(),
                size,
            }],
        };

        let freed = cache.remove_candidates(&candidates).unwrap();
        assert!(freed > 0, "should have freed some bytes");
        assert!(
            !version_dir.exists(),
            "version dir should have been removed"
        );
    }

    #[test]
    fn test_remove_candidates_empty_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        let candidates = CleanCandidates::default();
        let freed = cache.remove_candidates(&candidates).unwrap();
        assert_eq!(freed, 0);
    }

    #[test]
    fn test_remove_candidates_already_deleted_entry_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf());
        // Entry points to a path that doesn't exist — should not error
        let candidates = CleanCandidates {
            total_bytes: 100,
            entries: vec![CleanEntry {
                label: "node@99.0.0".to_string(),
                path: dir.path().join("nonexistent"),
                size: 100,
            }],
        };
        let result = cache.remove_candidates(&candidates);
        assert!(result.is_ok());
    }

    // --- dir_size ---

    #[test]
    fn test_dir_size_nonexistent_returns_zero() {
        // dir_size silently skips unreadable dirs
        let size = dir_size(std::path::Path::new("/nonexistent/path/abc"));
        assert_eq!(size, 0);
    }

    #[test]
    fn test_dir_size_with_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("b.txt"), b"world!").unwrap();
        let size = dir_size(dir.path());
        assert_eq!(size, 11); // 5 + 6 bytes
    }

    #[test]
    fn test_dir_size_nested() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("data"), b"12345").unwrap();
        let size = dir_size(dir.path());
        assert_eq!(size, 5);
    }
}
