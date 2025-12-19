use tauri::Manager;
use tauri::AppHandle;
use std::thread;
use std::time::Duration;
use arboard::Clipboard;
use regex::Regex;
use crate::db::ClipboardDB;

pub fn start_listener(app: AppHandle) {
    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to initialize clipboard: {}", e);
                return;
            }
        };

        let mut last_content = String::new();

        // Regex patterns for categorization
        // --- Dev / Infra ---
let docker_re      = Regex::new(r"(?m)^(docker|docker-compose)\s").unwrap();
let kubectl_re     = Regex::new(r"(?m)^kubectl\s").unwrap();
let helm_re        = Regex::new(r"(?m)^helm\s").unwrap();
let terraform_re   = Regex::new(r"(?m)^terraform\s").unwrap();
let ansible_re     = Regex::new(r"(?m)^ansible(-playbook)?\s").unwrap();
let aws_re         = Regex::new(r"(?m)^aws\s").unwrap();
let gcloud_re      = Regex::new(r"(?m)^gcloud\s").unwrap();
let az_re          = Regex::new(r"(?m)^az\s").unwrap();

// --- Version Control ---
let git_re         = Regex::new(r"(?m)^git\s").unwrap();
let gh_re          = Regex::new(r"(?m)^gh\s").unwrap();
let svn_re         = Regex::new(r"(?m)^svn\s").unwrap();

// --- Package Managers ---
let npm_re         = Regex::new(r"(?m)^npm\s").unwrap();
let npx_re         = Regex::new(r"(?m)^npx\s").unwrap();
let yarn_re        = Regex::new(r"(?m)^yarn\s").unwrap();
let pnpm_re        = Regex::new(r"(?m)^pnpm\s").unwrap();
let pip_re         = Regex::new(r"(?m)^(pip|pip3)\s").unwrap();
let poetry_re      = Regex::new(r"(?m)^poetry\s").unwrap();
let cargo_re       = Regex::new(r"(?m)^cargo\s").unwrap();
let go_mod_re      = Regex::new(r"(?m)^go\s(mod|get|build|run)\b").unwrap();
let brew_re        = Regex::new(r"(?m)^brew\s").unwrap();
let apt_re         = Regex::new(r"(?m)^(apt|apt-get)\s").unwrap();
let yum_re         = Regex::new(r"(?m)^(yum|dnf)\s").unwrap();

// --- Programming / Runtime ---
let node_re        = Regex::new(r"(?m)^node\s").unwrap();
let python_re      = Regex::new(r"(?m)^(python|python3)\s").unwrap();
let java_re        = Regex::new(r"(?m)^java\s").unwrap();
let mvn_re         = Regex::new(r"(?m)^mvn\s").unwrap();
let gradle_re      = Regex::new(r"(?m)^gradle\s").unwrap();
let dotnet_re      = Regex::new(r"(?m)^dotnet\s").unwrap();
let rustc_re       = Regex::new(r"(?m)^rustc\s").unwrap();

// --- OS / Shell ---
let bash_re        = Regex::new(r"(?m)^(cd|ls|pwd|cp|mv|rm|cat|less|grep|find|chmod|chown)\b").unwrap();
let zsh_re         = Regex::new(r"(?m)^zsh\s").unwrap();
let powershell_re  = Regex::new(r"(?m)^(Get-|Set-|New-|Remove-)\w+").unwrap();

// --- Networking / Debug ---
let curl_re        = Regex::new(r"(?m)^curl\s").unwrap();
let wget_re        = Regex::new(r"(?m)^wget\s").unwrap();
let httpie_re      = Regex::new(r"(?m)^http\s").unwrap();
let ping_re        = Regex::new(r"(?m)^ping\s").unwrap();
let netstat_re     = Regex::new(r"(?m)^(netstat|ss)\s").unwrap();
let lsof_re        = Regex::new(r"(?m)^lsof\s").unwrap();

// --- Databases ---
let psql_re        = Regex::new(r"(?m)^psql\s").unwrap();
let mysql_re       = Regex::new(r"(?m)^mysql\s").unwrap();
let redis_re       = Regex::new(r"(?m)^redis-cli\s").unwrap();
let mongo_re       = Regex::new(r"(?m)^mongo\s").unwrap();
let sqlite_re      = Regex::new(r"(?m)^sqlite3\s").unwrap();

