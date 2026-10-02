use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};
use tauri::Manager;

static LAST_RESPONSE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static PENDING_ACTION: OnceLock<Mutex<Option<PendingAction>>> = OnceLock::new();

#[derive(Clone, Debug)]
struct PendingAction {
    response_id: String,
    call_id: String,
    name: String,
    args: Value,
}

fn last_response() -> &'static Mutex<Option<String>> {
    LAST_RESPONSE.get_or_init(|| Mutex::new(None))
}

fn pending_action() -> &'static Mutex<Option<PendingAction>> {
    PENDING_ACTION.get_or_init(|| Mutex::new(None))
}

fn user_root() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Cartella utente non disponibile.".into())
}

fn allowed_roots() -> Result<Vec<PathBuf>, String> {
    let home = user_root()?;
    Ok(["Desktop", "Documents", "Downloads", "Pictures", "Videos"]
        .iter()
        .map(|name| home.join(name))
        .collect())
}

fn is_allowed_path(path: &Path) -> Result<bool, String> {
    let candidate = if path.exists() {
        path.canonicalize().map_err(|e| e.to_string())?
    } else {
        let parent = path.parent().ok_or("Percorso non valido.")?;
        parent.canonicalize().map_err(|e| e.to_string())?.join(
            path.file_name().ok_or("Nome file non valido.")?
        )
    };
    Ok(allowed_roots()?.into_iter().any(|root| candidate.starts_with(root)))
}

fn open_path_or_url(target: &str) -> Result<(), String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("Destinazione vuota.".into());
    }
    if target.starts_with("https://") || target.starts_with("http://") {
        Command::new("cmd").args(["/C", "start", "", target]).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let path = PathBuf::from(target);
    if !path.exists() || !is_allowed_path(&path)? {
        return Err("Per sicurezza posso aprire solo file e cartelle nelle cartelle utente consentite.".into());
    }
    Command::new("explorer.exe").arg(path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn web_search(query: &str) -> Result<(), String> {
    let query = query.trim();
    if query.is_empty() { return Err("Ricerca vuota.".into()); }
    let url = format!("https://www.google.com/search?q={}", urlencoding::encode(query));
    open_path_or_url(&url)
}

fn openai_key() -> Result<String, String> {
    if let Ok(value) = std::env::var("OPENAI_API_KEY") {
        if !value.trim().is_empty() { return Ok(value); }
    }
    keyring::Entry::new("windows-ia-assistant", "openai-api-key")
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|_| "Configura la OpenAI API key.".to_string())
}

fn model_name() -> String {
    std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-6-luna".to_string())
}

fn save_openai_api_key(value: String) -> Result<(), String> {
    if value.trim().is_empty() { return Err("API key vuota.".into()); }
    keyring::Entry::new("windows-ia-assistant", "openai-api-key")
        .map_err(|e| e.to_string())?
        .set_password(value.trim())
        .map_err(|e| e.to_string())
}

fn tool_definitions() -> Value {
    json!([
      {"type":"function","name":"open_path","description":"Open a permitted user file or folder in Windows Explorer.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"open_url","description":"Open an http or https URL in the default browser.","parameters":{"type":"object","properties":{"url":{"type":"string"}},"required":["url"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"web_search","description":"Open a web search in the default browser.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"find_file","description":"Find files or folders by name under Desktop, Documents, Downloads, Pictures and Videos.","parameters":{"type":"object","properties":{"name":{"type":"string"},"max_results":{"type":"integer"}},"required":["name"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"list_directory","description":"List entries in a permitted user folder.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"create_text_file","description":"Create a UTF-8 text file in a permitted user folder. Requires confirmation.","parameters":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"rename_file","description":"Rename a file or folder in a permitted user folder. Requires confirmation.","parameters":{"type":"object","properties":{"path":{"type":"string"},"new_name":{"type":"string"}},"required":["path","new_name"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"move_file","description":"Move a file or folder between permitted user folders. Requires confirmation.","parameters":{"type":"object","properties":{"source":{"type":"string"},"destination":{"type":"string"}},"required":["source","destination"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"copy_file","description":"Copy a file between permitted user folders. Requires confirmation.","parameters":{"type":"object","properties":{"source":{"type":"string"},"destination":{"type":"string"}},"required":["source","destination"],"additionalProperties":false,"strict":true}},
      {"type":"function","name":"delete_file","description":"Delete a file or empty folder inside a permitted user folder. Requires confirmation.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false,"strict":true}}
    ])
}

fn find_file(name: &str, max_results: usize) -> Result<Value, String> {
    let needle = name.trim().to_lowercase();
    if needle.is_empty() { return Err("Nome file vuoto.".into()); }
    let mut matches = Vec::new();
    for root in allowed_roots()? {
        if !root.exists() { continue; }
        for entry in walkdir::WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok) {
            if matches.len() >= max_results { break; }
            if entry.file_name().to_string_lossy().to_lowercase().contains(&needle) {
                matches.push(entry.path().to_string_lossy().to_string());
            }
        }
        if matches.len() >= max_results { break; }
    }
    Ok(json!({"matches": matches}))
}

fn list_directory(path: &str) -> Result<Value, String> {
    let path = PathBuf::from(path);
    if !path.is_dir() || !is_allowed_path(&path)? { return Err("Cartella non consentita.".into()); }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        entries.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "path": entry.path().to_string_lossy(),
            "directory": entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
        }));
        if entries.len() >= 200 { break; }
    }
    Ok(json!({"entries": entries}))
}

