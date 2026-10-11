use std::{io::Read, thread, time::Duration};

use anyhow::{Context, Result, bail};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// 飞书使用“时间戳 + 换行符 + 密钥”作为 HMAC 密钥，对空消息签名。
pub fn sign(timestamp: &str, secret: &str) -> Result<String> {
    let mac = HmacSha256::new_from_slice(format!("{timestamp}\n{secret}").as_bytes())
        .context("签名密钥无效")?;
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        mac.finalize().into_bytes(),
    ))
}

fn read_response(response: reqwest::blocking::Response) -> Result<Value> {
    // webhook 响应通常很小；限制读取大小，防止错误端点导致无界内存分配。
    const LIMIT: u64 = 64 * 1024;
    let mut bytes = Vec::new();
    response
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("读取飞书响应失败"))?;
    if bytes.len() as u64 > LIMIT {
        bail!("飞书响应超过 64 KiB");
    }
    serde_json::from_slice(&bytes).context("飞书返回的 JSON 无效")
}

fn retryable(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

pub fn send(
    url: &str,
    secret: Option<&str>,
    payload: &Value,
    timeout: u64,
    retries: u32,
) -> Result<()> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| anyhow::anyhow!("FEISHU_BOT_URL 必须是有效的 URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("webhook URL 必须使用 http 或 https");
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("创建 HTTP 客户端失败")?;
    let mut last_error = String::new();
    for attempt in 0..=retries {
        let timestamp = secret
            .map(|_| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .context("系统时间早于 Unix epoch，请检查系统时钟")
                    .map(|duration| duration.as_secs().to_string())
            })
            .transpose()?;
        let mut body = payload.clone();
        if let (Some(secret), Some(timestamp)) = (secret, timestamp.as_deref()) {
            body["timestamp"] = json!(timestamp);
            body["sign"] = json!(sign(timestamp, secret)?);
        }
        let result = client.post(parsed.clone()).json(&body).send();
        match result {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let result = read_response(response)?;
                    let code = result
                        .get("code")
                        .or_else(|| result.get("StatusCode"))
                        .and_then(Value::as_i64)
                        .context("飞书响应缺少数值类型的状态码")?;
                    if code != 0 {
                        bail!("飞书拒绝通知（错误码：{code}）");
                    }
                    return Ok(());
                }
                last_error = format!("HTTP {status}");
                if !retryable(status) {
                    bail!("飞书请求失败：{last_error}");
                }
            }
            Err(error) => last_error = format!("请求错误：{}", error.without_url()),
        }
        if attempt < retries {
            thread::sleep(Duration::from_secs(1_u64 << attempt.min(4)));
        }
    }
    bail!("飞书请求失败，已尝试 {} 次：{last_error}", retries + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_is_deterministic() {
        assert_eq!(
            sign("1", "secret").unwrap(),
            "6o+SjynWLFd+QtSzfgy9uvrayMJ+/S8z4k5MmO7xW68="
        );
    }

    fn mock_server(responses: Vec<String>) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let length = stream.read(&mut buffer).unwrap();
                    assert!(length > 0);
                    request.extend_from_slice(&buffer[..length]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                        let size: usize = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .unwrap()
                            .parse()
                            .unwrap();
                        if request.len() >= end + 4 + size {
                            break;
                        }
                    }
                }
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        (url, handle)
    }

    #[test]
    fn http_success_and_business_error() {
        for (body, success) in [
            ("{\"StatusCode\":0}", true),
            ("{\"code\":0}", true),
            ("{\"code\":19021}", false),
            ("{}", false),
            ("not json", false),
        ] {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let (url, server) = mock_server(vec![response]);
            assert_eq!(send(&url, None, &json!({}), 2, 0).is_ok(), success);
            server.join().unwrap();
        }
    }

    #[test]
    fn retries_server_errors() {
        let (url, server) = mock_server(vec![
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .into(),
            "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n{\"code\":0}".into(),
        ]);
        send(&url, Some("secret"), &json!({}), 2, 1).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn rejects_large_response() {
        let body = "x".repeat(64 * 1024 + 1);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (url, server) = mock_server(vec![response]);
        let error = send(&url, None, &json!({}), 2, 0).unwrap_err();
        assert!(error.to_string().contains("超过 64 KiB"));
        server.join().unwrap();
    }

    #[test]
    fn does_not_follow_redirects_or_retry_client_errors() {
        for status in ["302 Found", "400 Bad Request"] {
            let response = format!(
                "HTTP/1.1 {status}\r\nLocation: http://127.0.0.1:1/secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let (url, server) = mock_server(vec![response]);
            let error = send(&url, None, &json!({}), 2, 2).unwrap_err();
            assert!(error.to_string().contains(status));
            assert!(!error.to_string().contains(&url));
            server.join().unwrap();
        }
    }

    #[test]
    fn connection_errors_do_not_expose_webhook() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/private-token", listener.local_addr().unwrap());
        drop(listener);
        let error = send(&url, None, &json!({}), 1, 0).unwrap_err();
        assert!(!format!("{error:#}").contains("private-token"));
    }

    #[test]
    fn retry_only_throttling_or_server_errors() {
        assert!(retryable(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(retryable(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(!retryable(reqwest::StatusCode::BAD_REQUEST));
    }
}