// --- CI / Build ---
let make_re        = Regex::new(r"(?m)^make\s").unwrap();
let cmake_re       = Regex::new(r"(?m)^cmake\s").unwrap();
let bazel_re       = Regex::new(r"(?m)^bazel\s").unwrap();
let github_actions_re = Regex::new(r"(?m)^\s*(uses:|runs-on:|steps:)").unwrap();

        // Add more as needed

        loop {
            // Sleep first to avoid tight loop on start
            thread::sleep(Duration::from_millis(500));

            match clipboard.get_text() {
                Ok(text) => {
                    // Deduplication (Simple: check against last memory item)
                    // Enhancement: Check DB if last_content is empty (restart case)?
                    // For now, memory check is good for runtime loop.
                    if text != last_content && !text.trim().is_empty() {
                         // Length check constraint (50MB) - 50 * 1024 * 1024 chars approx
                         if text.len() > 50 * 1024 * 1024 {
                             eprintln!("Clipboard content too large, ignoring.");
                             last_content = text;
                             continue;
                         }

                        let mut category = if docker_re.is_match(&text) {
    Some("Docker".into())
} else if kubectl_re.is_match(&text) || helm_re.is_match(&text) {
    Some("Kubernetes".into())
} else if terraform_re.is_match(&text) || ansible_re.is_match(&text) {
    Some("IaC".into())
} else if aws_re.is_match(&text) || gcloud_re.is_match(&text) || az_re.is_match(&text) {
    Some("Cloud CLI".into())
} else if git_re.is_match(&text) || gh_re.is_match(&text) || svn_re.is_match(&text) {
    Some("Version Control".into())
} else if npm_re.is_match(&text)
    || npx_re.is_match(&text)
    || yarn_re.is_match(&text)
    || pnpm_re.is_match(&text)
    || pip_re.is_match(&text)
    || poetry_re.is_match(&text)
    || cargo_re.is_match(&text)
    || go_mod_re.is_match(&text)
    || brew_re.is_match(&text)
    || apt_re.is_match(&text)
    || yum_re.is_match(&text)
{
    Some("Package Management".into())
} else if node_re.is_match(&text)
    || python_re.is_match(&text)
    || java_re.is_match(&text)
    || mvn_re.is_match(&text)
    || gradle_re.is_match(&text)
    || dotnet_re.is_match(&text)
    || rustc_re.is_match(&text)
{
    Some("Runtime / Build".into())
} else if bash_re.is_match(&text) || zsh_re.is_match(&text) || powershell_re.is_match(&text) {
    Some("Shell / OS".into())
} else if curl_re.is_match(&text)
    || wget_re.is_match(&text)
    || httpie_re.is_match(&text)
    || ping_re.is_match(&text)
    || netstat_re.is_match(&text)
    || lsof_re.is_match(&text)
{
    Some("Networking".into())
} else if psql_re.is_match(&text)
    || mysql_re.is_match(&text)
    || redis_re.is_match(&text)
    || mongo_re.is_match(&text)
    || sqlite_re.is_match(&text)
{
    Some("Database".into())
} else if make_re.is_match(&text)
    || cmake_re.is_match(&text)
    || bazel_re.is_match(&text)
    || github_actions_re.is_match(&text)
{
    Some("CI / Build".into())
} else {
    None
};


                        // Access DB state
                        if let Some(db) = app.try_state::<ClipboardDB>() {
                             // If no regex match, try similarity match
                             if category.is_none() {
                                 if let Ok(Some(sim_cat)) = db.find_similar_category(&text) {
                                     category = Some(sim_cat);
                                 }
                             }

                             if let Err(e) = db.insert_item(text.clone(), category) {
                                 eprintln!("Failed to save clipboard item: {}", e);
                             }
                        }

                        last_content = text;
                    }
                }
                Err(_) => {
                    // Ignore errors (e.g. if clipboard is locked or non-text)
                }
            }
        }
    });
}
