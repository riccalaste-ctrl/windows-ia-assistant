use std::process::Command;
use serde_json::{json, Value};

fn open_shell(target: &str) -> Result<(), String> {
    Command::new("cmd").args(["/C", "start", "", target]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_target(target: String) -> Result<(), String> {
    let t = target.trim();
    if t.is_empty() { return Err("Destinazione vuota".into()); }
    open_shell(t)
}

#[tauri::command]
fn web_search(query: String) -> Result<(), String> {
    let q = urlencoding::encode(query.trim());
    open_shell(&format!("https://www.google.com/search?q={}", q))
}

#[tauri::command]
fn save_secret(key: String, value: String) -> Result<(), String> {
    if key != "openai-api-key" { return Err("Chiave non consentita".into()); }
    keyring::Entry::new("windows-ia-assistant", &key)
        .map_err(|e| e.to_string())?
        .set_password(&value)
        .map_err(|e| e.to_string())
}

fn openai_key() -> Result<String, String> {
    keyring::Entry::new("windows-ia-assistant", "openai-api-key")
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|_| "Configura prima la OpenAI API key.".to_string())
}

fn tool_definitions() -> Value {
    json!([
      {"type":"function","name":"open_target","description":"Open a Windows application, file, folder, or URL using the Windows shell.","parameters":{"type":"object","properties":{"target":{"type":"string"}},"required":["target"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"web_search","description":"Open a Google web search in the user's default browser.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"find_file","description":"Find files by name in the user's common Windows folders. Returns matching paths.","parameters":{"type":"object","properties":{"name":{"type":"string"},"max_results":{"type":"integer"}},"required":["name"],"additionalProperties":false,"strict":true}}
    ])
}

fn find_file(name: &str, max_results: usize) -> Result<Value, String> {
    let home = std::env::var_os("USERPROFILE").ok_or("USERPROFILE non disponibile")?;
    let roots = ["Downloads", "Documents", "Desktop"].map(|x| std::path::PathBuf::from(&home).join(x));
    let needle = name.to_lowercase();
    let mut matches = Vec::new();

    for root in roots {
        if !root.exists() { continue; }
        for entry in walkdir::WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok) {
            if matches.len() >= max_results { break; }
            let file_name = entry.file_name().to_string_lossy().to_lowercase();
            if file_name.contains(&needle) {
                matches.push(entry.path().to_string_lossy().to_string());
            }
        }
        if matches.len() >= max_results { break; }
    }

    Ok(json!({"matches": matches}))
}

fn execute_tool(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "open_target" => {
            let target = args.get("target").and_then(Value::as_str).ok_or("target mancante")?;
            open_target(target.to_string())?;
            Ok(json!({"ok":true,"opened":target}))
        }
        "web_search" => {
            let query = args.get("query").and_then(Value::as_str).ok_or("query mancante")?;
            web_search(query.to_string())?;
            Ok(json!({"ok":true,"query":query}))
        }
        "find_file" => {
            let name = args.get("name").and_then(Value::as_str).ok_or("name mancante")?;
            let max = args.get("max_results").and_then(Value::as_u64).unwrap_or(10).clamp(1, 50) as usize;
            find_file(name, max)
        }
        _ => Err(format!("Tool non autorizzato: {}", name))
    }
}

async fn response_request(client: &reqwest::Client, key: &str, body: Value) -> Result<Value, String> {
    let response = client.post("https://api.openai.com/v1/responses")
        .bearer_auth(key)
        .json(&body)
        .send().await.map_err(|e| e.to_string())?;

    let status = response.status();
    let value: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(value.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or("OpenAI API error").to_string());
    }
    Ok(value)
}

#[tauri::command]
async fn agent_message(message: String) -> Result<String, String> {
    let key = openai_key()?;
    let client = reqwest::Client::new();
    let mut body = json!({
      "model": std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-6-luna".into()),
      "store": true,
      "instructions": "You are a personal Windows voice agent. Speak Italian unless the user asks otherwise. You can act on the user's Windows computer only through the declared local tools. Never invent that an action happened: use a tool and verify its result. Prefer direct, concise responses. Do not expose internal tool JSON to the user.",
      "input": [{"role":"user","content":[{"type":"input_text","text":message}]}],
      "tools": tool_definitions()
    });

    for _ in 0..8 {
        let response = response_request(&client, &key, body.clone()).await?;
        let output = response.get("output").and_then(Value::as_array).ok_or("Risposta OpenAI senza output")?;
        let mut tool_outputs = Vec::new();
        let mut text_parts = Vec::new();

        for item in output {
            match item.get("type").and_then(Value::as_str) {
                Some("function_call") => {
                    let name = item.get("name").and_then(Value::as_str).ok_or("function_call senza nome")?;
                    let args_text = item.get("arguments").and_then(Value::as_str).unwrap_or("{}");
                    let args: Value = serde_json::from_str(args_text).map_err(|e| e.to_string())?;
                    let result = match execute_tool(name, &args) {
                        Ok(v) => v,
                        Err(e) => json!({"ok":false,"error":e})
                    };
                    let call_id = item.get("call_id").and_then(Value::as_str).ok_or("function_call senza call_id")?;
                    tool_outputs.push(json!({"type":"function_call_output","call_id":call_id,"output":result.to_string()}));
                }
                Some("message") => {
                    if let Some(contents) = item.get("content").and_then(Value::as_array) {
                        for c in contents {
                            if let Some(t) = c.get("text").and_then(Value::as_str) { text_parts.push(t.to_string()); }
                        }
                    }
                }
                _ => {}
            }
        }

        if !tool_outputs.is_empty() {
            body = json!({
              "model": std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-6-luna".into()),
              "store": true,
              "previous_response_id": response.get("id").and_then(Value::as_str).ok_or("Response senza id")?,
              "input": tool_outputs,
              "tools": tool_definitions()
            });
            continue;
        }

        if !text_parts.is_empty() { return Ok(text_parts.join("\n")); }
        return Ok("Operazione completata.".into());
    }

    Err("Troppi passaggi di tool.".into())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![open_target, web_search, save_secret, agent_message])
        .run(tauri::generate_context!())
        .expect("error while running Windows IA Assistant");
}
