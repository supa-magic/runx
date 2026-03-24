use std::fmt;

/// Operating system platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
}

impl Platform {
    /// Detect the current platform at runtime.
    pub fn detect() -> Result<Self, String> {
        Self::from_os_str(std::env::consts::OS)
    }

    /// Parse a platform from an OS string (e.g., "macos", "linux", "windows").
    pub fn from_os_str(os: &str) -> Result<Self, String> {
        match os {
            "macos" => Ok(Self::MacOS),
            "linux" => Ok(Self::Linux),
            "windows" => Ok(Self::Windows),
            other => Err(format!("unsupported platform: {other}")),
        }
    }

    /// Return the platform string commonly used in download URLs.
    pub fn as_download_str(&self) -> &'static str {
        match self {
            Self::MacOS => "darwin",
            Self::Linux => "linux",
            Self::Windows => "win",
        }
    }

    /// Minimal system PATH for this platform.
    pub fn system_path(&self) -> &'static str {
        match self {
            Self::MacOS | Self::Linux => "/usr/bin:/bin",
            Self::Windows => r"C:\Windows\System32;C:\Windows",
        }
    }

    /// PATH separator for this platform.
    pub fn path_separator(&self) -> char {
        match self {
            Self::MacOS | Self::Linux => ':',
            Self::Windows => ';',
        }
    }

    /// Executable suffix for this platform.
    pub fn exe_suffix(&self) -> &'static str {
        match self {
            Self::MacOS | Self::Linux => "",
            Self::Windows => ".exe",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MacOS => write!(f, "macOS"),
            Self::Linux => write!(f, "Linux"),
            Self::Windows => write!(f, "Windows"),
        }
    }
}

/// CPU architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    X86_64,
    Aarch64,
}

impl Arch {
    /// Detect the current architecture at runtime.
    pub fn detect() -> Result<Self, String> {
        Self::from_arch_str(std::env::consts::ARCH)
    }

    /// Parse an architecture from an arch string (e.g., "x86_64", "aarch64").
    pub fn from_arch_str(arch: &str) -> Result<Self, String> {
        match arch {
            "x86_64" => Ok(Self::X86_64),
            "aarch64" => Ok(Self::Aarch64),
            other => Err(format!("unsupported architecture: {other}")),
        }
    }

    /// Return the architecture string commonly used in download URLs.
    pub fn as_download_str(&self) -> &'static str {
        match self {
            Self::X86_64 => "x64",
            Self::Aarch64 => "arm64",
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86_64 => write!(f, "x86_64"),
            Self::Aarch64 => write!(f, "aarch64"),
        }
    }
}

/// A resolved platform target combining OS and architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Target {
    pub platform: Platform,
    pub arch: Arch,
}

impl Target {
    /// Detect the current platform target at runtime.
    pub fn detect() -> Result<Self, String> {
        Ok(Self {
            platform: Platform::detect()?,
            arch: Arch::detect()?,
        })
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.platform, self.arch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detect_succeeds() {
        // Should succeed on any supported CI/dev machine
        assert!(Platform::detect().is_ok());
    }

    #[test]
    fn test_arch_detect_succeeds() {
        assert!(Arch::detect().is_ok());
    }

    #[test]
    fn test_target_detect_succeeds() {
        let target = Target::detect().unwrap();
        // Verify both fields are populated
        assert!(!target.to_string().is_empty());
    }

    #[test]
    fn test_platform_download_str() {
        assert_eq!(Platform::MacOS.as_download_str(), "darwin");
        assert_eq!(Platform::Linux.as_download_str(), "linux");
        assert_eq!(Platform::Windows.as_download_str(), "win");
    }

    #[test]
    fn test_arch_download_str() {
        assert_eq!(Arch::X86_64.as_download_str(), "x64");
        assert_eq!(Arch::Aarch64.as_download_str(), "arm64");
    }

    #[test]
    fn test_platform_system_path() {
        assert_eq!(Platform::MacOS.system_path(), "/usr/bin:/bin");
        assert_eq!(Platform::Linux.system_path(), "/usr/bin:/bin");
        assert_eq!(
            Platform::Windows.system_path(),
            r"C:\Windows\System32;C:\Windows"
        );
    }

    #[test]
    fn test_platform_path_separator() {
        assert_eq!(Platform::MacOS.path_separator(), ':');
        assert_eq!(Platform::Linux.path_separator(), ':');
        assert_eq!(Platform::Windows.path_separator(), ';');
    }

    #[test]
    fn test_platform_exe_suffix() {
        assert_eq!(Platform::MacOS.exe_suffix(), "");
        assert_eq!(Platform::Linux.exe_suffix(), "");
        assert_eq!(Platform::Windows.exe_suffix(), ".exe");
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(Platform::MacOS.to_string(), "macOS");
        assert_eq!(Platform::Linux.to_string(), "Linux");
        assert_eq!(Platform::Windows.to_string(), "Windows");
    }

    #[test]
    fn test_arch_display() {
        assert_eq!(Arch::X86_64.to_string(), "x86_64");
        assert_eq!(Arch::Aarch64.to_string(), "aarch64");
    }

    #[test]
    fn test_platform_from_os_str_unsupported() {
        assert!(Platform::from_os_str("freebsd").is_err());
        assert!(Platform::from_os_str("").is_err());
    }

    #[test]
    fn test_arch_from_arch_str_unsupported() {
        assert!(Arch::from_arch_str("mips").is_err());
        assert!(Arch::from_arch_str("riscv64").is_err());
    }

    #[test]
    fn test_platform_from_os_str_valid() {
        assert_eq!(Platform::from_os_str("macos").unwrap(), Platform::MacOS);
        assert_eq!(Platform::from_os_str("linux").unwrap(), Platform::Linux);
        assert_eq!(Platform::from_os_str("windows").unwrap(), Platform::Windows);
    }

    #[test]
    fn test_arch_from_arch_str_valid() {
        assert_eq!(Arch::from_arch_str("x86_64").unwrap(), Arch::X86_64);
        assert_eq!(Arch::from_arch_str("aarch64").unwrap(), Arch::Aarch64);
    }

    #[test]
    fn test_target_display() {
        let target = Target {
            platform: Platform::MacOS,
            arch: Arch::Aarch64,
        };
        assert_eq!(target.to_string(), "macOS-aarch64");
    }
}
