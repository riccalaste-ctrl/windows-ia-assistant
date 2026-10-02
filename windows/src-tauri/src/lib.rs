use std::process::Command;

#[tauri::command]
fn open_target(target: String) -> Result<(), String> {
    let t = target.trim();
    if t.is_empty() { return Err("Destinazione vuota".into()); }
    Command::new("cmd")
        .args(["/C", "start", "", t])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn web_search(query: String) -> Result<(), String> {
    let q = urlencoding::encode(query.trim());
    let url = format!("https://www.google.com/search?q={}", q);
    Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_secret(key: String, value: String) -> Result<(), String> {
    if key != "openai-api-key" { return Err("Chiave non consentita".into()); }
    keyring::Entry::new("windows-ia-assistant", &key)
        .map_err(|e| e.to_string())?
        .set_password(&value)
        .map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![open_target, web_search, save_secret])
        .run(tauri::generate_context!())
        .expect("error while running Windows IA Assistant");
}