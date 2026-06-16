use crate::db::ClipboardDB;
use arboard::Clipboard;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Cursor;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;

/// Classifier groups that indicate the content is a credential/secret.
const SENSITIVE_GROUPS: &[&str] = &["Security", "Secret / Key", "JWT / Token", "SSH / Certificates"];

fn scores_are_sensitive(scores: &HashMap<String, f32>) -> bool {
    SENSITIVE_GROUPS.iter().any(|g| scores.contains_key(*g))
}

/// SHA-256 of bytes as lowercase hex; used to content-address images.
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Encodes a clipboard image to PNG (+ thumbnail), stores it in the blob table,
/// and records a history row referencing it by hash.
fn store_image(db: &ClipboardDB, img: arboard::ImageData, hash: &str) -> Result<(), String> {
    let width = img.width as u32;
    let height = img.height as u32;
    let rgba = image::RgbaImage::from_raw(width, height, img.bytes.into_owned())
        .ok_or_else(|| "invalid image buffer".to_string())?;
    let dynimg = image::DynamicImage::ImageRgba8(rgba);

    let mut png = Vec::new();
    dynimg
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let mut thumb = Vec::new();
    dynimg
        .thumbnail(240, 240)
        .write_to(&mut Cursor::new(&mut thumb), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    db.insert_blob(hash, "image/png", &png, Some(&thumb)).map_err(|e| e.to_string())?;
    db.insert_auto_grouped_content("image", hash.to_string(), vec![("Images".to_string(), 1.0)], false)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Captures an image from the clipboard when no text/files are present.
fn try_capture_image(app: &AppHandle, last_signature: &mut String, clipboard: &mut Clipboard) -> bool {
    let img = match clipboard.get_image() {
        Ok(i) => i,
        Err(_) => return false,
    };
    if img.bytes.is_empty() || img.bytes.len() > 40 * 1024 * 1024 {
        return false;
    }
    let hash = sha256_hex(&img.bytes);
    let signature = format!("image:{}", hash);
    if signature == *last_signature {
        return false;
    }
    if let Some(db) = app.try_state::<ClipboardDB>() {
        if store_image(db.inner(), img, &hash).is_ok() {
            *last_signature = signature;
            let _ = app.emit("clipboard-updated", ());
            return true;
        }
    }
    false
}

/// Captures a file selection from the clipboard (macOS). Returns true when file
/// paths are present on the clipboard, so the caller skips text/image handling.
fn try_capture_files(app: &AppHandle, last_signature: &mut String) -> bool {
    let paths = match crate::read_clipboard_file_paths() {
        Some(p) if !p.is_empty() => p,
        _ => return false,
    };
    let signature = format!("files:{}", paths.join("\u{0}"));
    if signature != *last_signature {
        if let Some(db) = app.try_state::<ClipboardDB>() {
            let json = serde_json::to_string(&paths).unwrap_or_default();
            if !json.is_empty()
                && db
                    .insert_auto_grouped_content("files", json, vec![("Files".to_string(), 1.0)], false)
                    .is_ok()
            {
                *last_signature = signature;
                let _ = app.emit("clipboard-updated", ());
            }
        }
    }
    true
}

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

fn starts_with_any(token: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| token == *prefix)
}

// ── Structural / format detectors ──────────────────────────────────────────────

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
            && local.len() >= 1
            && domain.len() >= 3
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
    // Require a closing tag to reduce false positives from HTML fragments
    trimmed.starts_with("<?xml")
        || (trimmed.starts_with('<') && trimmed.contains("</") && trimmed.contains('>'))
}

fn looks_like_yaml(text: &str) -> bool {
    if text.len() < 10 {
        return false;
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 3 {
        return false;
    }
    let kv_lines = lines
        .iter()
        .filter(|l| {
            let s = l.trim();
            !s.is_empty()
                && !s.starts_with('#')
                && (s.contains(": ") || s.ends_with(':') || s.starts_with("- "))
        })
        .count();
    let has_structure = text.contains("---") || text.contains("- ") || text.contains("  ");
    kv_lines >= 3 && has_structure && kv_lines * 10 >= lines.len() * 5
}

fn looks_like_csv(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return false;
    }
    let sample = &lines[..lines.len().min(5)];
    let comma_counts: Vec<usize> = sample.iter().map(|l| l.matches(',').count()).collect();
    if comma_counts[0] < 2 {
        return false;
    }
    let consistent = comma_counts.iter().filter(|&&c| c == comma_counts[0]).count();
    consistent * 2 >= sample.len()
}

