# GVC (Gradle 版本目录更新器)

一个快速、独立的 CLI 工具，用于检查和更新 Gradle 版本目录（`libs.versions.toml`）中的依赖。

[English](README.md) | 简体中文

## 特性

- 🚀 **直接查询 Maven 仓库** - 无需 Gradle 运行时，纯 Rust 性能
- 📦 **多仓库支持** - Maven Central、Google Maven、自定义仓库，智能过滤
- 🎯 **智能版本检测** - 语义化版本控制，稳定性过滤（alpha、beta、RC、dev）
- 📋 **三个命令**：
  - `check` - 查看可用更新但不应用
  - `update` - 应用依赖更新
  - `list` - 以 Maven 坐标格式显示所有依赖
- 🔒 **版本引用支持** - 处理 `[versions]` 表并自动解析
- 🎨 **美观的 CLI 输出** - 进度条、彩色输出、清晰的摘要
- ⚡ **智能请求优化** - 基于组模式的仓库过滤，最小化 HTTP 请求

## 前置要求

- Rust 1.70+（用于构建）
- 使用版本目录的 Gradle 项目（`gradle/libs.versions.toml`）
- Git（可选，用于分支/提交功能）
- 互联网连接（用于查询 Maven 仓库）

## 安装

### 从源码安装

```bash
cargo install --path .
```

或手动构建：

```bash
cargo build --release
# 二进制文件位于 target/release/gvc
```

## 使用

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

### 列出依赖

以 Maven 坐标格式显示所有依赖（用于验证）：

```bash
gvc list
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

### 更新依赖

应用依赖更新（默认仅更新稳定版本）：

```bash
gvc update
```

#### 选项

- `--stable-only` - 仅更新到稳定版本（默认启用）
- `--no-stable-only` - 允许更新到不稳定版本（alpha、beta、RC）
- `--no-git` - 跳过 Git 操作（不创建分支/提交）
- `--path`, `-p` - 指定项目目录

**示例：**

```bash
# 仅更新到稳定版本（默认行为）
gvc update

# 包含不稳定版本（alpha、beta、RC）
gvc update --no-stable-only

# 不使用 Git 集成更新
gvc update --no-git

# 更新特定项目
gvc update --path /path/to/project
```

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

## 许可证

Apache-2.0

## 贡献

欢迎贡献！请随时提交 Pull Request。

## 路线图

- [ ] 异步 HTTP 请求以实现并发版本查询
- [ ] Maven 元数据的本地缓存
- [ ] 交互式 TUI 模式以进行选择性更新
- [x] 支持 Gradle 插件更新（集成 Gradle Plugin Portal）✅
- [ ] 配置文件支持（`.gvcrc`）
- [ ] 更好的错误消息和建议
