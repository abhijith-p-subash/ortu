use crate::db::ClipboardDB;
use arboard::Clipboard;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;

fn add_score(scores: &mut HashMap<String, f32>, group: &str, score: f32) {
    let entry = scores.entry(group.to_string()).or_insert(0.0);
    if score > *entry {
        *entry = score;
    }
}

fn finalize_scores(scores: HashMap<String, f32>) -> Vec<(String, f32)> {
    let mut items: Vec<(String, f32)> = scores.into_iter().collect();
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    items
}

fn first_token_lowercase(text: &str) -> String {
    text.split_whitespace()
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '(' | '['))
        .to_ascii_lowercase()
}

fn looks_like_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("http://") || lower.contains("https://") || lower.contains("ftp://")
}

fn looks_like_email(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let trimmed = token.trim_matches(|c: char| ",;:()[]{}<>\"'".contains(c));
        let mut parts = trimmed.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        parts.next().is_none()
            && !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    })
}

fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

fn looks_like_xml(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("<?xml") || (trimmed.starts_with('<') && trimmed.contains('>'))
}

fn looks_like_windows_path(text: &str) -> bool {
    let trimmed = text.trim();
    let bytes = trimmed.as_bytes();
    bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn looks_like_unix_path(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with('/') && trimmed.len() > 1
}

fn looks_like_code_snippet(text: &str) -> bool {
    text.contains("```")
        || text.contains("function ")
        || text.contains("class ")
        || text.contains("=>")
}

fn starts_with_any(token: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| token == *prefix)
}

pub fn start_listener(app: AppHandle) {
    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to initialize clipboard: {}", e);
                return;
            }
        };

        let mut last_signature = String::new();

        loop {
            thread::sleep(Duration::from_millis(350));

            let text = match clipboard.get_text() {
                Ok(t) if !t.trim().is_empty() => t,
                _ => continue,
            };
            if text.len() > 50 * 1024 * 1024 {
                continue;
            }

            let normalized = text.trim().to_string();
            let signature = format!("text:{}", normalized);
            if signature == last_signature {
                continue;
            }

            let mut scores: HashMap<String, f32> = HashMap::new();
            let first = first_token_lowercase(&normalized);

            if starts_with_any(&first, &["docker", "docker-compose"]) {
                add_score(&mut scores, "Docker", 0.98);
                add_score(&mut scores, "DevOps", 0.92);
            }
            if starts_with_any(&first, &["kubectl", "helm"]) {
                add_score(&mut scores, "Kubernetes", 0.98);
                add_score(&mut scores, "DevOps", 0.92);
            }
            if first == "terraform" || first.starts_with("ansible") {
                add_score(&mut scores, "IaC", 0.95);
                add_score(&mut scores, "DevOps", 0.9);
            }
            if starts_with_any(&first, &["aws", "gcloud", "az"]) {
                add_score(&mut scores, "Cloud CLI", 0.93);
                add_score(&mut scores, "DevOps", 0.88);
            }
            if starts_with_any(&first, &["git", "gh", "svn"]) {
                add_score(&mut scores, "Version Control", 0.97);
            }
            if starts_with_any(
                &first,
                &[
                    "npm", "npx", "yarn", "pnpm", "pip", "pip3", "poetry", "cargo", "brew",
                    "apt", "apt-get", "yum", "dnf",
                ],
            ) || (first == "go"
                && normalized
                    .split_whitespace()
                    .nth(1)
                    .map(|s| matches!(s, "mod" | "get" | "build" | "run"))
                    .unwrap_or(false))
            {
                add_score(&mut scores, "Package Management", 0.9);
            }
            if starts_with_any(
                &first,
                &[
                    "node", "python", "python3", "java", "mvn", "gradle", "dotnet", "rustc",
                ],
            ) {
                add_score(&mut scores, "Runtime / Build", 0.84);
            }
            if starts_with_any(
                &first,
                &[
                    "cd", "ls", "pwd", "cp", "mv", "rm", "cat", "less", "grep", "find", "chmod",
                    "chown", "zsh",
                ],
            ) || first.starts_with("get-")
                || first.starts_with("set-")
                || first.starts_with("new-")
                || first.starts_with("remove-")
                || normalized.contains("||")
                || normalized.contains("&&")
                || normalized.contains(">>")
                || normalized.contains("<<")
                || normalized.contains("; ")
            {
                add_score(&mut scores, "Shell / OS", 0.82);
            }
            if starts_with_any(&first, &["curl", "wget", "http", "ping", "netstat", "ss", "lsof"])
            {
                add_score(&mut scores, "Networking", 0.86);
            }
            if starts_with_any(
                &first,
                &["psql", "mysql", "redis-cli", "mongo", "sqlite3", "select", "insert", "update", "delete", "create", "alter", "drop"],
            ) {
                add_score(&mut scores, "Database", 0.9);
            }
            if starts_with_any(&first, &["make", "cmake", "bazel"])
                || normalized.contains("runs-on:")
                || normalized.contains("uses:")
                || normalized.contains("steps:")
            {
                add_score(&mut scores, "CI / Build", 0.88);
            }
            if looks_like_url(&normalized) {
                add_score(&mut scores, "URL", 0.97);
                add_score(&mut scores, "Web", 0.9);
            }
            if looks_like_email(&normalized) {
                add_score(&mut scores, "Email", 0.9);
            }
            if looks_like_json(&normalized) {
                add_score(&mut scores, "JSON", 0.92);
            }
            if looks_like_xml(&normalized) {
                add_score(&mut scores, "XML", 0.88);
            }
            if looks_like_windows_path(&normalized) || looks_like_unix_path(&normalized) {
                add_score(&mut scores, "Path", 0.86);
            }
            if looks_like_code_snippet(&normalized) {
                add_score(&mut scores, "Code Snippet", 0.8);
            }
            if scores.is_empty() {
                add_score(&mut scores, "Text", 0.4);
            }

            if let Some(db) = app.try_state::<ClipboardDB>() {
                if scores.len() == 1 && scores.get("Text").is_some() {
                    if let Ok(Some(sim_cat)) = db.find_similar_category(&normalized) {
                        add_score(&mut scores, &sim_cat, 0.45);
                    }
                }
                if db
                    .insert_auto_grouped_content("text", normalized, finalize_scores(scores))
                    .is_ok()
                {
                    last_signature = signature;
                    let _ = app.emit("clipboard-updated", ());
                }
            }
        }
    });
}