fn looks_like_markdown(text: &str) -> bool {
    let signals = [
        text.contains("\n# ") || text.starts_with("# "),
        text.contains("\n## ") || text.starts_with("## "),
        text.contains("\n- ") || text.contains("\n* "),
        text.contains("**") || text.contains("__"),
        text.contains("```"),
        text.contains("]("),
    ];
    signals.iter().filter(|&&x| x).count() >= 2
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
    (trimmed.starts_with('/') && trimmed.len() > 1)
        || trimmed.starts_with("~/")
        || trimmed.starts_with("../")
        || trimmed.starts_with("./")
}

fn looks_like_uuid(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() != 36 {
        return false;
    }
    let parts: Vec<&str> = trimmed.split('-').collect();
    parts.len() == 5
        && [8usize, 4, 4, 4, 12]
            .iter()
            .zip(&parts)
            .all(|(&len, part)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

fn looks_like_ip_address(text: &str) -> bool {
    // Handles plain IPs and CIDR notation (e.g. 192.168.1.0/24)
    let trimmed = text.trim();
    let addr = trimmed.split('/').next().unwrap_or(trimmed);
    let parts: Vec<&str> = addr.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok())
}

fn looks_like_jwt(text: &str) -> bool {
    let trimmed = text.trim();
    // JWTs always start with "eyJ" (base64 of '{"')
    if !trimmed.starts_with("eyJ") {
        return false;
    }
    let parts: Vec<&str> = trimmed.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| {
            p.len() >= 8
                && p.chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '=')
        })
}

fn looks_like_base64(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 40 || trimmed.contains(' ') || trimmed.contains('\n') {
        return false;
    }
    // Pure hex strings (git hashes, etc.) are not base64
    if trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    let stripped = trimmed.trim_end_matches('=');
    stripped.len() >= 32
        && stripped
            .chars()
            .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '-' || c == '_')
}

fn looks_like_phone_number(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 7 || trimmed.len() > 20 {
        return false;
    }
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 7 || digits.len() > 15 {
        return false;
    }
    // IP addresses have 4 octet groups — skip them
    if looks_like_ip_address(trimmed) {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || " -.+()/".contains(c))
        && (trimmed.starts_with('+')
            || trimmed.starts_with('(')
            || trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false))
}

fn looks_like_env_var(text: &str) -> bool {
    // Matches: KEY=value  or  export KEY=value  (UPPER_SNAKE_CASE keys)
    let trimmed = text.trim();
    let content = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    if let Some(eq_pos) = content.find('=') {
        let key = &content[..eq_pos];
        !key.is_empty()
            && key.len() >= 2
            && key
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            && key
                .chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
    } else {
        false
    }
}

fn looks_like_secret_key(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("sk-")           // OpenAI / Anthropic
        || t.starts_with("sk-ant-")
        || t.starts_with("gh_")    // GitHub fine-grained PAT
        || t.starts_with("ghp_")   // GitHub personal access token
        || t.starts_with("ghs_")
        || t.starts_with("gho_")
        || t.starts_with("AKIA")   // AWS Access Key ID
        || t.starts_with("ASIA")   // AWS temporary credentials
        || t.starts_with("AIza")   // Google API key
        || t.starts_with("xoxb-")  // Slack bot token
        || t.starts_with("xoxp-")  // Slack user token
        || (t.starts_with("Bearer ") && t.len() > 20)
}

fn looks_like_ssh_key(text: &str) -> bool {
    text.starts_with("ssh-rsa ")
        || text.starts_with("ssh-ed25519 ")
        || text.starts_with("ssh-ecdsa ")
        || text.starts_with("ecdsa-sha2-")
        || (text.starts_with("-----BEGIN") && text.contains("PRIVATE KEY-----"))
        || (text.starts_with("-----BEGIN") && text.contains("PUBLIC KEY-----"))
        || (text.starts_with("-----BEGIN") && text.contains("CERTIFICATE-----"))
}

// ── Code detection ─────────────────────────────────────────────────────────────

