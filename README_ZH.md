# GVC (Gradle 版本目录管理器)

[![Crates.io](https://img.shields.io/crates/v/gvc.svg)](https://crates.io/crates/gvc)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

一个快速、独立的 CLI 工具，用于检查、列出、更新、新增、审计并诊断 Gradle 版本目录（`libs.versions.toml`）中的依赖与插件。

[English](README.md) | 简体中文

## 特性

- 🚀 **直接查询 Maven 仓库** - 无需 Gradle 运行时，纯 Rust 性能
- 📦 **多仓库支持** - Maven Central、Google Maven、自定义仓库，智能过滤
- 🎯 **智能版本检测** - 语义化版本控制，稳定性过滤（alpha、beta、RC、dev）
- 📋 **七个命令**：
  - `check` - 查看可用更新但不应用
  - `outdated` - 以类似包管理器的表格展示过期条目
  - `update` - 应用依赖更新
  - `list` - 以 Maven 坐标格式显示所有依赖
  - `audit` - 发现重复坐标、缺失版本引用等版本目录质量问题
  - `add` - 直接向版本目录写入依赖或插件并自动管理版本别名
  - `doctor` - 诊断 Kotlin/Android 版本目录一致性
- 🔒 **版本引用支持** - 处理 `[versions]` 表并自动解析
- 🎨 **美观的 CLI 输出** - 进度条、彩色输出、清晰的摘要
- ⚡ **智能请求优化** - 基于组模式的仓库过滤，最小化 HTTP 请求

## 前置要求

- Rust stable（用于从源码构建）
- 使用版本目录的 Gradle 项目（`gradle/libs.versions.toml`）
- Git（可选，用于分支/提交功能）
- 互联网连接（用于查询 Maven 仓库）

## 安装

### 从 crates.io 安装（推荐）

```bash
cargo install gvc
```

### 从 GitHub Releases 安装

从 [releases 页面](https://github.com/kingsword09/gvc/releases) 下载预编译的二进制文件：

```bash
# Linux/macOS
curl -L https://github.com/kingsword09/gvc/releases/download/v0.1.1/gvc-linux-x86_64 -o gvc
chmod +x gvc
sudo mv gvc /usr/local/bin/
```

### 从源码安装

```bash
git clone https://github.com/kingsword09/gvc.git
cd gvc
cargo install --path .
```

或手动构建：

```bash
cargo build --release
# 二进制文件位于 target/release/gvc
```

## 快速上手

```bash
gvc check              # 验证项目并列出可用更新
gvc outdated           # 以表格展示过期的版本目录条目
gvc audit              # 离线检查版本目录质量
gvc update --no-git    # 在不创建 Git 分支的情况下应用更新
gvc check --format json --fail-on-updates  # 适合 agent/CI 的更新检查
gvc doctor --format json --fail-on-issues  # Kotlin/Android 版本目录诊断
```

- 如果版本目录不在当前目录，请使用 `--path /path/to/project`。
- 使用 `--catalog gradle/custom.versions.toml` 可以指定项目内的某个版本目录文件。
- 调试或开发时，可使用 `--verbose` 或设置环境变量 `GVC_VERBOSE=1` 以查看 HTTP 请求、缓存等详细日志。

## 使用

### 命令速查表

| 命令 | 作用 | 常用参数 |
| --- | --- | --- |
| `gvc check` | 验证项目并打印可用的依赖/插件更新（不会写入文件）。 | `--include-unstable` 展示预发布版本；`--path` 指定其他项目。 |
| `gvc outdated` | 以类似包管理器的表格展示过期的版本别名、库和插件。 | `--include-unstable` 包含预发布版本；`--fail-on-updates` 在自动化场景中以退出码 2 表示发现更新。 |
| `gvc update` | 预览或应用版本目录更新，支持稳定性过滤与 Git 集成。 | `--dry-run` 预览；`--apply` 明确应用；`--target "*glob*"` 定向升级；`--no-git` 跳过 Git；`--no-stable-only` 允许预发布版本。 |
| `gvc list` | 以 Maven 坐标格式展示版本目录中的所有条目。 | `--path` 指向其他项目。 |
| `gvc audit` | 离线检查版本目录的可维护性。 | `--fail-on-issues` 在发现 warning/error 时以退出码 2 结束；`--format json` 适合自动化。 |
| `gvc doctor` | 离线检查 Kotlin、KSP、Android Gradle Plugin 与 Compose 的版本目录一致性。 | `--fail-on-issues` 在发现 warning/error 时以退出码 2 结束；`--format json` 适合自动化。 |
| `gvc add` | 默认向 `[libraries]` 插入新条目，也可写入 `[plugins]`。 | `-P/--plugin` 指定插件；`--no-stable-only` 解析 `:latest` 时允许预发布版本；`--alias` / `--version-alias` 自定义键名。 |

全局自动化参数：

- `--format text|json` - 输出人类可读文本或稳定 JSON 对象。
- `--quiet` - 在文本模式下隐藏进度输出。
- `--no-color` - 禁用 ANSI 颜色。
- `--catalog <file>` - 在 `--path` 项目内指定版本目录文件。

退出码：

- `0` - 命令成功完成。
- `1` - 校验、解析、网络、Git 或写入错误。
- `2` - `gvc check/outdated --fail-on-updates` 发现可用更新，或 `gvc audit/doctor --fail-on-issues` 发现诊断项。

### 检查更新

查看可用的依赖更新但不修改任何文件：

```bash
gvc check
# 或
gvc --path /path/to/project check
```

默认情况下，只显示稳定版本。要包含预发布版本：

```bash
gvc check --include-unstable
```

用于 CI 或 agent 的更新门禁：

```bash
gvc check --format json --fail-on-updates
```

### 查看过期条目

当你想要类似包管理器的紧凑视图时，可以使用 `outdated`：

```bash
gvc outdated
gvc outdated --include-unstable
gvc outdated --format json --fail-on-updates
```

`outdated` 与 `check` 使用同一套解析器，因此会尊重项目中配置的 Gradle 仓库以及相同的稳定版本过滤规则。

### 审计版本目录质量

对版本目录运行离线可维护性审计：

```bash
gvc audit
gvc audit --format json
gvc audit --fail-on-issues
```

当前 audit 会检查：

- 指向缺失 `[versions]` 别名的 `version.ref`。
- 多个别名指向同一个库或插件坐标。
- 可以迁移到 `[versions]` 的 inline version。
- 未被 catalog 中库或插件引用的 `[versions]` 别名。
- 多个 `[versions]` 别名使用同一个版本值。

### 列出依赖

以 Maven 坐标格式显示所有依赖（用于验证）：

```bash
gvc list
gvc list --format json
```

输出示例：
```
📦 Dependencies:

Libraries:
  androidx.core:core-ktx:1.12.0
  com.squareup.okhttp3:okhttp:4.12.0
  org.jetbrains.compose.runtime:runtime:1.9.0

Plugins:
  org.jetbrains.kotlin.jvm:1.9.0
  com.android.application:8.1.0

Summary:
  4 libraries
  2 plugins
```

### 诊断 Kotlin/Android 版本目录

对 Kotlin/Android 项目的版本目录运行离线诊断：

```bash
gvc doctor
gvc doctor --format json
gvc doctor --fail-on-issues
```

当前 doctor 会检查：

- Kotlin Gradle Plugin 条目是否使用同一个版本。
- KSP 版本是否使用预期的 `<kotlin-version>-<ksp-version>` 前缀。
- Kotlin 2.x + Compose 项目是否声明 `org.jetbrains.kotlin.plugin.compose`。
- Kotlin Compose compiler 插件版本是否与 Kotlin Gradle Plugin 一致。
- `com.android.*` 插件是否共享同一个 Android Gradle Plugin 版本。

`doctor` 只检查版本目录且不会访问网络，适合 CI 和 agent 工作流。

### 更新依赖

应用依赖更新（默认仅更新稳定版本）：

```bash
gvc update
```

#### 选项

- `--stable-only` - 仅更新到稳定版本（默认启用）
- `--no-stable-only` - 允许更新到不稳定版本（alpha、beta、RC）
- `--dry-run` - 只预览更新，不写入版本目录
- `--apply` - 明确应用更新（默认行为）
- `--target <glob>` - 使用 glob 匹配别名，仅更新匹配到的依赖（例如 `*okhttp*`）
- `-i`, `--interactive` - 在写入前逐项确认或跳过每个更新
- `--filter <glob>` - `--target` 的向后兼容别名
- `--no-git` - 跳过 Git 操作（不创建分支/提交）
- `--path`, `-p` - 指定项目目录
- `--format json` - 以 JSON 输出本次应用的更新报告

交互模式会在每个候选更新处暂停，展示旧版本与新版本，并允许你选择接受、跳过、应用剩余全部更新或直接取消。

dry-run 模式是只读的，不要求 Git 工作区干净：

```bash
gvc update --dry-run
gvc update --dry-run --target kotlin --format json
```

当提供 `--target` 时，GVC 会把所有符合条件的库、版本别名或插件列出来让你挑选目标；配合 `-i/--interactive` 可以进一步选择想安装的稳定版或预发布版本。若未开启交互模式，只有在目标模式精确匹配一个条目时才会自动选择最新可升级版本；匹配多个条目时需要收窄目标或添加 `--interactive`。

**示例：**

```bash
# 仅更新到稳定版本（默认行为）
gvc update

# 包含不稳定版本（alpha、beta、RC）
gvc update --no-stable-only

# 逐项确认每一个变更
gvc update --interactive

# 仅更新匹配到关键字的依赖
gvc update --target "*okhttp*"

# 不使用 Git 集成更新
gvc update --no-git

# 更新特定项目
gvc update --path /path/to/project
```

#### 定向更新

当提供 `--target` 时，GVC 会执行以下步骤：

1. 列出所有与 glob 模式匹配的版本别名、库或插件（匹配不区分大小写）。
2. 让你选择要更新的目标条目。
3. 从配置的仓库里拉取该依赖的全部版本信息。
4. 若打开交互模式（`-i`），可以在最近的稳定版和预发布版本中选择（`m` 展示更多、`s` 跳过、`q` 取消）。
5. 若未开启交互模式且只匹配到一个条目，则会依据稳定性规则自动挑选最新可升级版本；若匹配多个条目，GVC 会提示收窄目标或添加 `--interactive`。

这样就能在不影响其他依赖的情况下，精确更新单个库或插件，甚至指定升级到某个预发布版本。

```bash
# 为别名中包含 "okhttp" 的依赖挑选目标版本
gvc update --target "*okhttp*" --interactive
```

- 不加 `--interactive` 时，GVC 会按照稳定性规则自动选择最新版本，适合脚本化使用。
- 想评估 beta/RC 等预发布版本时，可结合 `--no-stable-only`。

### 添加依赖或插件

直接基于坐标写入新的版本目录条目：

```bash
# 添加库（格式：group:artifact:version，默认目标）
gvc add androidx.lifecycle:lifecycle-runtime-ktx:2.6.2

# 添加插件（格式：plugin.id:version）
gvc add -P org.jetbrains.kotlin.jvm:1.9.24

# 自动解析最新版本
gvc add com.squareup.okhttp3:okhttp:latest
gvc add -P org.jetbrains.kotlin.android:latest --no-stable-only  # 需要时允许预发布版本
```

- GVC 会自动生成目录别名和版本键；若需自定义，可使用 `--alias` 或 `--version-alias`。
- `-P` 是插件短参数；全局 `-p/--path` 专用于项目路径。
- 库条目写入为 `{ module = "group:artifact", version = { ref = "<alias>" } }`。
- 插件条目写入为 `{ id = "plugin.id", version = { ref = "<alias>" } }`。
- 已存在的版本别名不会被隐式更新。只有在明确希望新条目移动某个已有版本键时，才传入 `--update-version-alias`。
- 写入前会根据当前仓库配置（库）或 Gradle Plugin Portal（插件）校验坐标与版本是否存在；处理 `:latest` 时默认选择稳定版，可通过 `--no-stable-only` 允许预发布版本。
- `--path` 参数的行为与其他命令一致。

## 工作原理

GVC 直接查询 Maven 仓库，无需 Gradle：

1. **项目验证** - 检查 `gradle/libs.versions.toml` 和 `gradlew`
2. **仓库配置** - 读取 Gradle 构建文件以检测配置的 Maven 仓库
3. **TOML 解析** - 使用 `toml_edit` 解析版本目录，同时保留格式
4. **版本解析**：
   - 解析所有支持的 TOML 格式中的依赖
   - 从 `[versions]` 表解析版本引用
   - 通过 HTTP 查询 Maven 仓库获取最新版本
   - 根据仓库组模式应用智能过滤
5. **版本比较**：
   - 语义化版本支持（1.0.0、2.1.3）
   - 过滤不稳定版本（alpha、beta、RC、dev、snapshot、preview 等）
   - 防止版本降级
6. **应用更新** - 更新 TOML 文件，同时保持原始格式

### 支持的 TOML 格式

GVC 支持所有 Gradle 版本目录格式：

```toml
# 简单字符串格式
[libraries]
okhttp = "com.squareup.okhttp3:okhttp:4.11.0"

# 表格式带 module
okhttp = { module = "com.squareup.okhttp3:okhttp", version = "4.11.0" }

# 表格式带 group 和 name
okhttp = { group = "com.squareup.okhttp3", name = "okhttp", version = "4.11.0" }

# 版本引用（自动解析）
[versions]
okhttp = "4.11.0"

[libraries]
okhttp = { group = "com.squareup.okhttp3", name = "okhttp", version.ref = "okhttp" }
```

### 智能仓库过滤

GVC 根据依赖组自动过滤仓库请求：

- **Google Maven** - 仅查询 `google.*`、`android.*`、`androidx.*` 包
- **Maven Central** - 查询所有其他包
- **自定义仓库** - 遵守 `mavenContent.includeGroupByRegex` 模式

这显著减少了不必要的 HTTP 请求并加快了检查速度。

## 架构概览

- `src/workflow.rs` 负责编排 CLI 命令、显示进度并处理 Git 交互。
- agent 模块聚焦特定职责：
  - `ProjectScannerAgent` 验证 Gradle 结构并定位 `libs.versions.toml`。
  - `DependencyUpdater` 解析、检查并更新版本目录，同时利用仓库信息解析版本。
  - `VersionControlAgent` 确保工作区干净，在启用时创建更新分支与提交。
- 若需深入了解 agent 之间的协作与扩展方式，请阅读 [AGENTS.md](AGENTS.md)。

## 项目要求

您的 Gradle 项目必须具有：

1. **版本目录文件**：`gradle/libs.versions.toml`
2. **Gradle 包装器**：`gradlew` 或 `gradlew.bat`（用于仓库检测）

**无需 Gradle 插件！** GVC 直接查询 Maven 仓库并更新您的 TOML 文件。

## 仓库检测

GVC 自动从您的 Gradle 构建文件中读取仓库配置：

- `settings.gradle.kts` / `settings.gradle`
- `build.gradle.kts` / `build.gradle`

检测到的仓库：
- `mavenCentral()`
- `google()`
- `gradlePluginPortal()`
- 自定义 `maven { url = "..." }` 声明
- 仓库内容过滤器（`mavenContent.includeGroupByRegex`）

## 示例

### 检查更新

```bash
$ gvc check

正在检查可用更新（稳定版本）...

1. 验证项目结构...
✓ 项目结构有效

2. 读取 Gradle 仓库配置...
   找到 3 个仓库：
   • Maven Central (https://repo1.maven.org/maven2)
   • Google Maven (https://dl.google.com/dl/android/maven2)
   • Gradle Plugin Portal (https://plugins.gradle.org/m2)

3. 检查可用更新...

检查版本变量...
[========================================] 10/10

检查库更新...
[========================================] 25/25

✓ 检查完成

📦 可用更新：
找到 5 个更新
   （仅显示稳定版本）

版本更新：
  • okio-version 3.16.0 → 3.16.2
  • kotlin-version 2.2.20 → 2.2.21
  • ktor-version 3.3.0 → 3.3.1

库更新：
  • some-direct-lib 0.9.0 → 0.10.0 (stable)

要应用这些更新，请运行：
  gvc update --stable-only
```

### 列出所有依赖

```bash
$ gvc list

正在列出版本目录中的依赖...

1. 验证项目结构...
✓ 项目结构有效

2. 读取版本目录...
✓ 目录已加载

📦 依赖：

Libraries:
  androidx.core:core-ktx:1.17.0
  com.squareup.okhttp3:okhttp:4.12.0
  io.ktor:ktor-server-core:3.3.0
  org.jetbrains.compose.runtime:runtime:1.9.0

Plugins:
  org.jetbrains.kotlin.jvm:2.2.20
  com.android.application:8.13.0

摘要：
  4 个库
  2 个插件
```

## 故障排除

### "未找到 Gradle wrapper"

确保您的项目根目录中有 `gradlew`（Linux/Mac）或 `gradlew.bat`（Windows）。

### "未找到 gradle/libs.versions.toml"

确保您的项目使用 Gradle 版本目录，并且文件存在于 `gradle/libs.versions.toml`。

### "工作目录有未提交的更改"

在运行更新命令之前提交或暂存您的更改，或使用 `--no-git` 跳过 Git 操作。

## 开发

### 项目结构

```
gvc/
├── src/
│   ├── main.rs              # 入口点
│   ├── cli.rs               # CLI 参数解析
│   ├── workflow.rs          # 命令编排
│   ├── error.rs             # 错误类型
│   ├── agents/
│   │   ├── dependency_updater.rs  # 核心更新逻辑
│   │   ├── project_scanner.rs     # 项目验证
│   │   └── version_control.rs     # Git 操作
│   ├── gradle/
│   │   └── config_parser.rs       # Gradle 配置解析
│   └── maven/
│       ├── repository.rs          # Maven HTTP 客户端
│       ├── version.rs             # 版本比较
│       └── mod.rs                 # Maven 坐标解析
├── Cargo.toml
└── README.md
```

### 构建

```bash
# 开发模式
cargo build

# 发布模式（优化）
cargo build --release
```

### 测试

```bash
cargo test
```

### 开发运行

```bash
# 检查更新
cargo run -- check

# 列出依赖
cargo run -- list

# 更新依赖
cargo run -- update --no-git
```

想了解这些工作流背后的 agent 设计，请参阅 [AGENTS.md](AGENTS.md)。

## 许可证

Apache-2.0

## 贡献

欢迎贡献！请随时提交 Pull Request。

### 开发设置

1. 克隆仓库：
   ```bash
   git clone https://github.com/kingsword09/gvc.git
   cd gvc
   ```

2. 构建和测试：
   ```bash
   cargo build
   cargo test
   cargo fmt
   cargo clippy --all-targets --all-features
   ```

3. 本地运行：
   ```bash
   cargo run -- check
   cargo run -- update --no-git
   ```

查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解更多详情。

## 更新日志

查看 [CHANGELOG.md](CHANGELOG.md) 了解版本历史。

## 路线图

- [ ] 异步 HTTP 请求以实现并发版本查询
- [ ] Maven 元数据的本地缓存
- [ ] 交互式 TUI 模式以进行选择性更新
- [x] 支持 Gradle 插件更新（集成 Gradle Plugin Portal）✅
- [ ] 配置文件支持（`.gvcrc`）
- [ ] 更好的错误消息和建议
