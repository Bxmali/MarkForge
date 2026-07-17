use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatRequest {
    pub settings: AiSettings,
    pub user_message: String,
    pub history: Vec<AiChatTurn>,
    pub current_options: Value,
    pub queue_count: usize,
    pub output_dir_set: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatResponse {
    pub reply: String,
    pub options_patch: Option<Value>,
    pub actions: Vec<String>,
}

const SYSTEM_PROMPT: &str = r#"你是 MarkForge「AI 模式」高级水印导演。用户只说自然语言需求，你负责理解并一次性改好参数、触发操作，不要让用户去拧滑块。

必须只返回一个 JSON 对象（不要 markdown 代码块），字段：
{
  "reply": "给用户看的自然语言回复（亲切、简短、像聊天）",
  "optionsPatch": {
    "text": "可选，水印文案",
    "position": "可选，九宫格：top-left/top-center/top-right/center-left/center/center-right/bottom-left/bottom-center/bottom-right",
    "opacity": "可选，0.15~0.95",
    "fontScale": "可选，0.018~0.08",
    "textColor": "可选，#RRGGBB",
    "strokeColor": "可选，#RRGGBB",
    "marginRatio": "可选，0.015~0.12",
    "style": "可选，single=固定单点水印；moving=单点水印在画面里来回跑（视频动态水印，按帧换位置）；tiled=满屏平铺防盗水印",
    "crop": "可选对象 { left, top, right, bottom }，每边裁掉的比例 0~0.45。用户说裁切/去黑边/裁上下左右时填写"
  },
  "actions": ["可选：start_batch / pick_images / pick_output / open_output / clear_queue / retry_failed"]
}

能力与规则：
- 用户一句话描述风格、位置、文案、裁切、动态水印时，尽量在同一次 optionsPatch 里改全
- 「视频动态水印 / 来回跑 / 四处飘 / bounce」→ style=moving（不是 tiled），单点水印按帧在画面里弹跳移动
- 「满屏水印 / 防盗水印 / 斜向铺满」→ style=tiled，并适当降低 opacity（如 0.25~0.4）、略缩小字号
- 「角落水印 / 普通水印 / 底部居中」→ style=single + 对应 position
- 「裁掉上下/左右 / 去边 / 裁成更方」→ 填 crop 各边比例；只提到某一边时其它边为 0
- 用户说「开始/批量/导出」且已有图与输出目录时，actions 加 start_batch
- 「选图/导入」→ pick_images；「输出目录」→ pick_output；「打开输出」→ open_output
- 没有把握时 optionsPatch 可省略或只改提到的项
- reply 要确认你做了什么（例如「已改成来回跑的动态水印」），不要复读 JSON
"#;

fn extract_json_object(raw: &str) -> Result<Value, String> {
    let text = raw.trim();
    let stripped = if text.starts_with("```") {
        let without_fence = text
            .trim_start_matches("```")
            .trim_start_matches("json")
            .trim_start_matches("JSON")
            .trim();
        without_fence
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        text.to_string()
    };
    let start = stripped
        .find('{')
        .ok_or_else(|| "模型未返回 JSON 对象".to_string())?;
    let end = stripped
        .rfind('}')
        .ok_or_else(|| "模型 JSON 不完整".to_string())?;
    if end <= start {
        return Err("模型 JSON 无效".into());
    }
    serde_json::from_str(&stripped[start..=end]).map_err(|e| format!("解析模型 JSON 失败：{e}"))
}

pub async fn chat(req: AiChatRequest) -> Result<AiChatResponse, String> {
    let base = req.settings.base_url.trim().trim_end_matches('/');
    let key = req.settings.api_key.trim();
    let model = req.settings.model.trim();
    if base.is_empty() {
        return Err("请先填写 AI 中转站地址（Base URL）".into());
    }
    if key.is_empty() {
        return Err("请先填写 API Key（sk-...）".into());
    }
    if model.is_empty() {
        return Err("请先填写模型名称".into());
    }

    let mut messages = vec![json!({
        "role": "system",
        "content": SYSTEM_PROMPT,
    })];

    let context = format!(
        "当前水印参数：{}\n队列图片数：{}\n已选输出目录：{}",
        req.current_options,
        req.queue_count,
        if req.output_dir_set { "是" } else { "否" }
    );
    messages.push(json!({
        "role": "system",
        "content": context,
    }));

    for turn in &req.history {
        let role = if turn.role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        messages.push(json!({
            "role": role,
            "content": turn.content,
        }));
    }
    messages.push(json!({
        "role": "user",
        "content": req.user_message,
    }));

    let url = format!("{base}/chat/completions");
    let body = json!({
        "model": model,
        "temperature": 0.4,
        "max_tokens": 1000,
        "messages": messages,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{e}"))?;

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求 AI 失败：{e}"))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取 AI 响应失败：{e}"))?;
    if !status.is_success() {
        return Err(format!(
            "AI 返回 HTTP {status}：{}",
            text.chars().take(400).collect::<String>()
        ));
    }

    let data: Value =
        serde_json::from_str(&text).map_err(|e| format!("AI 响应不是 JSON：{e}"))?;
    let content = data
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if content.trim().is_empty() {
        return Err("模型没有返回内容".into());
    }

    let parsed = extract_json_object(&content)?;
    let reply = parsed
        .get("reply")
        .and_then(|v| v.as_str())
        .unwrap_or("好的，我已按你的意思处理啦～")
        .to_string();
    let options_patch = parsed
        .get("optionsPatch")
        .cloned()
        .or_else(|| parsed.get("options_patch").cloned())
        .filter(|v| v.is_object());
    let actions = parsed
        .get("actions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(AiChatResponse {
        reply,
        options_patch,
        actions,
    })
}
