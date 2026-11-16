# Script Hub

使用 Tauri 2、React、TypeScript 和 Vite 构建的本机任务管理桌面应用，以 Windows 为首版验收目标。

- 任务新增、编辑、复制、删除和搜索；完整命令与组合编辑两种方式。
- 自动识别任务输入变量，提供最终命令、工作目录及环境覆盖预览。
- 任务启动、停止、重启、任务自动启动；每个任务同时最多一个运行实例。
- 实时区分标准输出与错误输出，支持复制、筛选、清空、历史查看及启动配置快照。
- 导入任意普通文件，保留独立副本，支持引用保护、原路径替换和打开所在目录。
- 用户可编辑的版本化 JSON，外部修改冲突处理、有效备份及损坏恢复。
- 单实例、系统托盘、关闭窗口隐藏、明确退出时停止任务树；可选应用开机启动。

产品行为依据 [设计记录](docs/design.md)、[领域术语](GLOSSARY.md) 和 [ADR](docs/adr/)。配置字段、底层执行机制及验收方法见 [实现与验收说明](docs/implementation.md)。

## 开发

需要 Node.js 22.18+、pnpm 12.15.0、Rust 稳定版，以及当前系统的 Tauri 原生依赖。Windows 需要 Microsoft C++ Build Tools 和 WebView2；Linux 桌面运行需要图形会话与系统托盘支持。

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

仅预览界面：

```sh
pnpm dev
```

前端默认地址为 `http://localhost:1420`。浏览器模式会明确显示预览提示，本机命令、文件与配置操作需要通过桌面应用使用。

## 检查与构建

```sh
# 前端领域测试
pnpm test

# TypeScript 检查和前端生产构建
pnpm build

# Rust 编译、测试和静态检查
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings

# 格式检查
pnpm format:check
cargo fmt --manifest-path src-tauri/Cargo.toml --check

# 在目标系统构建桌面应用及安装包
pnpm tauri build
```

安装包输出位于 `src-tauri/target/release/bundle/`。GitHub Actions 在 Windows 与 Linux 上执行前端构建、领域测试和 Rust 检查；桌面交互的人工验收清单位于实现说明中。

## 数据

数据根目录固定为用户主目录下的 `.script-hub/`：

```text
.script-hub/
  config.json             # 应用设置、任务、输入变量、文件引用
  config.backup.json      # 上一份有效配置
  runs.json               # 独立的运行状态与配置快照
  files/<文件ID>/<原文件名>
  workspaces/<任务ID>/
  logs/<运行实例ID>.jsonl
```

每个任务保留最近 20 次运行记录，每次最多 10 MB 日志。输入变量的赋值按普通文本保存；不提供敏感变量类型。手动修改配置后，请先停止全部任务，再在应用设置中重新加载。

## 源码

- `src/App.tsx`：任务、文件、设置和运行输出界面。
- `src/domain.ts`：前端类型、变量识别与日志读取。
- `src-tauri/src/model.rs`：配置模型、校验与命令生成。
- `src-tauri/src/storage.rs`：原子写入、备份、冲突及恢复。
- `src-tauri/src/hub.rs`：运行生命周期、历史、日志与文件管理。
- `src-tauri/src/process.rs`、`windows_process.rs`：平台进程管理。
- `src-tauri/src/lib.rs`：Tauri 命令、托盘、单实例和应用开机启动。

当前应用标识为 `com.scripthub.app`；发布前请按实际归属修改。
