mod card;
mod cli;
mod context;
mod feishu;

use anyhow::{Context, Result};
use clap::Parser;

fn notifications_disabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

fn run(cli: cli::Cli) -> Result<()> {
    if !cli.dry_run && notifications_disabled(&std::env::var("FEISHU_DISABLE").unwrap_or_default())
    {
        println!("已禁用飞书通知（FEISHU_DISABLE）");
        return Ok(());
    }
    let context = context::GitLabContext::from_env();
    let commits = context::recent_commits(cli.commits, &context.commit_message);
    let payload = card::build(&cli, &context, &commits)?;
    if cli.dry_run {
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }
    let webhook = cli
        .webhook
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .context("请设置 FEISHU_BOT_URL 或传入 --webhook（可使用 --dry-run 预览）")?;
    feishu::send(
        webhook,
        cli.secret.as_deref().filter(|s| !s.is_empty()),
        &payload,
        cli.timeout,
        cli.retries,
    )?;
    println!("飞书通知已发送");
    Ok(())
}

fn main() {
    if let Err(error) = run(cli::Cli::parse()) {
        eprintln!("错误：{error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disable_accepts_truthy_values() {
        for value in ["true", "TRUE", "1", "yes", "on", " True "] {
            assert!(notifications_disabled(value), "{value}");
        }
        for value in ["", "false", "0", "no", "off", "other"] {
            assert!(!notifications_disabled(value), "{value}");
        }
    }
}
