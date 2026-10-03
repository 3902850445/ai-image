// ai-image 生图核心：API 调用（文生图/图生图）与图片保存
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::Local;
use serde::Serialize;
use std::time::Duration;

/// 追加一行中文日志到 exe 同目录 logs/ai-image.log（写失败静默忽略，不影响主流程）
fn log_line(level: &str, msg: &str) {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(dir) = exe.parent() else { return };
    let log_dir = dir.join("logs");
    if std::fs::create_dir_all(&log_dir).is_err() {
        return;
    }
    let line = format!(
        "[{}][{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        level,
        msg
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("ai-image.log"))
    {
        use std::io::Write as _;
        let _ = f.write_all(line.as_bytes());
    }
}

/// 截断长文本用于日志
fn clip(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

/// 统一返回给前端的生成结果（图片一律转成 base64，便于展示与落盘）
#[derive(Serialize)]
pub struct GenResult {
    /// 图片 base64（不带 data: 前缀）
    pub image_b64: String,
    /// 图片 MIME 类型（当前接口返回均为 png）
    pub mime: String,
    /// 实际使用的模型名
    pub model: String,
}

/// 规范化 baseUrl：去空白、去尾部斜杠、去尾部 /v1（与原项目行为一致）
fn normalize_base(raw: &str) -> String {
    let t = raw.trim();
    let t = t.trim_end_matches('/');
    let lower = t.to_lowercase();
    if lower.ends_with("/v1") {
        t[..t.len() - 3].to_string()
    } else {
        t.to_string()
    }
}

fn http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建网络客户端失败：{e}"))
}

/// 从供应商响应中提取图片；b64_json 直接用，url 则由 Rust 下载后转 base64
async fn extract_image(json: serde_json::Value) -> Result<GenResult, String> {
    let item = json["data"]
        .as_array()
        .and_then(|a| a.first())
        .or_else(|| json["images"].as_array().and_then(|a| a.first()))
        .ok_or_else(|| "供应商没有返回图片".to_string())?;

    if let Some(b64) = item["b64_json"].as_str() {
        if !b64.is_empty() {
            return Ok(GenResult {
                image_b64: b64.to_string(),
                mime: "image/png".into(),
                model: "gpt-image-2".into(),
            });
        }
    }

    if let Some(url) = item["url"].as_str() {
        if !url.is_empty() {
            log_line("INFO", &format!("接口返回图片 URL，开始由本地下载：{url}"));
            let client = http_client(180)?;
            let resp = match client.get(url).send().await {
                Ok(r) => r,
                Err(e) => {
                    log_line("ERROR", &format!("下载图片失败：{url}：{e}"));
                    return Err(format!("下载图片失败：{e}"));
                }
            };
            let status = resp.status();
            let bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    log_line("ERROR", &format!("读取图片数据失败（HTTP {}）：{e}", status.as_u16()));
                    return Err(format!("读取图片数据失败（HTTP {}）：{e}", status.as_u16()));
                }
            };
            if !status.is_success() {
                log_line("ERROR", &format!("下载图片失败（HTTP {}）：{url}", status.as_u16()));
                return Err(format!("下载图片失败（HTTP {}）", status.as_u16()));
            }
            log_line("INFO", &format!("图片下载完成（{} KB）", bytes.len() / 1024));
            return Ok(GenResult {
                image_b64: B64.encode(&bytes),
                mime: "image/png".into(),
                model: "gpt-image-2".into(),
            });
        }
    }

    Err("供应商没有返回图片".to_string())
}

