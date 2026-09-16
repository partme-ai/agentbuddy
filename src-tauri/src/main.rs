// AgentBuddy — WorkBuddy 智能体/专家团跨平台安装器
// CLI 无头模式：agentbuddy --cli --experts <path> [--list | --config-dir <path> --select a,b|all]
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginInfo {
    name: String,
    display_name: String,
    version: String,
    description: String,
    is_team: bool,
    has_icon: bool,
    #[serde(skip)]
    plugin_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct DetectResult {
    app_path: Option<String>,
    config_dir: String,
    config_exists: bool,
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

fn experts_root() -> PathBuf {
    // 候选：exe 旁的 experts（开发目录结构）→ macOS bundle Contents/Resources/experts → cwd/experts
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            for cand in [
                dir.join("experts"),                       // Contents/MacOS/experts
                dir.join("../Resources/experts"),          // Contents/Resources/experts
                dir.join("../../Resources/experts"),       // 兜底
            ] {
                if cand.is_dir() {
                    return cand;
                }
            }
        }
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cand = cwd.join("experts");
    if cand.is_dir() {
        return cand;
    }
    cand.parent().map(|p| p.join("experts")).unwrap_or(cand)
}

fn read_json(path: &Path) -> Option<Value> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn list_plugins(experts: &Path) -> Vec<PluginInfo> {
    let manifest_path = experts.join(".codebuddy-plugin/marketplace.json");
    let manifest = match read_json(&manifest_path) {
        Some(v) => v,
        None => return vec![],
    };
    let entries = manifest
        .get("plugins")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for entry in entries {
        let name = match entry.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let plugin_path = experts.join("plugins").join(&name);
        let pj = read_json(&plugin_path.join(".codebuddy-plugin/plugin.json"))
            .unwrap_or_else(|| json!({}));
        let display = pj
            .get("displayName")
            .and_then(|d| d.get("zh").or(d.get("en")))
            .and_then(|v| v.as_str())
            .unwrap_or(&name)
            .to_string();
        let version = pj
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0")
            .to_string();
        let description = pj
            .get("description")
            .and_then(|v| v.as_str())
            .or_else(|| entry.get("description").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();
        let is_team = pj
            .get("expertType")
            .and_then(|v| v.as_str())
            .map(|t| t == "team")
            .unwrap_or(true);
        let has_icon = plugin_path.join("avatars/team.png").is_file()
            || plugin_path.join("avatars/expert.png").is_file();
        out.push(PluginInfo {
            name,
            display_name: display,
            version,
            description,
            is_team,
            has_icon,
            plugin_path,
        });
    }
    out
}

fn detect_workbuddy(config_override: Option<&str>) -> DetectResult {
    let app_candidates = [
        "/Applications/WorkBuddy.app".to_string(),
        format!("{}/Applications/WorkBuddy.app", env::var("HOME").unwrap_or_default()),
        "/Applications/Agent Buddy.app".to_string(),
    ];
    let app_path = app_candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .cloned();
    let config_dir = config_override
        .map(expand_home)
        .unwrap_or_else(|| expand_home("~/.workbuddy"));
    DetectResult {
        app_path,
        config_dir: config_dir.display().to_string(),
        config_exists: config_dir.is_dir(),
    }
}

fn backup(path: &Path) {
    if path.is_file() {
        let bak = path.with_extension("json.bak-agentbuddy");
        let _ = fs::remove_file(&bak);
        let _ = fs::copy(path, &bak);
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst.parent().unwrap())?;
    copy_tree(src, dst)
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn iso_now() -> String {
    // 简单 UTC 时间戳，与 install.py 形状一致（无需 chrono）
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let (s, m, h, d, mo, y) = epoch_to_utc(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000Z", y, mo, d, h, m, s)
}

fn epoch_to_utc(secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = secs.div_euclid(86_400);
    let mut rem = secs.rem_euclid(86_400);
    let h = rem / 3600;
    rem %= 3600;
    let m = rem / 60;
    let s = rem % 60;
    let mut y = 1970i64;
    let mut days_left = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let dy = if leap { 366 } else { 365 };
        if days_left >= dy {
            days_left -= dy;
            y += 1;
        } else {
            break;
        }
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 0i64;
    for &dm in &mdays {
        if days_left >= dm {
            days_left -= dm;
            mo += 1;
        } else {
            break;
        }
    }
    (s, m, h, days_left + 1, mo + 1, y)
}

fn install_selected(selected: &[PluginInfo], experts: &Path, config_dir: &Path) -> Result<usize, String> {
    let plugins_dir = config_dir.join("plugins/marketplaces/my-experts/plugins");
    let cache_root = config_dir.join("plugins/cache/my-experts");
    fs::create_dir_all(&plugins_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&cache_root).map_err(|e| e.to_string())?;

    for p in selected {
        copy_dir_all(&p.plugin_path, &plugins_dir.join(&p.name)).map_err(|e| e.to_string())?;
        let cache_target = cache_root.join(&p.name).join(&p.version);
        copy_dir_all(&p.plugin_path, &cache_target).map_err(|e| e.to_string())?;
    }

    // marketplace manifest — merge（保留外来插件）
    let manifest_path = config_dir.join("plugins/marketplaces/my-experts/.codebuddy-plugin/marketplace.json");
    backup(&manifest_path);
    let src_manifest = read_json(&experts.join(".codebuddy-plugin/marketplace.json")).unwrap_or_else(|| json!({}));
    let selected_names: Vec<&str> = selected.iter().map(|p| p.name.as_str()).collect();
    let existing = read_json(&manifest_path).unwrap_or_else(|| json!({"name": "my-experts", "plugins": []}));
    let mut merged: Vec<Value> = existing
        .get("plugins")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| {
            e.get("name").and_then(|n| n.as_str())
                .map(|n| !selected_names.contains(&n))
                .unwrap_or(true)
        })
        .collect();
    if let Some(entries) = src_manifest.get("plugins").and_then(|p| p.as_array()) {
        for e in entries {
            if e.get("name").and_then(|n| n.as_str()).map(|n| selected_names.contains(&n)).unwrap_or(false) {
                merged.push(e.clone());
            }
        }
    }
    let market_obj = json!({"name": "my-experts", "plugins": merged});
    fs::create_dir_all(manifest_path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&market_obj).unwrap()).map_err(|e| e.to_string())?;

    // known_marketplaces
    let known_path = config_dir.join("plugins/known_marketplaces.json");
    backup(&known_path);
    let mut known = read_json(&known_path).unwrap_or_else(|| json!({}));
    let market_str = config_dir.join("plugins/marketplaces/my-experts").display().to_string();
    known.as_object_mut().unwrap().insert(
        "my-experts".to_string(),
        json!({
            "manifestName": "my-experts",
            "type": "directory",
            "source": {"source": "directory", "path": market_str},
            "installLocation": market_str,
            "description": format!("Marketplace from {}", market_str),
            "lastUpdated": iso_now(),
            "autoUpdate": false,
        }),
    );
    fs::write(&known_path, serde_json::to_string_pretty(&known).unwrap()).map_err(|e| e.to_string())?;

    // installed_plugins.json
    let inst_path = config_dir.join("plugins/installed_plugins.json");
    backup(&inst_path);
    let mut inst = read_json(&inst_path).unwrap_or_else(|| json!({"version": 2, "plugins": {}}));
    let map = inst
        .as_object_mut()
        .and_then(|o| o.get_mut("plugins"))
        .and_then(|v| v.as_object_mut());
    let mut plugins_map = match map {
        Some(m) => m.clone(),
        None => Map::new(),
    };
    for p in selected {
        let key = format!("{}@my-experts", p.name);
        plugins_map.insert(
            key,
            json!([{
                "scope": "user",
                "installPath": cache_root.join(&p.name).join(&p.version).display().to_string(),
                "version": p.version,
                "installedAt": iso_now(),
                "lastUpdated": iso_now(),
            }]),
        );
    }
    let final_inst = json!({"version": 2, "plugins": plugins_map});
    fs::write(&inst_path, serde_json::to_string_pretty(&final_inst).unwrap()).map_err(|e| e.to_string())?;

    Ok(selected.len())
}

#[tauri::command]
fn detect() -> DetectResult {
    detect_workbuddy(None)
}

#[tauri::command]
fn catalog() -> Vec<PluginInfo> {
    list_plugins(&experts_root())
}

#[tauri::command]
fn plugin_icon(name: String) -> Option<Vec<u8>> {
    let root = experts_root();
    let plugin = root.join("plugins").join(&name);
    for cand in ["avatars/team.png", "avatars/expert.png"] {
        let p = plugin.join(cand);
        if let Ok(bytes) = fs::read(&p) {
            return Some(bytes);
        }
    }
    None
}

#[tauri::command]
fn install(names: Vec<String>, config_dir: Option<String>) -> Result<usize, String> {
    let experts = experts_root();
    let catalog = list_plugins(&experts);
    let selected: Vec<PluginInfo> = catalog
        .into_iter()
        .filter(|p| names.contains(&p.name))
        .collect();
    let dir = config_dir
        .map(|d| expand_home(&d))
        .unwrap_or_else(|| expand_home("~/.workbuddy"));
    install_selected(&selected, &experts, &dir)
}


fn uninstall_selected(names: &[String], config_dir: &Path) -> Result<usize, String> {
    let plugins_dir = config_dir.join("plugins/marketplaces/my-experts/plugins");
    let cache_root = config_dir.join("plugins/cache/my-experts");
    let mut removed = 0usize;
    for name in names {
        let target = plugins_dir.join(name);
        if target.exists() {
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
            removed += 1;
        }
        let cache_target = cache_root.join(name);
        if cache_target.exists() {
            fs::remove_dir_all(&cache_target).map_err(|e| e.to_string())?;
        }
    }
    if removed == 0 {
        return Ok(0);
    }

    // installed_plugins.json：删掉 <name>@my-experts 条目
    let inst_path = config_dir.join("plugins/installed_plugins.json");
    if inst_path.is_file() {
        backup(&inst_path);
        let mut inst = read_json(&inst_path).unwrap_or_else(|| json!({"version": 2, "plugins": {}}));
        if let Some(map) = inst.as_object_mut()
            .and_then(|o| o.get_mut("plugins"))
            .and_then(|v| v.as_object_mut())
        {
            for name in names {
                map.remove(&format!("{}@my-experts", name));
            }
        }
        fs::write(&inst_path, serde_json::to_string_pretty(&inst).unwrap()).map_err(|e| e.to_string())?;
    }

    // marketplace manifest：删掉所选条目（外来条目保留）
    let manifest_path = config_dir.join("plugins/marketplaces/my-experts/.codebuddy-plugin/marketplace.json");
    if manifest_path.is_file() {
        backup(&manifest_path);
        let mut market = read_json(&manifest_path).unwrap_or_else(|| json!({"name": "my-experts", "plugins": []}));
        if let Some(entries) = market.get("plugins").and_then(|p| p.as_array()).cloned() {
            let kept: Vec<Value> = entries
                .into_iter()
                .filter(|e| {
                    e.get("name").and_then(|n| n.as_str())
                        .map(|n| !names.contains(&n.to_string()))
                        .unwrap_or(true)
                })
                .collect();
            market.as_object_mut().unwrap().insert("plugins".to_string(), json!(kept));
            fs::write(&manifest_path, serde_json::to_string_pretty(&market).unwrap()).map_err(|e| e.to_string())?;
        }
    }
    Ok(removed)
}

#[tauri::command]
fn uninstall(names: Vec<String>, config_dir: Option<String>) -> Result<usize, String> {
    let dir = config_dir
        .map(|d| expand_home(&d))
        .unwrap_or_else(|| expand_home("~/.workbuddy"));
    uninstall_selected(&names, &dir)
}

#[tauri::command]
fn pick_config_dir() -> Option<String> {
    None // 前端用 dialog plugin 替代；占位以保持 invoke 面稳定
}

fn cli_main() -> i32 {
    let args: Vec<String> = env::args().collect();
    let experts = args
        .windows(2)
        .find(|w| w[0] == "--experts")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(experts_root);
    let catalog = list_plugins(&experts);
    if args.iter().any(|a| a == "--list") {
        for p in &catalog {
            println!("{}\t{}\t{}", p.name, if p.is_team { "team" } else { "agent" }, p.display_name);
        }
        return 0;
    }
    let dir = args
        .windows(2)
        .find(|w| w[0] == "--config-dir")
        .map(|w| expand_home(&w[1]));
    let select_spec = args.windows(2).find(|w| w[0] == "--select").map(|w| w[1].clone());
    let names: Vec<String> = match select_spec.as_deref() {
        Some("all") | None => catalog.iter().map(|p| p.name.clone()).collect(),
        Some(s) => s.split(',').map(|x| x.trim().to_string()).collect(),
    };
    let selected: Vec<PluginInfo> = catalog
        .into_iter()
        .filter(|p| names.contains(&p.name))
        .collect();
    let dir = dir.unwrap_or_else(|| expand_home("~/.workbuddy"));
    if args.iter().any(|a| a == "--uninstall") {
        return match uninstall_selected(&names, &dir) {
            Ok(n) => {
                println!("uninstalled {} plugins from {}", n, dir.join("plugins/marketplaces/my-experts").display());
                0
            }
            Err(e) => {
                eprintln!("uninstall failed: {}", e);
                1
            }
        };
    }
    match install_selected(&selected, &experts, &dir) {
        Ok(n) => {
            println!("installed {} plugins into {}", n, dir.join("plugins/marketplaces/my-experts").display());
            0
        }
        Err(e) => {
            eprintln!("install failed: {}", e);
            1
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--cli" || a == "--list") {
        std::process::exit(cli_main());
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![detect, catalog, plugin_icon, install, uninstall, pick_config_dir])
        .run(tauri::generate_context!())
        .expect("error while running AgentBuddy");
}
