use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Status {
    Success,
    Failure,
}

impl Status {
    pub fn defaults(self) -> (&'static str, &'static str) {
        match self {
            Self::Success => ("🎉 有新的服务发布了~", "green"),
            Self::Failure => ("❌ 服务发布失败！", "red"),
        }
    }
}

#[derive(Debug, Parser)]
#[command(version, about = "将 GitLab CI 通知发送到飞书机器人")]
pub struct Cli {
    /// 通知状态：发布成功或失败
    pub status: Status,
    /// 自定义卡片标题，默认根据状态生成
    #[arg(long)]
    pub title: Option<String>,
    /// 卡片标题颜色，成功默认 green，失败默认 red
    #[arg(long, value_parser = ["blue", "wathet", "turquoise", "green", "yellow", "orange", "red", "carmine", "violet", "purple", "indigo", "grey"])]
    pub color: Option<String>,
    /// 机器人 webhook 地址（建议使用 FEISHU_BOT_URL，避免泄露到命令行历史）
    #[arg(long, env = "FEISHU_BOT_URL", hide_env_values = true)]
    pub webhook: Option<String>,
    /// 机器人签名密钥（建议使用 FEISHU_BOT_SECRET）
    #[arg(long, env = "FEISHU_BOT_SECRET", hide_env_values = true)]
    pub secret: Option<String>,
    /// 卡片补充消息（支持飞书 Markdown）
    #[arg(long, env = "FEISHU_MESSAGE")]
    pub message: Option<String>,
    /// 最近非合并提交的数量；0 表示不调用 git
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(0..=100))]
    pub commits: u32,
    /// 每次 HTTP 请求的超时时间，单位为秒
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=300))]
    pub timeout: u64,
    /// 发送请求时的网络错误、HTTP 429 和 5xx 的额外重试次数
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u32).range(0..=5))]
    pub retries: u32,
    /// 仅打印未签名的 JSON，不发送请求（无需 webhook）
    #[arg(long)]
    pub dry_run: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_script_arguments() {
        let cli = Cli::try_parse_from(["gfn", "success", "--color=yellow", "--title=自定义标题"])
            .unwrap();
        assert_eq!(cli.title.as_deref(), Some("自定义标题"));
        assert_eq!(cli.color.as_deref(), Some("yellow"));
    }

    #[test]
    fn reject_invalid_options() {
        for args in [
            vec!["failure", "--commits=101"],
            vec!["success", "--timeout=0"],
            vec!["unknown"],
            vec!["running"],
            vec!["canceled"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("gfn").chain(args)).is_err());
        }
    }
}
