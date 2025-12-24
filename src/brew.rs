use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub desc: Option<String>,
    pub homepage: Option<String>,
    pub installed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrewInfoFormula {
    pub name: String,
    pub full_name: Option<String>,
    pub tap: Option<String>,
    pub oldname: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub versioned_formulae: Option<Vec<String>>,
    pub desc: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub versions: BrewVersions,
    pub urls: Option<BrewUrls>,
    pub revision: Option<i32>,
    pub version_scheme: Option<i32>,
    pub bottle: Option<serde_json::Value>,
    pub keg_only: Option<bool>,
    pub keg_only_reason: Option<serde_json::Value>,
    pub options: Option<Vec<serde_json::Value>>,
    pub build_dependencies: Option<Vec<String>>,
    pub dependencies: Option<Vec<String>>,
    pub test_dependencies: Option<Vec<String>>,
    pub recommended_dependencies: Option<Vec<String>>,
    pub optional_dependencies: Option<Vec<String>>,
    pub uses_from_macos: Option<Vec<serde_json::Value>>,
    pub requirements: Option<Vec<serde_json::Value>>,
    pub conflicts_with: Option<Vec<String>>,
    pub conflicts_with_reasons: Option<Vec<String>>,
    pub link_overwrite: Option<Vec<String>>,
    pub caveats: Option<String>,
    pub installed: Option<Vec<BrewInstalled>>,
    pub linked_keg: Option<String>,
    pub pinned: Option<bool>,
    pub outdated: Option<bool>,
    pub deprecated: Option<bool>,
    pub deprecation_date: Option<String>,
    pub deprecation_reason: Option<String>,
    pub disabled: Option<bool>,
    pub disable_date: Option<String>,
    pub disable_reason: Option<String>,
    pub analytics: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrewVersions {
    pub stable: String,
    pub head: Option<String>,
    pub bottle: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrewUrls {
    pub stable: Option<BrewUrl>,
    pub head: Option<BrewUrl>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrewUrl {
    pub url: String,
    pub tag: Option<String>,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrewInstalled {
    pub version: String,
    pub used_options: Vec<String>,
    pub built_as_bottle: bool,
    pub poured_from_bottle: bool,
    pub time: Option<i64>,
    pub runtime_dependencies: Option<Vec<serde_json::Value>>,
    pub installed_as_dependency: bool,
    pub installed_on_request: bool,
}

#[derive(Debug)]
pub enum BrewError {
    CommandFailed(String),
    ParseError(String),
    NotInstalled,
}

impl std::fmt::Display for BrewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrewError::CommandFailed(msg) => write!(f, "Brew command failed: {}", msg),
            BrewError::ParseError(msg) => write!(f, "Failed to parse brew output: {}", msg),
            BrewError::NotInstalled => write!(f, "Homebrew is not installed or not in PATH"),
        }
    }
}

impl std::error::Error for BrewError {}

pub type BrewResult<T> = Result<T, BrewError>;

/// Check if brew is installed and accessible
pub fn is_brew_installed() -> bool {
    Command::new("brew")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Get list of all installed packages (single batch call)
pub async fn get_installed_packages() -> BrewResult<Vec<Package>> {
    let output = tokio::process::Command::new("brew")
        .args(["info", "--json=v2", "--installed"])
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(BrewError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);

    #[derive(Deserialize)]
    struct BrewInfoResponse {
        formulae: Vec<BrewInfoFormula>,
    }

    let response: BrewInfoResponse = serde_json::from_str(&json_str)
        .map_err(|e| BrewError::ParseError(e.to_string()))?;

    let packages = response
        .formulae
        .into_iter()
        .map(|info| Package {
            name: info.name,
            version: Some(info.versions.stable),
            desc: info.desc,
            homepage: info.homepage,
            installed: true,
        })
        .collect();

    Ok(packages)
}

/// Search for packages (returns all if query is empty)
pub async fn search_packages(query: &str) -> BrewResult<Vec<String>> {
    let mut cmd = tokio::process::Command::new("brew");
    cmd.args(["search", "--formula"]);
    
    if !query.is_empty() {
        cmd.arg(query);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(BrewError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let packages = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.starts_with("==>"))
        .collect();

    Ok(packages)
}

/// Get detailed info about a specific package
pub async fn get_package_info(package_name: &str) -> BrewResult<BrewInfoFormula> {
    let output = tokio::process::Command::new("brew")
        .args(["info", "--json=v2", package_name])
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(BrewError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    
    #[derive(Deserialize)]
    struct BrewInfoResponse {
        formulae: Vec<BrewInfoFormula>,
    }
    
    let response: BrewInfoResponse = serde_json::from_str(&json_str)
        .map_err(|e| BrewError::ParseError(e.to_string()))?;

    response.formulae.into_iter().next()
        .ok_or_else(|| BrewError::ParseError("No formula found in response".to_string()))
}

/// Install a package
pub async fn install_package(package_name: &str) -> BrewResult<String> {
    let output = tokio::process::Command::new("brew")
        .args(["install", package_name])
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(BrewError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Uninstall a package
pub async fn uninstall_package(package_name: &str) -> BrewResult<String> {
    let output = tokio::process::Command::new("brew")
        .args(["uninstall", package_name])
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(BrewError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Update brew itself - returns (stdout, stderr) for display
pub async fn update_brew() -> BrewResult<(String, String)> {
    let output = tokio::process::Command::new("brew")
        .arg("update")
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // brew update writes progress to stderr, so we return both
    if !output.status.success() {
        return Err(BrewError::CommandFailed(format!("{}\n{}", stdout, stderr)));
    }

    Ok((stdout, stderr))
}

/// Upgrade all packages or a specific package
pub async fn upgrade_packages(package_name: Option<&str>) -> BrewResult<String> {
    let mut cmd = tokio::process::Command::new("brew");
    cmd.arg("upgrade");
    
    if let Some(name) = package_name {
        cmd.arg(name);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(BrewError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get brew statistics for status overview
pub async fn get_brew_stats() -> BrewResult<BrewStats> {
    let installed = tokio::process::Command::new("brew")
        .args(["list", "--formula", "-1"])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);

    let casks = tokio::process::Command::new("brew")
        .args(["list", "--cask", "-1"])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);

    let outdated = tokio::process::Command::new("brew")
        .args(["outdated", "--formula"])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);

    let formulae = tokio::process::Command::new("brew")
        .args(["formulae"])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);

    let leaves = tokio::process::Command::new("brew")
        .args(["leaves"])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);

    let taps = tokio::process::Command::new("brew")
        .args(["tap"])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0);

    Ok(BrewStats {
        installed,
        casks,
        outdated,
        formulae,
        leaves,
        taps,
    })
}

#[derive(Debug, Clone)]
pub struct BrewStats {
    pub installed: usize,
    pub casks: usize,
    pub outdated: usize,
    pub formulae: usize,
    pub leaves: usize,
    pub taps: usize,
}

/// Get list of outdated packages
pub async fn get_outdated_packages() -> BrewResult<Vec<String>> {
    let output = tokio::process::Command::new("brew")
        .args(["outdated", "--formula"])
        .output()
        .await
        .map_err(|e| BrewError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(BrewError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let packages = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(packages)
}