fn is_mutating(name: &str) -> bool {
    matches!(name, "create_text_file" | "rename_file" | "move_file" | "copy_file" | "delete_file")
}

fn execute_tool(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "open_path" => {
            let path = args.get("path").and_then(Value::as_str).ok_or("path mancante")?;
            open_path_or_url(path)?;
            Ok(json!({"ok":true,"opened":path}))
        }
        "open_url" => {
            let url = args.get("url").and_then(Value::as_str).ok_or("url mancante")?;
            if !(url.starts_with("https://") || url.starts_with("http://")) { return Err("Sono consentiti solo URL http/https.".into()); }
            open_path_or_url(url)?;
            Ok(json!({"ok":true,"opened":url}))
        }
        "web_search" => {
            let query = args.get("query").and_then(Value::as_str).ok_or("query mancante")?;
            web_search(query)?;
            Ok(json!({"ok":true,"query":query}))
        }
        "find_file" => {
            let name = args.get("name").and_then(Value::as_str).ok_or("name mancante")?;
            let max = args.get("max_results").and_then(Value::as_u64).unwrap_or(10).clamp(1, 50) as usize;
            find_file(name, max)
        }
        "list_directory" => {
            let path = args.get("path").and_then(Value::as_str).ok_or("path mancante")?;
            list_directory(path)
        }
        "create_text_file" => {
            let path = PathBuf::from(args.get("path").and_then(Value::as_str).ok_or("path mancante")?);
            if !is_allowed_path(&path)? { return Err("Percorso non consentito.".into()); }
            let content = args.get("content").and_then(Value::as_str).ok_or("content mancante")?;
            if path.exists() { return Err("Il file esiste già.".into()); }
            std::fs::write(&path, content).map_err(|e| e.to_string())?;
            Ok(json!({"ok":true,"created":path.to_string_lossy()}))
        }
        "rename_file" => {
            let path = PathBuf::from(args.get("path").and_then(Value::as_str).ok_or("path mancante")?);
            let new_name = args.get("new_name").and_then(Value::as_str).ok_or("new_name mancante")?;
            if !is_allowed_path(&path)? { return Err("Percorso non consentito.".into()); }
            let destination = path.parent().ok_or("Cartella padre non disponibile.")?.join(new_name);
            if !is_allowed_path(&destination)? { return Err("Destinazione non consentita.".into()); }
            std::fs::rename(&path, &destination).map_err(|e| e.to_string())?;
            Ok(json!({"ok":true,"renamed_to":destination.to_string_lossy()}))
        }
        "move_file" | "copy_file" => {
            let source = PathBuf::from(args.get("source").and_then(Value::as_str).ok_or("source mancante")?);
            let destination = PathBuf::from(args.get("destination").and_then(Value::as_str).ok_or("destination mancante")?);
            if !is_allowed_path(&source)? || !is_allowed_path(&destination)? { return Err("Percorso non consentito.".into()); }
            if name == "move_file" {
                std::fs::rename(&source, &destination).map_err(|e| e.to_string())?;
            } else {
                if source.is_dir() { return Err("La copia di cartelle non è ancora supportata.".into()); }
                std::fs::copy(&source, &destination).map_err(|e| e.to_string())?;
            }
            Ok(json!({"ok":true,"source":source.to_string_lossy(),"destination":destination.to_string_lossy()}))
        }
        "delete_file" => {
            let path = PathBuf::from(args.get("path").and_then(Value::as_str).ok_or("path mancante")?);
            if !is_allowed_path(&path)? || !path.exists() { return Err("Percorso non consentito o inesistente.".into()); }
            if path.is_dir() { std::fs::remove_dir(&path).map_err(|e| e.to_string())?; }
            else { std::fs::remove_file(&path).map_err(|e| e.to_string())?; }
            Ok(json!({"ok":true,"deleted":path.to_string_lossy()}))
        }
        _ => Err(format!("Tool non autorizzato: {name}"))
    }
}

