use crate::db::ClipboardDB;
use arboard::Clipboard;
use regex::Regex;
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

        let docker_re = Regex::new(r"(?m)^(docker|docker-compose)\s").unwrap();
        let kubectl_re = Regex::new(r"(?m)^kubectl\s").unwrap();
        let helm_re = Regex::new(r"(?m)^helm\s").unwrap();
        let terraform_re = Regex::new(r"(?m)^terraform\s").unwrap();
        let ansible_re = Regex::new(r"(?m)^ansible(-playbook)?\s").unwrap();
        let aws_re = Regex::new(r"(?m)^aws\s").unwrap();
        let gcloud_re = Regex::new(r"(?m)^gcloud\s").unwrap();
        let az_re = Regex::new(r"(?m)^az\s").unwrap();
        let git_re = Regex::new(r"(?m)^git\s").unwrap();
        let gh_re = Regex::new(r"(?m)^gh\s").unwrap();
        let svn_re = Regex::new(r"(?m)^svn\s").unwrap();
        let npm_re = Regex::new(r"(?m)^npm\s").unwrap();
        let npx_re = Regex::new(r"(?m)^npx\s").unwrap();
        let yarn_re = Regex::new(r"(?m)^yarn\s").unwrap();
        let pnpm_re = Regex::new(r"(?m)^pnpm\s").unwrap();
        let pip_re = Regex::new(r"(?m)^(pip|pip3)\s").unwrap();
        let poetry_re = Regex::new(r"(?m)^poetry\s").unwrap();
        let cargo_re = Regex::new(r"(?m)^cargo\s").unwrap();
        let go_mod_re = Regex::new(r"(?m)^go\s(mod|get|build|run)\b").unwrap();
        let brew_re = Regex::new(r"(?m)^brew\s").unwrap();
        let apt_re = Regex::new(r"(?m)^(apt|apt-get)\s").unwrap();
        let yum_re = Regex::new(r"(?m)^(yum|dnf)\s").unwrap();
        let node_re = Regex::new(r"(?m)^node\s").unwrap();
        let python_re = Regex::new(r"(?m)^(python|python3)\s").unwrap();
        let java_re = Regex::new(r"(?m)^java\s").unwrap();
        let mvn_re = Regex::new(r"(?m)^mvn\s").unwrap();
        let gradle_re = Regex::new(r"(?m)^gradle\s").unwrap();
        let dotnet_re = Regex::new(r"(?m)^dotnet\s").unwrap();
        let rustc_re = Regex::new(r"(?m)^rustc\s").unwrap();
        let bash_re =
            Regex::new(r"(?m)^(cd|ls|pwd|cp|mv|rm|cat|less|grep|find|chmod|chown)\b").unwrap();
        let zsh_re = Regex::new(r"(?m)^zsh\s").unwrap();
        let powershell_re = Regex::new(r"(?m)^(Get-|Set-|New-|Remove-)\w+").unwrap();
        let curl_re = Regex::new(r"(?m)^curl\s").unwrap();
        let wget_re = Regex::new(r"(?m)^wget\s").unwrap();
        let httpie_re = Regex::new(r"(?m)^http\s").unwrap();
        let ping_re = Regex::new(r"(?m)^ping\s").unwrap();
        let netstat_re = Regex::new(r"(?m)^(netstat|ss)\s").unwrap();
        let lsof_re = Regex::new(r"(?m)^lsof\s").unwrap();
        let psql_re = Regex::new(r"(?m)^psql\s").unwrap();
        let mysql_re = Regex::new(r"(?m)^mysql\s").unwrap();
        let redis_re = Regex::new(r"(?m)^redis-cli\s").unwrap();
        let mongo_re = Regex::new(r"(?m)^mongo\s").unwrap();
        let sqlite_re = Regex::new(r"(?m)^sqlite3\s").unwrap();
        let make_re = Regex::new(r"(?m)^make\s").unwrap();
        let cmake_re = Regex::new(r"(?m)^cmake\s").unwrap();
        let bazel_re = Regex::new(r"(?m)^bazel\s").unwrap();
        let github_actions_re = Regex::new(r"(?m)^\s*(uses:|runs-on:|steps:)").unwrap();
        let url_re = Regex::new(r"(?i)\b(https?|ftp)://[^\s/$.?#].[^\s]*").unwrap();
        let email_re = Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap();
        let json_re = Regex::new(r"(?s)^\s*[\{\[][\s\S]*[\}\]]\s*$").unwrap();
        let xml_re = Regex::new(r"(?i)^\s*<\?xml|^\s*<([a-z][\w-]*)(?:\s[^>]*)?>").unwrap();
        let sql_re =
            Regex::new(r"(?i)^\s*(select|insert|update|delete|create|alter|drop)\b").unwrap();
        let windows_path_re = Regex::new(r"^[a-zA-Z]:\\").unwrap();
        let unix_path_re = Regex::new(r"^(/[^/]+)+/?$").unwrap();
        let code_block_re = Regex::new(r"(?s)```[\s\S]*```").unwrap();
        let shell_op_re = Regex::new(r"(\|\||&&|>>|<<|;\s)").unwrap();

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

            if docker_re.is_match(&normalized) {
                add_score(&mut scores, "Docker", 0.98);
                add_score(&mut scores, "DevOps", 0.92);
            }
            if kubectl_re.is_match(&normalized) || helm_re.is_match(&normalized) {
                add_score(&mut scores, "Kubernetes", 0.98);
                add_score(&mut scores, "DevOps", 0.92);
            }
            if terraform_re.is_match(&normalized) || ansible_re.is_match(&normalized) {
                add_score(&mut scores, "IaC", 0.95);
                add_score(&mut scores, "DevOps", 0.9);
            }
            if aws_re.is_match(&normalized)
                || gcloud_re.is_match(&normalized)
                || az_re.is_match(&normalized)
            {
                add_score(&mut scores, "Cloud CLI", 0.93);
                add_score(&mut scores, "DevOps", 0.88);
            }
            if git_re.is_match(&normalized) || gh_re.is_match(&normalized) || svn_re.is_match(&normalized)
            {
                add_score(&mut scores, "Version Control", 0.97);
            }
            if npm_re.is_match(&normalized)
                || npx_re.is_match(&normalized)
                || yarn_re.is_match(&normalized)
                || pnpm_re.is_match(&normalized)
                || pip_re.is_match(&normalized)
                || poetry_re.is_match(&normalized)
                || cargo_re.is_match(&normalized)
                || go_mod_re.is_match(&normalized)
                || brew_re.is_match(&normalized)
                || apt_re.is_match(&normalized)
                || yum_re.is_match(&normalized)
            {
                add_score(&mut scores, "Package Management", 0.9);
            }
            if node_re.is_match(&normalized)
                || python_re.is_match(&normalized)
                || java_re.is_match(&normalized)
                || mvn_re.is_match(&normalized)
                || gradle_re.is_match(&normalized)
                || dotnet_re.is_match(&normalized)
                || rustc_re.is_match(&normalized)
            {
                add_score(&mut scores, "Runtime / Build", 0.84);
            }
            if bash_re.is_match(&normalized)
                || zsh_re.is_match(&normalized)
                || powershell_re.is_match(&normalized)
                || shell_op_re.is_match(&normalized)
            {
                add_score(&mut scores, "Shell / OS", 0.82);
            }
            if curl_re.is_match(&normalized)
                || wget_re.is_match(&normalized)
                || httpie_re.is_match(&normalized)
                || ping_re.is_match(&normalized)
                || netstat_re.is_match(&normalized)
                || lsof_re.is_match(&normalized)
            {
                add_score(&mut scores, "Networking", 0.86);
            }
            if psql_re.is_match(&normalized)
                || mysql_re.is_match(&normalized)
                || redis_re.is_match(&normalized)
                || mongo_re.is_match(&normalized)
                || sqlite_re.is_match(&normalized)
                || sql_re.is_match(&normalized)
            {
                add_score(&mut scores, "Database", 0.9);
            }
            if make_re.is_match(&normalized)
                || cmake_re.is_match(&normalized)
                || bazel_re.is_match(&normalized)
                || github_actions_re.is_match(&normalized)
            {
                add_score(&mut scores, "CI / Build", 0.88);
            }
            if url_re.is_match(&normalized) {
                add_score(&mut scores, "URL", 0.97);
                add_score(&mut scores, "Web", 0.9);
            }
            if email_re.is_match(&normalized) {
                add_score(&mut scores, "Email", 0.9);
            }
            if json_re.is_match(&normalized) {
                add_score(&mut scores, "JSON", 0.92);
            }
            if xml_re.is_match(&normalized) {
                add_score(&mut scores, "XML", 0.88);
            }
            if windows_path_re.is_match(&normalized) || unix_path_re.is_match(&normalized) {
                add_score(&mut scores, "Path", 0.86);
            }
            if code_block_re.is_match(&normalized)
                || normalized.contains("function ")
                || normalized.contains("class ")
                || normalized.contains("=>")
            {
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
