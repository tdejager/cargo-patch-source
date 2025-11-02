use crate::error::{PatchError, Result};
use cargo_metadata::MetadataCommand;
use regex::Regex;
use std::path::Path;

/// Information about a crate that can be patched
#[derive(Debug, Clone)]
pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub manifest_path: std::path::PathBuf,
}

/// Query metadata for a workspace at the given path
pub fn query_workspace_crates(workspace_path: &Path) -> Result<Vec<CrateInfo>> {
    let manifest_path = workspace_path.join("Cargo.toml");

    if !manifest_path.exists() {
        return Err(PatchError::SourceWorkspaceNotFound {
            path: manifest_path,
        });
    }

    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()
        .map_err(|e| PatchError::CargoMetadataError { source: e })?;

    let workspace_members: Vec<_> = metadata
        .workspace_packages()
        .into_iter()
        .map(|pkg| CrateInfo {
            name: pkg.name.clone(),
            version: pkg.version.to_string(),
            manifest_path: pkg.manifest_path.clone().into_std_path_buf(),
        })
        .collect();

    if workspace_members.is_empty() {
        return Err(PatchError::NotAWorkspace {
            path: workspace_path.to_path_buf(),
        });
    }

    Ok(workspace_members)
}

/// Filter crates by pattern (supports wildcards)
pub fn filter_crates_by_pattern(
    crates: Vec<CrateInfo>,
    pattern: Option<&str>,
) -> Result<Vec<CrateInfo>> {
    let Some(pattern) = pattern else {
        return Ok(crates);
    };

    let re = glob_pattern_regex(pattern)?;

    let filtered: Vec<_> = crates
        .into_iter()
        .filter(|c| re.is_match(&c.name))
        .collect();

    if filtered.is_empty() {
        return Err(PatchError::NoMatchingCrates {
            pattern: pattern.to_string(),
        });
    }

    Ok(filtered)
}

/// Compile a glob-like pattern into a Regex instance.
pub fn glob_pattern_regex(pattern: &str) -> Result<Regex> {
    let mut escaped = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => escaped.push_str(".*"),
            '?' => escaped.push('.'),
            _ => escaped.push_str(&regex::escape(&ch.to_string())),
        }
    }
    escaped.push('$');

    Regex::new(&escaped).map_err(|e| PatchError::InvalidPattern {
        pattern: pattern.to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_pattern_regex_handles_special_chars() {
        let re = glob_pattern_regex("crate+name?(test)*").unwrap();
        assert!(re.is_match("crate+name1(test)foo"));
        assert!(!re.is_match("crate-name1(test)foo"));
    }

    #[test]
    fn glob_pattern_regex_star_matches_slashes() {
        let re = glob_pattern_regex("foo*bar").unwrap();
        assert!(re.is_match("foobar"));
        assert!(re.is_match("foo123bar"));
        assert!(!re.is_match("foo123baz"));
    }
}
