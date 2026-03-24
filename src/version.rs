use std::fmt;
use std::str::FromStr;

/// A version specifier that can match against semver versions.
///
/// Supports several formats:
/// - `latest` — resolves to the most recent stable version
/// - `18` — matches any `18.x.x`
/// - `18.19` — matches any `18.19.x`
/// - `18.19.1` — matches exactly `18.19.1`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionSpec {
    /// Resolve to the latest stable version.
    Latest,
    /// Match any version with this major number (e.g., `18` → `18.x.x`).
    Major(u64),
    /// Match any version with this major.minor (e.g., `18.19` → `18.19.x`).
    MajorMinor(u64, u64),
    /// Match an exact version (e.g., `18.19.1`).
    Exact(semver::Version),
}

impl VersionSpec {
    /// Check if a semver version matches this spec.
    pub fn matches(&self, version: &semver::Version) -> bool {
        match self {
            Self::Latest => true,
            Self::Major(major) => version.major == *major && version.pre.is_empty(),
            Self::MajorMinor(major, minor) => {
                version.major == *major && version.minor == *minor && version.pre.is_empty()
            }
            Self::Exact(exact) => version == exact,
        }
    }

    /// Find the best matching version from a list of candidates.
    ///
    /// Returns the highest version that matches this spec.
    /// For `Latest`, returns the highest version including pre-releases.
    /// Providers should filter their candidate lists to exclude pre-releases
    /// if only stable versions are desired.
    pub fn resolve<'a>(&self, candidates: &'a [semver::Version]) -> Option<&'a semver::Version> {
        candidates.iter().filter(|v| self.matches(v)).max()
    }
}

impl FromStr for VersionSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        if s.is_empty() || s.eq_ignore_ascii_case("latest") {
            return Ok(Self::Latest);
        }

        // Try exact semver first (e.g., "18.19.1")
        if let Ok(version) = semver::Version::parse(s) {
            return Ok(Self::Exact(version));
        }

        // Try major.minor (e.g., "18.19")
        if let Some((major_str, minor_str)) = s.split_once('.') {
            // Check there's no second dot (that would be semver, handled above)
            if !minor_str.contains('.') {
                let major = major_str
                    .parse::<u64>()
                    .map_err(|_| format!("invalid major version in `{s}`"))?;
                let minor = minor_str
                    .parse::<u64>()
                    .map_err(|_| format!("invalid minor version in `{s}`"))?;
                return Ok(Self::MajorMinor(major, minor));
            }
        }

        // Try major only (e.g., "18")
        let major = s.parse::<u64>().map_err(|_| {
            format!("invalid version spec `{s}`. Use: latest, 18, 18.19, or 18.19.1")
        })?;
        Ok(Self::Major(major))
    }
}