async fn response_request(client: &reqwest::Client, key: &str, body: Value) -> Result<Value, String> {
    let response = client.post("https://api.openai.com/v1/responses").bearer_auth(key).json(&body).send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    let value: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(value.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or("OpenAI API error").to_string());
    }
    Ok(value)
}

fn response_text(response: &Value) -> Option<String> {
    let mut parts = Vec::new();
    for item in response.get("output")?.as_array()? {
        if item.get("type").and_then(Value::as_str) != Some("message") { continue; }
        if let Some(content) = item.get("content").and_then(Value::as_array()) {
            for part in content {
                if let Some(text) = part.get("text").and_then(Value::as_str) { parts.push(text.to_string()); }
            }
        }
    }
    if parts.is_empty() { None } else { Some(parts.join("\n")) }
}

async fn send_tool_result(client: &reqwest::Client, key: &str, response_id: &str, call_id: &str, result: Value) -> Result<Value, String> {
    response_request(client, key, json!({
        "model": model_name(),
        "previous_response_id": response_id,
        "input": [{"type":"function_call_output","call_id":call_id,"output":result.to_string()}],
        "tools": tool_definitions(),
        "instructions": "Sei un assistente vocale Windows. Rispondi in italiano salvo richiesta diversa e descrivi solo azioni effettivamente eseguite."
    })).await
}