/// 文生图：POST {baseUrl}/v1/images/generations
#[tauri::command]
async fn generate_image(
    base_url: String,
    api_key: String,
    prompt: String,
    size: String,
    quality: String,
    response_format: Option<String>,
) -> Result<GenResult, String> {
    let root = normalize_base(&base_url);
    if root.is_empty() {
        return Err("API 地址不能为空，请先在设置中填写".into());
    }
    if api_key.trim().is_empty() {
        return Err("请先在设置中配置 API Key".into());
    }
    if prompt.trim().is_empty() {
        return Err("提示词不能为空".into());
    }

    let url = format!("{root}/v1/images/generations");
    let mut body = serde_json::json!({
        "model": "gpt-image-2",
        "prompt": prompt,
        "n": 1,
        "size": size,
        "quality": quality,
    });
    if let Some(rf) = response_format {
        if !rf.is_empty() {
            body["response_format"] = serde_json::json!(rf);
        }
    }
    log_line(
        "INFO",
        &format!(
            "文生图请求 → {url}（size={}，quality={}，response_format={:?}）",
            body["size"].as_str().unwrap_or(""),
            body["quality"].as_str().unwrap_or(""),
            body["response_format"].as_str()
        ),
    );
    let start = std::time::Instant::now();

    let client = http_client(300)?;
    let resp = match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key.trim()))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log_line(
                "ERROR",
                &format!("文生图请求失败（{}s）→ {url}：{e}", start.elapsed().as_secs_f32().round()),
            );
            return Err(format!("请求失败：{e}"));
        }
    };

    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            log_line("ERROR", &format!("文生图读取响应失败（HTTP {}）：{e}", status.as_u16()));
            return Err(format!("读取响应失败（HTTP {}）：{e}", status.as_u16()));
        }
    };

    if !status.is_success() {
        log_line(
            "ERROR",
            &format!(
                "文生图接口返回 HTTP {}，响应原文：{}",
                status.as_u16(),
                clip(&text, 800)
            ),
        );
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        let msg = json["error"]["message"]
            .as_str()
            .unwrap_or("供应商返回了错误");
        return Err(format!("生成失败（HTTP {}）：{msg}", status.as_u16()));
    }

    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            log_line(
                "ERROR",
                &format!("文生图响应不是有效 JSON：{e}，原文：{}", clip(&text, 800)),
            );
            return Err(format!("解析响应失败（HTTP {}）：{e}", status.as_u16()));
        }
    };
    log_line(
        "INFO",
        &format!(
            "文生图接口返回 HTTP {}（{} KB，{}s），开始提取图片",
            status.as_u16(),
            text.len() / 1024,
            start.elapsed().as_secs_f32().round()
        ),
    );

    extract_image(json).await
}

/// 图生图：POST {baseUrl}/v1/images/edits（multipart，参考图字段名 image）
#[tauri::command]
async fn edit_image(
    base_url: String,
    api_key: String,
    prompt: String,
    size: String,
    quality: String,
    images_b64: Vec<String>,
    response_format: Option<String>,
) -> Result<GenResult, String> {
    let root = normalize_base(&base_url);
    if root.is_empty() {
        return Err("API 地址不能为空，请先在设置中填写".into());
    }
    if api_key.trim().is_empty() {
        return Err("请先在设置中配置 API Key".into());
    }
    if prompt.trim().is_empty() {
        return Err("提示词不能为空".into());
    }
    if images_b64.is_empty() {
        return Err("图生图至少需要一张参考图".into());
    }

    let url = format!("{root}/v1/images/edits");
    log_line(
        "INFO",
        &format!(
            "图生图请求 → {url}（参考图 {} 张，size={size}，quality={quality}）",
            images_b64.len()
        ),
    );
    let start = std::time::Instant::now();
    let mut form = reqwest::multipart::Form::new()
        .text("prompt", prompt)
        .text("model", "gpt-image-2")
        .text("size", size)
        .text("quality", quality)
        .text("n", "1");
    if let Some(rf) = response_format {
        if !rf.is_empty() {
            form = form.text("response_format", rf);
        }
    }
    for (i, b64) in images_b64.iter().enumerate() {
        let bytes = B64
            .decode(b64)
            .map_err(|e| format!("第 {} 张参考图解码失败：{e}", i + 1))?;
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(format!("ref_{}.png", i + 1))
            .mime_str("image/png")
            .map_err(|e| format!("构造参考图分片失败：{e}"))?;
        form = form.part("image", part);
    }

    let client = http_client(300)?;
    let resp = match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key.trim()))
        .multipart(form)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log_line(
                "ERROR",
                &format!("图生图请求失败（{}s）→ {url}：{e}", start.elapsed().as_secs_f32().round()),
            );
            return Err(format!("请求失败：{e}"));
        }
    };

    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            log_line("ERROR", &format!("图生图读取响应失败（HTTP {}）：{e}", status.as_u16()));
            return Err(format!("读取响应失败（HTTP {}）：{e}", status.as_u16()));
        }
    };

    if !status.is_success() {
        log_line(
            "ERROR",
            &format!(
                "图生图接口返回 HTTP {}，响应原文：{}",
                status.as_u16(),
                clip(&text, 800)
            ),
        );
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        let msg = json["error"]["message"]
            .as_str()
            .unwrap_or("供应商返回了错误");
        return Err(format!("生成失败（HTTP {}）：{msg}", status.as_u16()));
    }

    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            log_line(
                "ERROR",
                &format!("图生图响应不是有效 JSON：{e}，原文：{}", clip(&text, 800)),
            );
            return Err(format!("解析响应失败（HTTP {}）：{e}", status.as_u16()));
        }
    };
    log_line(
        "INFO",
        &format!(
            "图生图接口返回 HTTP {}（{} KB，{}s），开始提取图片",
            status.as_u16(),
            text.len() / 1024,
            start.elapsed().as_secs_f32().round()
        ),
    );

    extract_image(json).await
}

