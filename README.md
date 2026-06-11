# GitViz - Git 仓库可视化工具

一个基于终端的 Git 仓库可视化与分析工具，使用 Rust 编写。

## 功能特性

- **仓库概览**：提交次数、贡献者、时间跨度、语言分布
- **提交时间线**：浏览提交历史，支持搜索
- **贡献者统计**：提交次数排行榜，可视化柱状图
- **文件热点**：识别变更最频繁的文件/目录
- **分支图谱**：分支列表与合并点可视化

## 演示

```
gitviz /path/to/repo
```

## 安装

```bash
cargo build --release
./target/release/gitviz /path/to/repo
```

## 使用方法

```
gitviz [选项] [仓库路径]

参数:
  [仓库路径]  Git 仓库路径 [默认: .]

选项:
  -p, --path <路径>  Git 仓库路径 [默认: .]
  -h, --help         显示帮助
  -V, --version      显示版本
```

## 快捷键

| 按键 | 操作 |
|------|------|
| `h` / `←` | 上一个标签页 |
| `l` / `→` | 下一个标签页 |
| `j` / `↓` | 向下滚动 |
| `k` / `↑` | 向上滚动 |
| `1`-`5` | 跳转到指定标签页 |
| `g` | 滚动到顶部 |
| `G` | 滚动到底部 |
| `/` | 搜索模式 |
| `Enter`/`Esc` | 退出搜索 |
| `q` / `Esc` | 退出 |

## 架构

```
src/
├── main.rs              # 入口，CLI 解析，TUI 启动
├── models/              # 数据结构
│   ├── git_object.rs    # Git 对象类型（Commit、Tree、Blob 等）
│   └── stats.rs         # 分析结果类型
├── git_parser/          # Git 仓库解析
│   ├── object_reader.rs # 读取并解压 .git/objects（松散对象 + 包文件）
│   ├── commit_parser.rs # 解析提交对象，SHA1 计算
│   ├── tree_parser.rs   # 解析树对象，语言检测
│   └── repo.rs          # 仓库抽象，分支/引用解析
├── analyzer/            # 统计分析
│   ├── stats.rs         # 仓库分析（概览、贡献者、热点、分支）
│   └── diff.rs          # Diff 计算，LCS 算法
└── tui_ui/              # 终端 UI
    ├── app.rs           # 应用状态，视图模式，滚动
    ├── event.rs         # 键盘事件处理
    └── render.rs        # Ratatui 各视图渲染
```

## 展示的 Rust 特性

- **所有权/借用**：分析管道中的共享引用，解析中的所有权转移
- **结构体/枚举**：Git 对象建模（`GitObjectType`、`Commit`、`Tree`），视图模式
- **特征**：`Repository` 特征抽象，`Display`/`FromStr` 实现
- **泛型**：`longest_common_subsequence_len<T: PartialEq>`，泛型 `Result<T>` 类型别名
- **错误处理**：自定义 `GitVizError` 枚举配合 `thiserror`，`Result` 传播
- **并发**：`rayon` 并行提交解析
- **底层 I/O**：直接读取 `.git/objects`，通过 `flate2` 进行 `zlib` 解压
- **SHA1 实现**：纯 Rust SHA1 用于提交哈希验证

## 依赖

| 库 | 用途 |
|----|------|
| `ratatui` | 终端 UI 框架 |
| `crossterm` | 跨平台终端控制 |
| `flate2` | Git 对象的 zlib 解压 |
| `chrono` | 日期/时间处理 |
| `thiserror` | 错误类型派生宏 |
| `rayon` | 数据并行 |
| `clap` | CLI 参数解析 |
| `anyhow` | 顶层错误处理 |

## 测试

```bash
cargo test
```
