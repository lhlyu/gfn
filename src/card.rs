use anyhow::{Context, Result};
use minijinja::{Environment, UndefinedBehavior, context};
use serde_json::Value;

use crate::{cli::Cli, context::GitLabContext};

const TEMPLATE: &str = include_str!("../templates/feishu-card.json.j2");

// 转义插入的元数据；--message 则保留 Markdown 格式。
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(
            c,
            '\\' | '*' | '_' | '[' | ']' | '(' | ')' | '`' | '~' | '<' | '>'
        ) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

fn display(value: &str) -> String {
    if value.is_empty() {
        "—".into()
    } else {
        escape(value)
    }
}

fn link(label: &str, url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => {
            let url = url.as_str().replace('(', "%28").replace(')', "%29");
            format!("[{}]({url})", display(label))
        }
        _ => display(label),
    }
}

fn text(label: &str, value: &str) -> String {
    format!("**{label}:** {}", display(value))
}

pub fn build(cli: &Cli, context: &GitLabContext, commits: &str) -> Result<Value> {
    let (default_title, default_color) = cli.status.defaults();
    let project = if context.description.is_empty() {
        context.project.clone()
    } else {
        format!("{} - {}", context.project, context.description)
    };
    let mut links = Vec::new();
    if !context.pipeline_url.is_empty() {
        links.push(link("查看流水线", &context.pipeline_url));
    }
    if !context.job_url.is_empty() {
        links.push(link("查看任务", &context.job_url));
    }

    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment
        .add_template("feishu-card", TEMPLATE)
        .context("注册飞书卡片模板失败")?;
    let rendered = environment
        .get_template("feishu-card")
        .context("加载飞书卡片模板失败")?
        .render(context! {
            title => cli.title.as_deref().unwrap_or(default_title),
            color => cli.color.as_deref().unwrap_or(default_color),
            project_text => format!("**🚀 项目:** {}", link(&project, &context.project_url)),
            namespace_text => text("🏷️ 群组", &context.namespace),
            publisher_text => text("👷 发布人", &context.publisher),
            version_text => text("🔖 版本", &context.version),
            links => links.join(" | "),
            message => cli.message.as_deref().unwrap_or_default(),
            commits => escape(commits),
        })
        .context("渲染飞书卡片模板失败")?;
    serde_json::from_str(&rendered).context("解析飞书卡片 JSON 失败")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn payload_roundtrips_special_characters() {
        let cli =
            Cli::try_parse_from(["gfn", "failure", "--title=标题\"\n", "--message=**hello**"])
                .unwrap();
        let context = GitLabContext {
            project: "a[b]".into(),
            ..Default::default()
        };
        let body = build(&cli, &context, "fix \"quotes\"\nsecond line").unwrap();
        let serialized = serde_json::to_string(&body).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&serialized).unwrap(), body);
        assert_eq!(body["card"]["header"]["template"], "red");
        assert_eq!(body["card"]["header"]["title"]["content"], "标题\"\n");
        assert!(serialized.contains("**hello**"));
        assert_eq!(escape("a[b]"), "a\\[b\\]");
    }

    #[test]
    fn optional_sections_render_valid_json_in_all_combinations() {
        for mask in 0..8 {
            let mut cli = Cli::try_parse_from(["gfn", "success"]).unwrap();
            let mut context = GitLabContext {
                project: "demo".into(),
                project_url: "https://gitlab.example.com/demo".into(),
                ..Default::default()
            };
            if mask & 1 != 0 {
                context.pipeline_url = "https://gitlab.example.com/pipelines/1".into();
                context.job_url = "https://gitlab.example.com/jobs/1".into();
            }
            if mask & 2 != 0 {
                cli.message = Some("**消息**\n\"quoted\" \\ 中文".into());
            }
            let commits = if mask & 4 != 0 {
                "fix \"quotes\"\nnext"
            } else {
                ""
            };
            let body = build(&cli, &context, commits).unwrap();
            let elements = body["card"]["body"]["elements"].as_array().unwrap();
            assert_eq!(
                elements.len(),
                1 + usize::from(mask & 1 != 0)
                    + usize::from(mask & 2 != 0)
                    + 2 * usize::from(mask & 4 != 0)
            );
            assert_eq!(
                elements[0]["columns"][0]["elements"][0]["content"],
                "**🚀 项目:** [demo](https://gitlab.example.com/demo)"
            );
        }
    }
}
