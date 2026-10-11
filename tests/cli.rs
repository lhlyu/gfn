use std::process::{Command, Output};

fn run(args: &[&str], disabled: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gfn"))
        .args(args)
        .env("FEISHU_DISABLE", disabled)
        .env_remove("FEISHU_BOT_URL")
        .env_remove("FEISHU_BOT_SECRET")
        .env_remove("FEISHU_MESSAGE")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap()
}

#[test]
fn disabled_notification_needs_no_webhook_or_checkout() {
    let output = run(&["success"], "true");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("已禁用"));
}

#[test]
fn dry_run_uses_embedded_template_even_when_disabled() {
    let output = run(&["failure", "--dry-run", "--commits=0"], "true");
    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["msg_type"], "interactive");
    assert_eq!(payload["card"]["header"]["template"], "red");
}

#[test]
fn invalid_arguments_exit_with_code_two_even_when_disabled() {
    for args in [
        vec!["unknown"],
        vec!["success", "--commits=101"],
        vec!["success", "--timeout=0"],
        vec!["success", "--retries=6"],
    ] {
        assert_eq!(run(&args, "true").status.code(), Some(2));
    }
}

#[test]
fn command_line_message_overrides_environment_and_dry_run_is_unsigned() {
    let output = Command::new(env!("CARGO_BIN_EXE_gfn"))
        .args(["success", "--dry-run", "--commits=0", "--message=from CLI"])
        .env("FEISHU_DISABLE", "false")
        .env("FEISHU_MESSAGE", "from environment")
        .env("FEISHU_BOT_SECRET", "private-test-secret")
        .env_remove("FEISHU_BOT_URL")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(payload.get("sign").is_none());
    assert!(payload.get("timestamp").is_none());
    let elements = payload["card"]["body"]["elements"].as_array().unwrap();
    assert!(
        elements
            .iter()
            .any(|element| element["content"] == "from CLI")
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-test-secret"));
}

#[test]
fn enabled_notification_requires_webhook() {
    let output = run(&["success", "--commits=0"], "false");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("FEISHU_BOT_URL"));
}