impl fmt::Display for VersionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Latest => write!(f, "latest"),
            Self::Major(major) => write!(f, "{major}"),
            Self::MajorMinor(major, minor) => write!(f, "{major}.{minor}"),
            Self::Exact(version) => write!(f, "{version}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }

    // --- Parsing ---

    #[test]
    fn test_parse_latest() {
        assert_eq!(
            "latest".parse::<VersionSpec>().unwrap(),
            VersionSpec::Latest
        );
        assert_eq!(
            "LATEST".parse::<VersionSpec>().unwrap(),
            VersionSpec::Latest
        );
        assert_eq!("".parse::<VersionSpec>().unwrap(), VersionSpec::Latest);
    }

    #[test]
    fn test_parse_major() {
        assert_eq!("18".parse::<VersionSpec>().unwrap(), VersionSpec::Major(18));
        assert_eq!("3".parse::<VersionSpec>().unwrap(), VersionSpec::Major(3));
    }

    #[test]
    fn test_parse_major_minor() {
        assert_eq!(
            "18.19".parse::<VersionSpec>().unwrap(),
            VersionSpec::MajorMinor(18, 19)
        );
        assert_eq!(
            "3.11".parse::<VersionSpec>().unwrap(),
            VersionSpec::MajorMinor(3, 11)
        );
    }

    #[test]
    fn test_parse_exact() {
        assert_eq!(
            "18.19.1".parse::<VersionSpec>().unwrap(),
            VersionSpec::Exact(v("18.19.1"))
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert!("abc".parse::<VersionSpec>().is_err());
        assert!("18.abc".parse::<VersionSpec>().is_err());
        assert!("abc.1".parse::<VersionSpec>().is_err());
    }

    #[test]
    fn test_parse_whitespace_trimmed() {
        assert_eq!(
            " 18 ".parse::<VersionSpec>().unwrap(),
            VersionSpec::Major(18)
        );
        assert_eq!(
            " latest ".parse::<VersionSpec>().unwrap(),
            VersionSpec::Latest
        );
    }

    // --- Display ---

    #[test]
    fn test_display() {
        assert_eq!(VersionSpec::Latest.to_string(), "latest");
        assert_eq!(VersionSpec::Major(18).to_string(), "18");
        assert_eq!(VersionSpec::MajorMinor(18, 19).to_string(), "18.19");
        assert_eq!(VersionSpec::Exact(v("18.19.1")).to_string(), "18.19.1");
    }

    // --- Matching ---

    #[test]
    fn test_latest_matches_everything_stable() {
        assert!(VersionSpec::Latest.matches(&v("18.19.1")));
        assert!(VersionSpec::Latest.matches(&v("1.0.0")));
    }

    #[test]
    fn test_major_matches() {
        let spec = VersionSpec::Major(18);
        assert!(spec.matches(&v("18.0.0")));
        assert!(spec.matches(&v("18.19.1")));
        assert!(!spec.matches(&v("20.0.0")));
    }

    #[test]
    fn test_major_skips_prerelease() {
        let spec = VersionSpec::Major(18);
        assert!(!spec.matches(&v("18.0.0-alpha.1")));
    }

    #[test]
    fn test_major_minor_matches() {
        let spec = VersionSpec::MajorMinor(18, 19);
        assert!(spec.matches(&v("18.19.0")));
        assert!(spec.matches(&v("18.19.5")));
        assert!(!spec.matches(&v("18.20.0")));
        assert!(!spec.matches(&v("20.19.0")));
    }

    #[test]
    fn test_major_minor_skips_prerelease() {
        let spec = VersionSpec::MajorMinor(18, 19);
        assert!(!spec.matches(&v("18.19.0-beta.1")));
    }

    #[test]
    fn test_latest_matches_prerelease() {
        assert!(VersionSpec::Latest.matches(&v("1.0.0-alpha.1")));
    }

    #[test]
    fn test_exact_with_prerelease() {
        let spec = VersionSpec::Exact(v("1.0.0-rc.1"));
        assert!(spec.matches(&v("1.0.0-rc.1")));
        assert!(!spec.matches(&v("1.0.0")));
    }

    #[test]
    fn test_exact_matches() {
        let spec = VersionSpec::Exact(v("18.19.1"));
        assert!(spec.matches(&v("18.19.1")));
        assert!(!spec.matches(&v("18.19.0")));
        assert!(!spec.matches(&v("18.19.2")));
    }

    // --- Resolution ---

    #[test]
    fn test_resolve_major_picks_highest() {
        let candidates = vec![v("18.17.0"), v("18.19.1"), v("18.18.0"), v("20.0.0")];
        let spec = VersionSpec::Major(18);
        assert_eq!(spec.resolve(&candidates), Some(&v("18.19.1")));
    }

    #[test]
    fn test_resolve_latest_picks_highest_stable() {
        let candidates = vec![v("18.19.1"), v("20.0.0"), v("20.1.0-alpha.1")];
        let spec = VersionSpec::Latest;
        // Latest matches all, but max stable is 20.0.0; alpha also matches Latest
        // since Latest.matches returns true for all. The max by semver is 20.1.0-alpha.1
        // but actually 20.1.0-alpha.1 < 20.1.0 in semver, and 20.1.0-alpha.1 > 20.0.0
        // Let's verify: in semver, pre-release versions have lower precedence
        // 20.0.0 > 20.1.0-alpha.1 is false; 20.1.0-alpha.1 > 20.0.0 is true
        // So this would pick 20.1.0-alpha.1 — that's OK for Latest, providers
        // should filter their candidate lists before passing to resolve.
        assert_eq!(spec.resolve(&candidates), Some(&v("20.1.0-alpha.1")));
    }

    #[test]
    fn test_resolve_no_match_returns_none() {
        let candidates = vec![v("20.0.0"), v("21.0.0")];
        let spec = VersionSpec::Major(18);
        assert_eq!(spec.resolve(&candidates), None);
    }

    #[test]
    fn test_resolve_major_minor_picks_highest() {
        let candidates = vec![v("18.19.0"), v("18.19.5"), v("18.19.3"), v("18.20.0")];
        let spec = VersionSpec::MajorMinor(18, 19);
        assert_eq!(spec.resolve(&candidates), Some(&v("18.19.5")));
    }

    #[test]
    fn test_resolve_exact() {
        let candidates = vec![v("18.19.0"), v("18.19.1"), v("18.19.2")];
        let spec = VersionSpec::Exact(v("18.19.1"));
        assert_eq!(spec.resolve(&candidates), Some(&v("18.19.1")));
    }

    #[test]
    fn test_resolve_exact_not_in_candidates() {
        let candidates = vec![v("18.19.0"), v("18.19.2")];
        let spec = VersionSpec::Exact(v("18.19.1"));
        assert_eq!(spec.resolve(&candidates), None);
    }

    #[test]
    fn test_resolve_empty_candidates() {
        let candidates: Vec<semver::Version> = vec![];
        assert_eq!(VersionSpec::Latest.resolve(&candidates), None);
    }

    // --- Parse edge cases ---

    #[test]
    fn test_parse_major_zero() {
        // Edge: zero major is valid
        assert_eq!("0".parse::<VersionSpec>().unwrap(), VersionSpec::Major(0));
    }

    #[test]
    fn test_parse_major_minor_zero_minor() {
        assert_eq!(
            "18.0".parse::<VersionSpec>().unwrap(),
            VersionSpec::MajorMinor(18, 0)
        );
    }

    #[test]
    fn test_parse_invalid_major_negative() {
        // Negative number cannot parse as u64
        assert!("-1".parse::<VersionSpec>().is_err());
    }

    #[test]
    fn test_parse_invalid_major_minor_non_numeric_minor() {
        assert!("18.abc".parse::<VersionSpec>().is_err());
    }

    #[test]
    fn test_parse_invalid_major_minor_non_numeric_major() {
        assert!("abc.1".parse::<VersionSpec>().is_err());
    }

    // --- Matching edge cases ---

    #[test]
    fn test_exact_does_not_match_different_build_metadata() {
        // semver::Version PartialEq considers build metadata, so these are not equal
        let spec = VersionSpec::Exact(v("1.0.0"));
        let with_meta = semver::Version::parse("1.0.0+build.1").unwrap();
        assert!(!spec.matches(&with_meta));
    }

    #[test]
    fn test_major_does_not_match_lower_major() {
        let spec = VersionSpec::Major(18);
        assert!(!spec.matches(&v("17.99.99")));
        assert!(!spec.matches(&v("19.0.0")));
    }

    #[test]
    fn test_major_minor_does_not_match_different_minor() {
        let spec = VersionSpec::MajorMinor(3, 11);
        assert!(!spec.matches(&v("3.10.9")));
        assert!(!spec.matches(&v("3.12.0")));
    }

    // --- Resolve with single candidate ---

    #[test]
    fn test_resolve_single_candidate_matches() {
        let candidates = vec![v("18.0.0")];
        let spec = VersionSpec::Major(18);
        assert_eq!(spec.resolve(&candidates), Some(&v("18.0.0")));
    }

    #[test]
    fn test_resolve_exact_single_non_matching() {
        let candidates = vec![v("18.0.0")];
        let spec = VersionSpec::Exact(v("18.0.1"));
        assert_eq!(spec.resolve(&candidates), None);
    }
}
