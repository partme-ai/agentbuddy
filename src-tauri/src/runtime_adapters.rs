// AgentBuddy — 多运行时适配器：WorkBuddy / OpenClaw / ZQClaw / Hermes
// 每个运行时实现 探测 / 安装 / 卸载 三个操作；安装内容从 experts 插件树转换生成。
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetRuntime {
    Workbuddy,
    Openclaw,
    Zqclaw,
    Hermes,
}

impl TargetRuntime {
    pub fn all() -> [TargetRuntime; 4] {
        [TargetRuntime::Workbuddy, TargetRuntime::Openclaw, TargetRuntime::Zqclaw, TargetRuntime::Hermes]
    }
    pub fn id(&self) -> &'static str {
        match self {
            TargetRuntime::Workbuddy => "workbuddy",
            TargetRuntime::Openclaw => "openclaw",
            TargetRuntime::Zqclaw => "zqclaw",
            TargetRuntime::Hermes => "hermes",
        }
    }
    pub fn display_name(&self) -> &'static str {
        match self {
            TargetRuntime::Workbuddy => "WorkBuddy",
            TargetRuntime::Openclaw => "OpenClaw",
            TargetRuntime::Zqclaw => "ZQClaw",
            TargetRuntime::Hermes => "Hermes Agent",
        }
    }
    /// 引导用户去安装该运行时的官方入口
    pub fn install_hint(&self) -> (&'static str, &'static str) {
        match self {
            TargetRuntime::Workbuddy => ("WorkBuddy 官网下载桌面版", "https://workbuddy.ai"),
            TargetRuntime::Openclaw => ("OpenClaw 文档（安装引导）", "https://docs.openclaw.ai"),
            TargetRuntime::Zqclaw => ("ZQClaw 安装引导（内部）", "https://docs.openclaw.ai"),
            TargetRuntime::Hermes => ("Hermes Agent 官方文档", "https://hermes-agent.nousresearch.com/docs/"),
        }
    }
    fn root(&self, home: &Path) -> PathBuf {
        match self {
            TargetRuntime::Workbuddy => home.join(".workbuddy"),
            TargetRuntime::Openclaw => home.join(".openclaw"),
            TargetRuntime::Zqclaw => home.join(".zqclaw"),
            TargetRuntime::Hermes => home.join(".hermes"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub runtime: &'static str,
    pub display_name: &'static str,
    pub installed: bool,
    pub detail: String,
    pub install_hint_title: String,
    pub install_hint_url: String,
}

/// 探测：运行时根目录 / 应用是否存在
pub fn detect_runtimes(home: &Path) -> Vec<RuntimeStatus> {
    TargetRuntime::all()
        .iter()
        .map(|rt| {
            let root = rt.root(home);
            let installed = match rt {
                TargetRuntime::Workbuddy => root.is_dir() || Path::new("/Applications/WorkBuddy.app").exists(),
                _ => root.is_dir(),
            };
            let detail = if installed {
                format!("{} 根目录已存在", root.display())
            } else {
                format!("未检测到 {}（{}）", rt.display_name(), root.display())
            };
            let (title, url) = rt.install_hint();
            RuntimeStatus {
                runtime: rt.id(),
                display_name: rt.display_name(),
                installed,
                detail,
                install_hint_title: title.to_string(),
                install_hint_url: url.to_string(),
            }
        })
        .collect()
}

// ────────────────────────── 内容转换 ──────────────────────────

/// 从 workbuddy 格式的 agent .md（frontmatter name/description + 正文）提取字段
fn parse_agent_md(text: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut desc = String::new();
    let mut body = text.to_string();
    if text.starts_with("---") {
        if let Some(end) = text[3..].find("\n---") {
            let fm = &text[3..3 + end];
            for line in fm.lines() {
                if let Some(v) = line.strip_prefix("name:") {
                    name = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("description:") {
                    desc = v.trim().to_string();
                }
            }
            body = text[3 + end + 4..].to_string();
        }
    }
    (name, desc, body)
}

/// OpenClaw / ZQClaw 三件套：SOUL.md（人格正文）+ AGENTS.md（行为）+ IDENTITY.md（身份）
fn write_claw_agent(agent_dir: &Path, agent_name: &str, desc: &str, body: &str) -> std::io::Result<()> {
    fs::create_dir_all(agent_dir)?;
    fs::write(
        agent_dir.join("SOUL.md"),
        format!("# {} 灵魂\n\n{}\n", agent_name, body.trim()),
    )?;
    fs::write(
        agent_dir.join("AGENTS.md"),
        format!(
            "# {} 行为契约\n\n- 保持角色一致性，按职能路由任务\n- 输出前自检交付质量\n- 无法处理时上报而不是静默失败\n",
            agent_name
        ),
    )?;
    fs::write(
        agent_dir.join("IDENTITY.md"),
        format!("# {}\n\n- **身份**：{}\n- **职责**：{}\n", agent_name, agent_name, desc),
    )?;
    Ok(())
}

/// Hermes：分类 SKILL.md（~/.hermes/skills/agentbuddy/<name>/SKILL.md）
fn write_hermes_skill(skills_root: &Path, skill_name: &str, desc: &str, body: &str) -> std::io::Result<()> {
    let dir = skills_root.join("agentbuddy").join(skill_name);
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: {}\ndescription: \"{}\"\n---\n\n{}",
            skill_name,
            desc.replace('"', "'"),
            body.trim()
        ),
    )
}

/// 收集一个插件内全部 agent 定义（agents/*.md，递归一层）
fn collect_agents(plugin_path: &Path) -> Vec<(String, String, String)> {
    let agents_dir = plugin_path.join("agents");
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
                let text = fs::read_to_string(&path).unwrap_or_default();
                let (name, desc, body) = parse_agent_md(&text);
                let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let name = if name.is_empty() { stem } else { name };
                out.push((name, desc, body));
            }
        }
    }
    out
}

