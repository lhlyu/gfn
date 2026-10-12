# gfn

用于 GitLab CI 的飞书机器人通知工具。从 GitHub Releases 下载对应平台的压缩包，解压后将二进制放入 PATH。

## 使用

```sh
export FEISHU_BOT_URL="https://open.feishu.cn/open-apis/bot/v2/hook/你的机器人ID"

# 发送通知
gfn success
gfn failure --title="❌ 生产部署失败" --color=red
gfn success --message="**环境:** staging" --commits=3

# 预览卡片 JSON，不发送请求，也不需要 webhook
gfn success --dry-run
```

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `<STATUS>` | 必填 | `success` / `failure` |
| `--title` | 根据状态 | 自定义通知标题 |
| `--color` | 根据状态 | 卡片颜色，可选值见 `--help` |
| `--message` | `FEISHU_MESSAGE` | 补充消息（支持 Markdown） |
| `--commits` | 5 | 最近非 merge 提交数，0–100 |
| `--timeout` | 10 | 请求超时秒数，1–300 |
| `--retries` | 2 | 网络错误、HTTP 429 / 5xx 的重试次数，0–5 |
| `--webhook` | `FEISHU_BOT_URL` | 机器人 webhook |
| `--secret` | `FEISHU_BOT_SECRET` | 机器人签名密钥 |
| `--dry-run` | false | 仅打印 JSON，不发送 |

命令行参数优先于环境变量。发送失败退出码为 1，参数错误为 2。

### GitLab CI

在现有 `stages` 最后添加 `notify` 阶段：

```yaml
notify_success:
  stage: notify
  when: on_success
  script:
    - gfn success

notify_failure:
  stage: notify
  when: on_failure
  script:
    - gfn failure
```

在 GitLab **Settings → CI/CD → Variables** 配置环境变量。若通知失败不应影响流水线，为通知 job 添加 `allow_failure: true`。

## 环境变量

| 变量 | 说明 |
| --- | --- |
| `FEISHU_BOT_URL` | 飞书自定义机器人 webhook（必填，除非禁用或 dry-run） |
| `FEISHU_BOT_SECRET` | 机器人签名密钥（启用签名校验时需要） |
| `FEISHU_MESSAGE` | 补充消息（支持 Markdown） |
| `FEISHU_DISABLE` | `true` / `1` / `yes` / `on` 禁用通知 |

**自动读取的 GitLab 变量**（无需手动配置）：

- `CI_PROJECT_NAME` / `CI_PROJECT_URL` / `CI_PROJECT_DESCRIPTION` / `CI_PROJECT_NAMESPACE`
- `GITLAB_USER_LOGIN` / `GITLAB_USER_NAME`
- `CI_COMMIT_TAG` / `CI_COMMIT_REF_SLUG` / `CI_COMMIT_REF_NAME` / `CI_COMMIT_SHORT_SHA` / `CI_COMMIT_MESSAGE`

获取最近提交需要 git，提交数量受 GitLab 的 `GIT_DEPTH` 限制。git 不可用或读取失败时回退到 `CI_COMMIT_MESSAGE`；`--commits=0` 禁用读取和回退。
