use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersion {
    pub name: String,
    pub current_version: String,
    pub upstream_version: Option<String>,
    pub upstream_url: Option<String>,
    pub status: VersionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionStatus {
    UpToDate,
    UpdateAvailable,
    Unknown,
    Error,
}

pub struct VersionChecker;

impl VersionChecker {
    pub async fn check_package_version(
        package_name: &str,
        current_version: &str,
        repo_url: Option<&str>,
    ) -> Result<PackageVersion, Box<dyn std::error::Error>> {
        let upstream_version = if let Some(repo_url) = repo_url {
            Self::fetch_upstream_version(repo_url).await.ok()
        } else {
            None
        };

        let status = match &upstream_version {
            Some(upstream) if upstream != current_version => VersionStatus::UpdateAvailable,
            Some(_) => VersionStatus::UpToDate,
            None => VersionStatus::Unknown,
        };

        Ok(PackageVersion {
            name: package_name.to_string(),
            current_version: current_version.to_string(),
            upstream_version,
            upstream_url: repo_url.map(|s| s.to_string()),
            status,
        })
    }

    pub async fn fetch_upstream_version(
        repo_url: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if repo_url.contains("github.com") {
            Self::fetch_github_latest_tag(repo_url).await
        } else {
            Err("Unsupported repository type".into())
        }
    }

    pub async fn fetch_github_latest_tag(
        repo_url: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let repo_path = repo_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .strip_suffix(".git")
            .unwrap_or(repo_url);

        let parts: Vec<&str> = repo_path.split('/').collect();
        if parts.len() < 3 {
            return Err("Invalid GitHub URL".into());
        }

        let owner = parts[1];
        let repo = parts[2];

        let api_url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            owner, repo
        );

        let client = reqwest::Client::new();
        let response = client
            .get(&api_url)
            .header("User-Agent", "pax-builder")
            .send()
            .await?;

        if response.status().is_success() {
            let release: serde_json::Value = response.json().await?;
            if let Some(tag_name) = release.get("tag_name").and_then(|v| v.as_str()) {
                let version = tag_name.trim_start_matches('v');
                return Ok(version.to_string());
            }
        }

        Err("No release found".into())
    }

    async fn fetch_github_releases(
        repo_url: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let repo_path = repo_url
            .strip_prefix("https://github.com/")
            .unwrap_or(repo_url)
            .strip_suffix(".git")
            .unwrap_or(repo_url);

        let parts: Vec<&str> = repo_path.split('/').collect();
        if parts.len() < 2 {
            return Err("Invalid GitHub URL".into());
        }

        let owner = parts[0];
        let repo = parts[1];

        let api_url = format!("https://api.github.com/repos/{}/{}/tags", owner, repo);

        let client = reqwest::Client::new();
        let response = client
            .get(&api_url)
            .header("User-Agent", "pax-builder")
            .send()
            .await?;

        if response.status().is_success() {
            let tags: Vec<serde_json::Value> = response.json().await?;
            let versions: Vec<String> = tags
                .iter()
                .filter_map(|tag| {
                    tag.get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim_start_matches('v').to_string())
                })
                .collect();
            Ok(versions)
        } else {
            Ok(vec![])
        }
    }

    pub async fn check_all_packages(
        recipes_dir: &std::path::Path,
    ) -> Result<Vec<PackageVersion>, Box<dyn std::error::Error>> {
        let oreon11_dir = recipes_dir.join("oreon-11");
        if !oreon11_dir.exists() {
            return Ok(vec![]);
        }

        let mut versions = Vec::new();
        let entries = std::fs::read_dir(&oreon11_dir)?;

        for entry in entries {
            let entry = entry?;
            let package_dir = entry.path();
            if package_dir.is_dir() {
                let package_name = package_dir
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                let yaml_path = package_dir.join("pax.yaml");
                if yaml_path.exists() {
                    if let Ok(spec) = Self::load_spec(&yaml_path).await {
                        let repo_url = spec
                            .get("repository")
                            .or_else(|| spec.get("homepage"))
                            .and_then(|v| v.as_str());

                        let current_version = spec
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");

                        if let Ok(version_info) =
                            Self::check_package_version(&package_name, current_version, repo_url)
                                .await
                        {
                            versions.push(version_info);
                        }
                    }
                }
            }
        }

        Ok(versions)
    }

    async fn load_spec(
        path: &std::path::Path,
    ) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error>> {
        use std::fs::read_to_string;
        let content = read_to_string(path)?;
        let spec: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).or_else(|_| serde_yaml::from_str(&content))?;
        Ok(spec)
    }
}