fn looks_like_code_snippet(text: &str) -> bool {
    if text.contains("```") {
        return true;
    }
    if text.contains("function ") || text.contains("class ") || text.contains("def ") {
        return true;
    }
    if text.contains("#include") || text.contains("using namespace") {
        return true;
    }
    // Arrow functions (JS/TS)
    if text.contains("=>") && (text.contains("const ") || text.contains("let ")) {
        return true;
    }
    // Rust function body
    if text.contains("fn ") && text.contains('{') && text.contains('}') {
        return true;
    }
    // Java / C# method or field
    if (text.contains("public ") || text.contains("private ") || text.contains("protected "))
        && (text.contains("void ") || text.contains("static "))
    {
        return true;
    }
    // ES module imports
    if text.contains("import ")
        && (text.contains(" from '")
            || text.contains(" from \"")
            || text.contains("import {"))
    {
        return true;
    }
    false
}

fn detect_language(text: &str) -> Option<&'static str> {
    // Order matters: check more specific patterns first
    if text.contains(": string")
        || text.contains(": number")
        || text.contains(": boolean")
        || text.contains("interface ")
        || text.contains(": void")
        || text.contains("<T>")
    {
        return Some("TypeScript");
    }
    if (text.contains("def ") && text.contains(':'))
        || text.contains("self.")
        || (text.contains("import ") && text.contains("from "))
        || text.to_ascii_lowercase().contains("print(")
    {
        return Some("Python");
    }
    if (text.contains("fn ") || text.contains("pub fn") || text.contains("let mut"))
        && (text.contains("->") || text.contains("impl ") || text.contains("use "))
    {
        return Some("Rust");
    }
    if text.contains("package ") && text.contains("func ") {
        return Some("Go");
    }
    if text.contains("#include") || text.contains("std::") || text.contains("int main") {
        return Some("C/C++");
    }
    if text.contains("public class") || text.contains("@Override") || text.contains("System.out") {
        return Some("Java");
    }
    if (text.contains("const ") || text.contains("let ") || text.contains("var "))
        && (text.contains("=>") || text.contains("===") || text.contains("!=="))
    {
        return Some("JavaScript");
    }
    None
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

            // 1. File selection (macOS) — handled before text so a Finder copy
            //    isn't mistaken for its text path representation.
            if try_capture_files(&app, &mut last_signature) {
                continue;
            }

            // 2. Text — or fall through to image capture when there is none.
            let text = match clipboard.get_text() {
                Ok(t) if !t.trim().is_empty() => t,
                _ => {
                    try_capture_image(&app, &mut last_signature, &mut clipboard);
                    continue;
                }
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
            let lower = normalized.to_ascii_lowercase();
            let line_count = normalized.lines().count();

            // ── DevOps / Containers ────────────────────────────────────────────
            if starts_with_any(&first, &["docker", "docker-compose"]) {
                add_score(&mut scores, "Docker", 0.98);
                add_score(&mut scores, "DevOps", 0.92);
            }
            // Dockerfile instruction pattern
            if ["FROM ", "RUN ", "COPY ", "ADD ", "ENV ", "EXPOSE ", "CMD ", "ENTRYPOINT "]
                .iter()
                .filter(|kw| normalized.contains(*kw))
                .count()
                >= 2
            {
                add_score(&mut scores, "Docker", 0.88);
                add_score(&mut scores, "DevOps", 0.82);
            }
            if starts_with_any(&first, &["kubectl", "helm"]) {
                add_score(&mut scores, "Kubernetes", 0.98);
                add_score(&mut scores, "DevOps", 0.92);
            }
            if first == "terraform" || first.starts_with("ansible") {
                add_score(&mut scores, "IaC", 0.95);
                add_score(&mut scores, "DevOps", 0.9);
            }
            if starts_with_any(&first, &["aws", "gcloud", "az", "doctl", "flyctl"]) {
                add_score(&mut scores, "Cloud CLI", 0.93);
                add_score(&mut scores, "DevOps", 0.88);
            }

            // ── Version Control ────────────────────────────────────────────────
            if starts_with_any(&first, &["git", "gh", "svn", "hg"]) {
                add_score(&mut scores, "Version Control", 0.97);
            }
            // Git commit hash: 7–12 char short form or 40-char full SHA
            if !normalized.contains(' ')
                && (normalized.len() == 40
                    || (normalized.len() >= 7 && normalized.len() <= 12))
                && normalized.chars().all(|c| c.is_ascii_hexdigit())
            {
                add_score(&mut scores, "Version Control", 0.80);
            }

            // ── Package Management ─────────────────────────────────────────────
            if starts_with_any(
                &first,
                &[
                    "npm", "npx", "yarn", "pnpm", "pip", "pip3", "poetry", "cargo", "brew",
                    "apt", "apt-get", "yum", "dnf", "pacman", "gem", "bundle", "composer",
                    "nuget", "mix", "hex",
                ],
            ) || (first == "go"
                && normalized
                    .split_whitespace()
                    .nth(1)
                    .map(|s| matches!(s, "mod" | "get" | "build" | "run" | "test" | "install"))
                    .unwrap_or(false))
            {
                add_score(&mut scores, "Package Management", 0.9);
            }

            // ── Runtime / Build ────────────────────────────────────────────────
            if starts_with_any(
                &first,
                &[
                    "node", "python", "python3", "java", "mvn", "gradle", "dotnet", "rustc",
                    "deno", "bun", "ruby", "perl", "php", "elixir",
                ],
            ) {
                add_score(&mut scores, "Runtime / Build", 0.84);
            }

            // ── Shell / OS ─────────────────────────────────────────────────────
            if starts_with_any(
                &first,
                &[
                    "cd", "ls", "pwd", "cp", "mv", "rm", "cat", "less", "grep", "find", "chmod",
                    "chown", "zsh", "bash", "sh", "fish", "echo", "export", "source", "env",
                    "printenv", "xargs", "tee", "awk", "sed", "sort", "uniq", "wc", "head",
                    "tail", "touch", "mkdir", "rmdir", "ln", "du", "df", "ps", "kill",
                    "killall", "top", "htop", "screen", "tmux", "nohup", "sudo", "su",
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
                || normalized.contains("$(")
                || normalized.contains("${")
                || normalized.starts_with("#!/")
            {
                add_score(&mut scores, "Shell / OS", 0.82);
            }

            // ── Config / Environment variables ─────────────────────────────────
            if looks_like_env_var(&normalized) {
                add_score(&mut scores, "Config / Env", 0.88);
            }

            // ── Networking ─────────────────────────────────────────────────────
            if starts_with_any(
                &first,
                &[
                    "curl", "wget", "http", "ping", "netstat", "ss", "lsof", "nmap", "dig",
                    "nslookup", "traceroute", "tracert", "ifconfig", "ip", "iptables", "ufw",
                    "nc", "netcat", "socat", "tcpdump", "ssh", "scp", "rsync", "sftp", "ftp",
                ],
            ) {
                add_score(&mut scores, "Networking", 0.86);
            }
            if looks_like_ip_address(&normalized) {
                add_score(&mut scores, "Networking", 0.85);
            }

            // ── SSH / Certificates ─────────────────────────────────────────────
            if looks_like_ssh_key(&normalized) {
                add_score(&mut scores, "SSH / Certificates", 0.96);
                add_score(&mut scores, "Security", 0.85);
            }

            // ── Database ───────────────────────────────────────────────────────
            if starts_with_any(
                &first,
                &["psql", "mysql", "redis-cli", "mongo", "sqlite3", "mongosh"],
            ) {
                add_score(&mut scores, "Database", 0.92);
            }
            {
                let sql_keywords = [
                    "select ", "insert ", "update ", "delete ", "create table",
                    "alter table", "drop table", "truncate ", "from ", "where ",
                    "join ", "having ", "group by", "order by",
                ];
                let matched = sql_keywords
                    .iter()
                    .filter(|kw| lower.contains(*kw))
                    .count();
                if matched >= 3 {
                    add_score(&mut scores, "Database", 0.92);
                } else if matched == 1 || matched == 2 {
                    add_score(&mut scores, "Database", 0.72);
                }
            }

            // ── CI / Build ─────────────────────────────────────────────────────
            if starts_with_any(&first, &["make", "cmake", "bazel", "meson", "ninja"])
                || normalized.contains("runs-on:")
                || normalized.contains("uses:")
                || normalized.contains("steps:")
                || lower.contains(".github/workflows")
                || lower.contains("pipeline:")
            {
                add_score(&mut scores, "CI / Build", 0.88);
            }

            // ── URL ────────────────────────────────────────────────────────────
            if looks_like_url(&normalized) {
                add_score(&mut scores, "URL", 0.97);
                add_score(&mut scores, "Web", 0.9);
            }

            // ── Email ──────────────────────────────────────────────────────────
            if looks_like_email(&normalized) {
                add_score(&mut scores, "Email", 0.90);
            }

            // ── Structured data formats ────────────────────────────────────────
            if looks_like_json(&normalized) {
                add_score(&mut scores, "JSON", 0.92);
            }
            if looks_like_xml(&normalized) {
                add_score(&mut scores, "XML", 0.88);
            }
            if looks_like_yaml(&normalized) {
                add_score(&mut scores, "YAML", 0.87);
            }
            if looks_like_csv(&normalized) {
                add_score(&mut scores, "CSV", 0.84);
            }
            if looks_like_markdown(&normalized) {
                add_score(&mut scores, "Markdown", 0.85);
            }

            // ── File paths ─────────────────────────────────────────────────────
            if looks_like_windows_path(&normalized) || looks_like_unix_path(&normalized) {
                add_score(&mut scores, "Path", 0.86);
            }

            // ── Code snippets ──────────────────────────────────────────────────
            if looks_like_code_snippet(&normalized) {
                add_score(&mut scores, "Code Snippet", 0.82);
                if let Some(lang) = detect_language(&normalized) {
                    add_score(&mut scores, lang, 0.85);
                    // Strengthen "Code Snippet" when we can confirm the language
                    add_score(&mut scores, "Code Snippet", 0.85);
                }
            }
            // Multi-line indented block heuristic
            if line_count >= 3 {
                let indented = normalized
                    .lines()
                    .filter(|l| l.starts_with("    ") || l.starts_with('\t'))
                    .count();
                if indented >= 2 && indented * 2 >= line_count {
                    add_score(&mut scores, "Code Snippet", 0.74);
                }
            }

            // ── UUIDs ──────────────────────────────────────────────────────────
            if looks_like_uuid(&normalized) {
                add_score(&mut scores, "UUID", 0.95);
            }

            // ── Auth / Security ────────────────────────────────────────────────
            if looks_like_jwt(&normalized) {
                add_score(&mut scores, "JWT / Token", 0.92);
                add_score(&mut scores, "Security", 0.82);
            }
            if looks_like_secret_key(&normalized) {
                add_score(&mut scores, "Secret / Key", 0.90);
                add_score(&mut scores, "Security", 0.82);
            }

            // ── Contact info ───────────────────────────────────────────────────
            if looks_like_phone_number(&normalized) {
                add_score(&mut scores, "Phone Number", 0.82);
            }

            // ── Encoded data (low priority — only if nothing else matched) ─────
            if scores.is_empty() && looks_like_base64(&normalized) {
                add_score(&mut scores, "Base64", 0.72);
            }

            // ── Fallback ───────────────────────────────────────────────────────
            if scores.is_empty() {
                add_score(&mut scores, "Text", 0.4);
            }

            if let Some(db) = app.try_state::<ClipboardDB>() {
                if scores.len() == 1 && scores.get("Text").is_some() {
                    if let Ok(Some(sim_cat)) = db.find_similar_category(&normalized) {
                        add_score(&mut scores, &sim_cat, 0.45);
                    }
                }

                // Optional auto-masking: when enabled, secrets detected by the
                // classifier are stored encrypted + masked instead of plaintext.
                let looks_sensitive = scores_are_sensitive(&scores);
                let auto_mask = looks_sensitive
                    && db.get_setting("auto_mask_secrets").ok().flatten().as_deref() == Some("1");

                let (content_to_store, is_sensitive) = if auto_mask {
                    match crate::crypto::get_or_create_key(&app)
                        .and_then(|key| crate::crypto::encrypt(&key, &normalized))
                    {
                        Ok(enc) => (enc, true),
                        Err(_) => (normalized, false), // fall back to plaintext on key failure
                    }
                } else {
                    (normalized, false)
                };

                if db
                    .insert_auto_grouped_content(
                        "text",
                        content_to_store,
                        finalize_scores(scores),
                        is_sensitive,
                    )
                    .is_ok()
                {
                    last_signature = signature;
                    let _ = app.emit("clipboard-updated", ());
                }
            }
        }
    });
}