/// 清理文件名中的非法字符（Windows/桌面平台通用）
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "image".to_string()
    } else {
        trimmed
    }
}

/// 保存图片：桌面平台写入指定目录文件；移动平台经 gallery 插件写入系统相册
#[tauri::command]
fn save_image(app: tauri::AppHandle, data_b64: String, dir: String, filename: String) -> Result<String, String> {
    #[cfg(desktop)]
    let _ = &app;
    #[cfg(mobile)]
    let _ = &dir;
    #[cfg(mobile)]
    return save_image_to_gallery(app, data_b64, filename);
    #[cfg(desktop)]
    return save_image_to_file(data_b64, dir, filename);
}

/// 移动端：通过 gallery 插件写入系统相册（Android → Pictures/ai-image）
#[cfg(mobile)]
fn save_image_to_gallery(
    app: tauri::AppHandle,
    data_b64: String,
    filename: String,
) -> Result<String, String> {
    use tauri_plugin_gallery::{GalleryExt, SaveRequest};
    let safe_name = sanitize_filename(&filename);
    log_line("INFO", &format!("保存图片到系统相册 → {safe_name}"));
    let bytes = B64
        .decode(&data_b64)
        .map_err(|e| format!("图片数据解码失败：{e}"))?;
    if bytes.is_empty() {
        log_line("ERROR", "保存失败：图片数据为空");
        return Err("图片数据为空，无法保存".into());
    }
    let resp = app
        .gallery()
        .save_to_gallery(SaveRequest {
            data_b64,
            filename: safe_name,
        })
        .map_err(|e| {
            log_line("ERROR", &format!("保存到相册失败：{e}"));
            format!("保存到相册失败：{e}")
        })?;
    log_line("INFO", &format!("图片已保存到相册：{}", resp.uri));
    Ok(resp.uri)
}

/// 桌面端：直接写入指定目录文件
#[cfg(desktop)]
fn save_image_to_file(data_b64: String, dir: String, filename: String) -> Result<String, String> {
    log_line("INFO", &format!("保存图片 → {dir}\\{filename}"));
    if dir.trim().is_empty() {
        log_line("ERROR", "保存失败：保存目录为空");
        return Err("保存目录为空，请先在设置中选择保存路径".into());
    }
    let bytes = match B64.decode(&data_b64) {
        Ok(b) => b,
        Err(e) => {
            log_line("ERROR", &format!("保存失败：图片数据解码失败：{e}"));
            return Err(format!("图片数据解码失败：{e}"));
        }
    };
    if bytes.is_empty() {
        log_line("ERROR", "保存失败：图片数据为空");
        return Err("图片数据为空，无法保存".into());
    }
    let safe_name = sanitize_filename(&filename);
    let dir_path = std::path::Path::new(dir.trim());
    if !dir_path.exists() {
        if let Err(e) = std::fs::create_dir_all(dir_path) {
            log_line("ERROR", &format!("保存失败：创建目录 {dir} 失败：{e}"));
            return Err(format!("创建保存目录失败：{e}"));
        }
    }
    let path = dir_path.join(&safe_name);
    if let Err(e) = std::fs::write(&path, &bytes) {
        log_line("ERROR", &format!("保存失败：写入 {} 失败：{e}", path.display()));
        return Err(format!("写入文件失败：{e}"));
    }
    log_line("INFO", &format!("图片已保存：{}（{} KB）", path.display(), bytes.len() / 1024));
    Ok(path.display().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_gallery::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            generate_image,
            edit_image,
            save_image
        ])
        .setup(|_app| {
            log_line("INFO", "应用启动完成");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
