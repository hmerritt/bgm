pub const APP_NAME: &str = "aura";
#[allow(dead_code)]
pub const BINARY_FILENAME: &str = "aura";

const GIT_COMMIT: &str = match option_env!("AURA_GIT_COMMIT") {
    Some(value) => value,
    None => "",
};
const GIT_BRANCH: &str = match option_env!("AURA_GIT_BRANCH") {
    Some(value) => value,
    None => "",
};
const BUILD_DATE: &str = match option_env!("AURA_BUILD_DATE") {
    Some(value) => value,
    None => "",
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const VERSION_PRERELEASE: &str = match option_env!("AURA_VERSION_PRERELEASE") {
    Some(value) => value,
    None => "",
};
const VERSION_METADATA: &str = match option_env!("AURA_VERSION_METADATA") {
    Some(value) => value,
    None => "",
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub revision: String,
    pub branch: String,
    pub build_date: String,
    pub version: String,
    pub version_prerelease: String,
    pub version_metadata: String,
}

pub fn get_version() -> VersionInfo {
    VersionInfo {
        revision: GIT_COMMIT.to_string(),
        branch: GIT_BRANCH.to_string(),
        build_date: BUILD_DATE.to_string(),
        version: VERSION.to_string(),
        version_prerelease: VERSION_PRERELEASE.to_string(),
        version_metadata: VERSION_METADATA.to_string(),
    }
}

impl VersionInfo {
    #[allow(dead_code)]
    pub fn version_number(&self) -> String {
        if self.version == "unknown" && self.version_prerelease == "unknown" {
            return "Version unknown".to_string();
        }

        let mut version = self.version.clone();
        if !self.version_prerelease.is_empty() {
            version.push('-');
            version.push_str(&self.version_prerelease);
        }
        if !self.version_metadata.is_empty() {
            version.push('+');
            version.push_str(&self.version_metadata);
        }
        version
    }

    pub fn full_version_number(&self, rev: bool) -> String {
        if self.version == "unknown" && self.version_prerelease == "unknown" {
            return format!("{APP_NAME} [Version unknown]");
        }

        let mut version_string = format!("{APP_NAME} [Version {}", self.version);

        if rev && !self.revision.is_empty() {
            if !self.branch.is_empty() && self.branch != "master" && self.branch != "HEAD" {
                version_string.push('/');
                version_string.push_str(&self.branch);
            }
            version_string.push_str(" (");
            version_string.push_str(&self.revision);
            version_string.push(')');
        }

        version_string.push(']');
        version_string
    }
}

#[cfg(test)]
mod tests {
    use super::VersionInfo;

    fn version_info(branch: &str, prerelease: &str, revision: &str, metadata: &str) -> VersionInfo {
        VersionInfo {
            revision: revision.to_string(),
            branch: branch.to_string(),
            build_date: "1700000000".to_string(),
            version: "1.2.3".to_string(),
            version_prerelease: prerelease.to_string(),
            version_metadata: metadata.to_string(),
        }
    }

    #[test]
    fn version_number_returns_base_version() {
        let info = version_info("master", "", "abc123", "");
        assert_eq!(info.version_number(), "1.2.3");
    }

    #[test]
    fn version_number_includes_prerelease_and_metadata() {
        let info = version_info("master", "dev", "abc123", "build.7");
        assert_eq!(info.version_number(), "1.2.3-dev+build.7");
    }

    #[test]
    fn full_version_without_revision_block() {
        let info = version_info("feature/foo", "dev", "abc123", "");
        assert_eq!(info.full_version_number(false), "aura [Version 1.2.3]");
    }

    #[test]
    fn full_version_with_feature_branch_revision_block() {
        let info = version_info("feature/foo", "dev", "abc123", "");
        assert_eq!(
            info.full_version_number(true),
            "aura [Version 1.2.3/feature/foo (abc123)]"
        );
    }

    #[test]
    fn full_version_with_master_branch_revision_block() {
        let info = version_info("master", "", "abc123", "");
        assert_eq!(
            info.full_version_number(true),
            "aura [Version 1.2.3 (abc123)]"
        );
    }

    #[test]
    fn full_version_with_head_branch_revision_block() {
        let info = version_info("HEAD", "", "abc123", "");
        assert_eq!(
            info.full_version_number(true),
            "aura [Version 1.2.3 (abc123)]"
        );
    }

    #[test]
    fn full_version_without_git_metadata() {
        let info = version_info("feature/foo", "", "", "");
        assert_eq!(info.full_version_number(true), "aura [Version 1.2.3]");
    }
}