// ────────────────────────── 安装 / 卸载 ──────────────────────────

/// 把一个插件安装到 claw 系运行时（OpenClaw / ZQClaw）
/// - 团队插件：成员逐个转为 <root>/agents/<agent>/agent/ 三件套
/// - 单专家插件：同样按其 agents/*.md 处理
/// - 无 agents 目录的插件：把 SKILL.md 主体作为单 agent 三件套
fn install_claw(root: &Path, plugin_path: &Path, plugin_name: &str) -> std::io::Result<usize> {
    let agents = collect_agents(plugin_path);
    let mut count = 0;
    if !agents.is_empty() {
        for (name, desc, body) in &agents {
            let dir = root.join("agents").join(name).join("agent");
            write_claw_agent(&dir, name, desc, body)?;
            count += 1;
        }
    } else {
        let skill = plugin_path.join("SKILL.md");
        let text = fs::read_to_string(skill).unwrap_or_default();
        let (_, desc, body) = parse_agent_md(&text);
        let dir = root.join("agents").join(plugin_name).join("agent");
        write_claw_agent(&dir, plugin_name, &desc, &body)?;
        count += 1;
    }
    Ok(count)
}

/// 把一个插件安装到 Hermes（skills/agentbuddy/<name>/SKILL.md）
fn install_hermes(root: &Path, plugin_path: &Path, plugin_name: &str) -> std::io::Result<usize> {
    let skills_root = root.join("skills");
    let agents = collect_agents(plugin_path);
    let mut count = 0;
    if !agents.is_empty() {
        // 团队：lead（或全部成员）各成一条 skill，名前缀团队名防撞
        for (name, desc, body) in &agents {
            write_hermes_skill(&skills_root, &format!("{}-{}", plugin_name, name), desc, body)?;
            count += 1;
        }
    } else {
        let skill = plugin_path.join("SKILL.md");
        let text = fs::read_to_string(skill).unwrap_or_default();
        let (_, desc, body) = parse_agent_md(&text);
        write_hermes_skill(&skills_root, plugin_name, &desc, &body)?;
        count += 1;
    }
    Ok(count)
}

/// 统一入口：安装所选插件到目标运行时
pub fn install_plugins(
    runtime: TargetRuntime,
    home: &Path,
    experts_root: &Path,
    plugin_names: &[String],
    workbuddy_engine: &dyn Fn(&[String]) -> Result<usize, String>,
) -> Result<usize, String> {
    match runtime {
        TargetRuntime::Workbuddy => workbuddy_engine(plugin_names),
        TargetRuntime::Openclaw => {
            let root = home.join(".openclaw");
            let mut total = 0;
            for name in plugin_names {
                total += install_claw(&root, &experts_root.join("plugins").join(name), name)
                    .map_err(|e| e.to_string())?;
            }
            Ok(total)
        }
        TargetRuntime::Zqclaw => {
            let root = home.join(".zqclaw");
            let mut total = 0;
            for name in plugin_names {
                total += install_claw(&root, &experts_root.join("plugins").join(name), name)
                    .map_err(|e| e.to_string())?;
            }
            Ok(total)
        }
        TargetRuntime::Hermes => {
            let root = home.join(".hermes");
            let mut total = 0;
            for name in plugin_names {
                total += install_hermes(&root, &experts_root.join("plugins").join(name), name)
                    .map_err(|e| e.to_string())?;
            }
            Ok(total)
        }
    }
}

/// 统一入口：从目标运行时卸载所选插件
pub fn uninstall_plugins(
    runtime: TargetRuntime,
    home: &Path,
    plugin_names: &[String],
    workbuddy_engine: &dyn Fn(&[String]) -> Result<usize, String>,
) -> Result<usize, String> {
    match runtime {
        TargetRuntime::Workbuddy => workbuddy_engine(plugin_names),
        TargetRuntime::Openclaw | TargetRuntime::Zqclaw => {
            let root = runtime.root(home).join("agents");
            let mut removed = 0;
            for name in plugin_names {
                // 团队插件按成员名展开删除；无成员文件时按插件名删
                let mut removed_any = false;
                if let Ok(entries) = fs::read_dir(&root) {
                    for entry in entries.flatten() {
                        let agent_dir_name = entry.file_name().to_string_lossy().to_string();
                        if agent_dir_name.starts_with(name.as_str()) || plugin_names.contains(&agent_dir_name) {
                            let _ = fs::remove_dir_all(entry.path());
                            removed_any = true;
                        }
                    }
                }
                if !removed_any {
                    let _ = fs::remove_dir_all(root.join(name));
                }
                removed += 1;
            }
            Ok(removed)
        }
        TargetRuntime::Hermes => {
            let skills = home.join(".hermes/skills/agentbuddy");
            let mut removed = 0;
            for name in plugin_names {
                let dir = skills.join(name);
                if dir.exists() {
                    fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
                }
                removed += 1;
            }
            Ok(removed)
        }
    }
}
