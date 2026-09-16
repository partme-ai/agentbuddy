# AgentBuddy

WorkBuddy 智能体 / 专家团跨平台安装器（Tauri v2）。

- 自动识别 WorkBuddy 安装位置（/Applications/WorkBuddy.app、~/.workbuddy），支持手动更改
- 列出全部专家团与单专家（含图标、描述、版本），可搜索可筛选
- 默认预选视频制作剪辑相关团队（video-factory / short-drama-studio / dreamina-design / dreamina-canvas / dreamina-3d）
- 非破坏式安装：只替换本包拥有的插件，注册表写入前自动 .bak，保留外来插件

## 构建

```bash
cd src-tauri
cargo tauri build        # macOS: .app + .dmg（另见 dist 说明）
# Windows/Linux：在对应 CI runner 上同样命令（.msi / .AppImage / .deb）
```

`experts/` 是构建资源符号链接（指向 workbuddy-agent-experts 仓的预构建树）；CI 上用发布 zip 解压或 clone 该仓后创建同名链接。

## CLI 无头模式

```bash
agentbuddy --cli --list
agentbuddy --cli --config-dir /path --select video-factory-team,short-drama-studio-team
```