async fn run_agent(message: String) -> Result<String, String> {
    let key = openai_key()?;
    let client = reqwest::Client::new();
    let previous = last_response().lock().unwrap().clone();
    let mut body = json!({
      "model": model_name(),
      "store": true,
      "instructions": "Sei l'assistente vocale personale dell'utente su Windows. Rispondi in italiano salvo richiesta diversa. Puoi agire sul PC solo tramite gli strumenti locali dichiarati. Non affermare mai di aver fatto qualcosa se il tool non l'ha eseguito. Mantieni le risposte vocali brevi. Le azioni che modificano o cancellano dati richiedono una conferma esplicita prima dell'esecuzione.",
      "input": [{"role":"user","content":[{"type":"input_text","text":message}]}],
      "tools": tool_definitions()
    });
    if let Some(previous_response_id) = previous { body["previous_response_id"] = json!(previous_response_id); }

    for _ in 0..8 {
        let response = response_request(&client, &key, body).await?;
        if let Some(text) = response_text(&response) {
            *last_response().lock().unwrap() = response.get("id").and_then(Value::as_str).map(str::to_owned);
            return Ok(text);
        }

        let mut calls = Vec::new();
        if let Some(output) = response.get("output").and_then(Value::as_array) {
            for item in output {
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    let name = item.get("name").and_then(Value::as_str).ok_or("function_call senza nome")?;
                    let call_id = item.get("call_id").and_then(Value::as_str).ok_or("function_call senza call_id")?;
                    let args: Value = serde_json::from_str(item.get("arguments").and_then(Value::as_str).unwrap_or("{}")).map_err(|e| e.to_string())?;
                    calls.push((name.to_string(), call_id.to_string(), args));
                }
            }
        }
        if calls.is_empty() { return Ok("Operazione completata.".into()); }

        for (name, call_id, args) in calls {
            let response_id = response.get("id").and_then(Value::as_str).ok_or("Response senza id")?.to_string();
            if is_mutating(&name) {
                *pending_action().lock().unwrap() = Some(PendingAction { response_id, call_id, name: name.clone(), args: args.clone() });
                return Ok(format!("Devo confermare l'azione {} prima di eseguirla.", human_tool_name(&name)));
            }
            let result = execute_tool(&name, &args).unwrap_or_else(|e| json!({"ok":false,"error":e}));
            let next = send_tool_result(&client, &key, &response_id, &call_id, result).await?;
            if let Some(text) = response_text(&next) {
                *last_response().lock().unwrap() = next.get("id").and_then(Value::as_str).map(str::to_owned);
                return Ok(text);
            }
            body = json!({
                "model": model_name(),
                "previous_response_id": next.get("id").and_then(Value::as_str).ok_or("Response senza id")?,
                "input": [{"role":"user","content":[{"type":"input_text","text":"Continua l'azione richiesta e restituisci una risposta breve."}]}],
                "tools": tool_definitions()
            });
        }
    }
    Err("Ho raggiunto il limite di passaggi dell'agente.".into())
}

fn human_tool_name(name: &str) -> &'static str {
    match name {
        "create_text_file" => "creazione del file",
        "rename_file" => "rinomina",
        "move_file" => "spostamento",
        "copy_file" => "copia",
        "delete_file" => "eliminazione",
        _ => "modifica locale"
    }
}

#[tauri::command]
async fn agent_message(message: String) -> Result<String, String> {
    run_agent(message).await
}

#[tauri::command]
async fn confirm_pending(confirmed: bool) -> Result<String, String> {
    let pending = pending_action().lock().unwrap().take().ok_or("Nessuna azione in attesa.")?;
    if !confirmed { return Ok("Azione annullata.".into()); }
    let key = openai_key()?;
    let client = reqwest::Client::new();
    let result = execute_tool(&pending.name, &pending.args).unwrap_or_else(|e| json!({"ok":false,"error":e}));
    let response = send_tool_result(&client, &key, &pending.response_id, &pending.call_id, result).await?;
    if let Some(text) = response_text(&response) {
        *last_response().lock().unwrap() = response.get("id").and_then(Value::as_str).map(str::to_owned);
        Ok(text)
    } else {
        Ok("Operazione completata.".into())
    }
}

#[tauri::command]
fn clear_conversation() {
    *last_response().lock().unwrap() = None;
    *pending_action().lock().unwrap() = None;
}

#[tauri::command]
fn app_info() -> Value {
    json!({"name":"Windows IA Assistant","version":env!("CARGO_PKG_VERSION"),"model":model_name(),"voice_mode":"local wake-word adapter + cloud agent","source_foundation":"Coucou Windows/Tauri"})
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::Windows, None))
        .invoke_handler(tauri::generate_handler![agent_message, confirm_pending, save_openai_api_key, clear_conversation, app_info])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Windows IA Assistant");
}
