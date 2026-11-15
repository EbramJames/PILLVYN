use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
    pub status_message: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

pub async fn check_for_updates(
    current_version: &str,
    repo_slug: &str,
    user_consented: bool,
) -> Result<UpdateCheckResult, String> {
    if !user_consented {
        return Err("Automatic/remote update checks require explicit user consent in Settings -> Privacy & Updates.".into());
    }

    let clean_repo = repo_slug.trim().trim_matches('/');
    if !is_valid_repo_slug(clean_repo) {
        return Err("Invalid GitHub repository slug (expected owner/repo)".into());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("PILLVYN-Updater/1.0")
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let url = format!("https://api.github.com/repos/{clean_repo}/releases/latest");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub Releases (offline?): {e}"))?;

    if resp.status().as_u16() == 404 {
        return Ok(UpdateCheckResult {
            current_version: current_version.to_string(),
            latest_version: None,
            update_available: false,
            release_url: Some(format!("https://github.com/{clean_repo}/releases")),
            release_notes: None,
            status_message: "No published GitHub release found yet for this repository.".into(),
        });
    }

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned status {}", resp.status()));
    }

    let release: GitHubRelease = resp
        .json()
        .await
        .map_err(|e| format!("Invalid GitHub release payload: {e}"))?;
    let latest_clean = release.tag_name.trim_start_matches('v').to_string();
    let update_available = is_newer_semver(&latest_clean, current_version);

    Ok(UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version: Some(latest_clean.clone()),
        update_available,
        release_url: Some(release.html_url),
        release_notes: release.body.map(|b| b.chars().take(600).collect()),
        status_message: if update_available {
            format!("PILLVYN {latest_clean} is available for download.")
        } else {
            format!("You are running the latest version ({current_version}).")
        },
    })
}

fn is_valid_repo_slug(slug: &str) -> bool {
    let parts: Vec<&str> = slug.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        })
}

pub fn is_newer_semver(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u64, u64, u64) {
        let clean = v.split('-').next().unwrap_or(v);
        let mut nums = clean.split('.').map(|s| s.parse::<u64>().unwrap_or(0));
        (
            nums.next().unwrap_or(0),
            nums.next().unwrap_or(0),
            nums.next().unwrap_or(0),
        )
    };
    parse(candidate) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison_works() {
        assert!(is_newer_semver("1.1.0", "1.0.0"));
        assert!(is_newer_semver("1.1.1", "1.1.0"));
        assert!(!is_newer_semver("1.1.0", "1.1.0"));
        assert!(!is_newer_semver("1.0.0", "1.1.0"));
        assert!(is_valid_repo_slug("EbramJames/PILLVYN"));
        assert!(!is_valid_repo_slug("https://evil.com/x"));
    }
}
