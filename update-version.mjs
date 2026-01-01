
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const newVersion = process.argv[2];

if (!newVersion) {
  console.error("Usage: node update-version.mjs <new_version>");
  process.exit(1);
}

// Regex to validate semver roughly
if (!/^\d+\.\d+\.\d+/.test(newVersion)) {
  console.error("Error: Version must be in format x.y.z (e.g. 1.0.2)");
  process.exit(1);
}

const files = [
  {
    path: path.join(__dirname, 'package.json'),
    type: 'json',
    key: 'version'
  },
  {
    path: path.join(__dirname, 'src-tauri', 'tauri.conf.json'),
    type: 'json',
    key: 'version'
  },
  {
    path: path.join(__dirname, 'src-tauri', 'Cargo.toml'),
    type: 'toml',
    key: 'version'
  }
];

console.log(`Updating version to ${newVersion}...`);

for (const file of files) {
  try {
    const content = fs.readFileSync(file.path, 'utf8');
    let newContent = content;

    if (file.type === 'json') {
      const json = JSON.parse(content);
      // For tauri.conf.json, version is nested under 'version' or 'package.version' depending on structure, 
      // but usually it's top level in v2 config?
      // Let's check the actual file structure. 
      // tauri.conf.json v2: "version": "1.0.1" is at root according to previous ViewFile.
      // package.json: "version" is at root.
      
      json[file.key] = newVersion;
      newContent = JSON.stringify(json, null, 2); // default spacing usually 2
      // Maintain newline at end if exists? JSON.stringify doesn't add it.
    } else if (file.type === 'toml') {
      // Simple regex replace for TOML to avoid parsing dependency issues
      // Looking for: version = "1.0.0"
      // We want to replace the FIRST occurrence which is usually the package version.
      // Or specifically under [package] section.
      const versionRegex = /^version\s*=\s*"[^"]+"/m; 
      if (versionRegex.test(content)) {
          newContent = content.replace(versionRegex, `version = "${newVersion}"`);
      } else {
          console.warn(`Could not find version key in ${file.path}`);
      }
    }

    fs.writeFileSync(file.path, newContent);
    console.log(`Updated ${path.relative(__dirname, file.path)}`);

  } catch (e) {
    console.error(`Failed to update ${file.path}:`, e);
    process.exit(1);
  }
}

console.log("Version update complete!");
