use std::{env, process::Command};

#[derive(Debug, Default)]
pub struct GitLabContext {
    pub project: String,
    pub project_url: String,
    pub description: String,
    pub namespace: String,
    pub publisher: String,
    pub version: String,
    pub commit_message: String,
}

impl GitLabContext {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| env::var(key).unwrap_or_default())
    }

    fn from_lookup(get: impl Fn(&str) -> String) -> Self {
        let reference = first_nonempty([
            get("CI_COMMIT_TAG"),
            get("CI_COMMIT_REF_SLUG"),
            get("CI_COMMIT_REF_NAME"),
        ]);
        let sha = get("CI_COMMIT_SHORT_SHA");
        let version = [reference, sha]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        Self {
            project: get("CI_PROJECT_NAME"),
            project_url: get("CI_PROJECT_URL"),
            description: get("CI_PROJECT_DESCRIPTION"),
            namespace: get("CI_PROJECT_NAMESPACE"),
            publisher: first_nonempty([get("GITLAB_USER_LOGIN"), get("GITLAB_USER_NAME")]),
            version,
            commit_message: get("CI_COMMIT_MESSAGE"),
        }
    }
}

fn first_nonempty(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default()
}

/// 缺少 git 或读取失败时使用回退消息；浅克隆只返回本地可用的提交。
pub fn recent_commits(count: u32, fallback: &str) -> String {
    if count == 0 {
        return String::new();
    }
    match Command::new("git")
        .args([
            "--no-pager",
            "log",
            "--no-show-signature",
            "--no-color",
            "--pretty=format:- %s (%an) %cr",
            "--no-merges",
            &format!("-{count}"),
        ])
        .output()
    {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        _ => fallback.trim().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_variables_are_allowed() {
        let context = GitLabContext::from_lookup(|_| String::new());
        assert!(context.version.is_empty());
        assert_eq!(recent_commits(0, "fallback"), "");
    }

    #[test]
    fn tags_take_precedence() {
        let context = GitLabContext::from_lookup(|key| {
            match key {
                "CI_COMMIT_TAG" => "v1.0",
                "CI_COMMIT_REF_SLUG" => "main",
                "CI_COMMIT_SHORT_SHA" => "abc123",
                _ => "",
            }
            .into()
        });
        assert_eq!(context.version, "v1.0-abc123");
    }
}
