# AI 图片生成（ai-image）

跨平台 AI 图片生成应用，基于 **Tauri 2 + Vue 3 + TypeScript** 构建。一套代码覆盖 Windows、macOS、Linux 桌面端与 Android 移动端。

## 功能特性

### 图片生成
- **文生图**：输入提示词直接生成图片
- **图生图**：添加参考图辅助生成（最多 16 张，支持点击添加 / Ctrl+V 粘贴 / 拖拽导入）
- **参数调节**：
  - 生成数量 1~10 张，按供应商并发能力并行生成
  - 比例选择（自动 / 1:1 / 4:3 / 3:4 / 16:9 / 9:16 / 3:2 / 2:3）
  - 像素档位 1K / 2K / 4K（16px 对齐，面积等比换算）
  - 质量三档（低 / 中 / 高）
- **多供应商管理**：内置两个预置供应商，可新增、编辑、删除、切换激活；支持"是否接受 response_format"兼容开关与并发数配置

### 下载保存
- 保存格式 **PNG / JPG** 可选，JPG 压缩质量可调（50%~100%）
- **自定义保存目录**：桌面端可固定目录，也可留空在首次保存时弹出系统目录选择框
- **批量下载**：一键保存当前全部结果
- **平台自适应**：
  - Windows / macOS / Linux：写入所选目录文件
  - Android：自动保存到系统相册 `Pictures/ai-image` 相册集（经 MediaStore 写入，Android 10+ 无需存储权限）

### 其他
- **中文运行日志**：界面内可展开查看，完整日志落盘于程序同目录 `logs/ai-image.log`（含请求地址、HTTP 状态码、失败响应摘要）
- **本地配置持久化**：API 地址、密钥、保存偏好等全部保存在本机，不上传任何数据
- 生成结果支持复制到剪贴板、单张下载、批量下载

## 平台支持

| 平台 | 状态 | 说明 |
|---|---|---|
| Windows 10+ | ✅ | WebView2 运行时（Win10 22H2 一般已内置） |
| macOS | ✅ | 需在 macOS 机器上构建 |
| Linux | ✅ | 需 webkit2gtk-4.1 |
| Android | ✅ | 需 Android SDK / NDK，可在 Windows 上构建 APK |

> 桌面端不支持交叉打包：macOS 包需在 macOS 上构建（推荐用 CI）。

## 开发环境要求

- **Node.js 20+**（项目根已带 `.mise.toml`，用 [mise](https://mise.jdx.dev/) 管理会自动使用 node 20）
- **Rust 1.77.2+**（同样可用 mise 管理）
- **Windows 编译额外要求**：Visual Studio Build Tools 2022（含"使用 C++ 的桌面开发"工作负载与 Windows SDK）
- **Android 构建**：Android Studio + SDK + NDK，以及 Rust 的 `aarch64-linux-android` 等目标平台

网络加速已在项目内配置（不影响全局）：`.npmrc` 指向 npmmirror，`.cargo/config.toml` 指向 rsproxy。

## 常用命令

```bash
# 安装前端依赖
npm install

# 开发模式（桌面）
npm run tauri dev

# 构建桌面安装包（NSIS / MSI / dmg / AppImage 按平台产出）
npm run tauri build

# 仅构建可执行文件（不打包安装器）
cargo build --release   # 在 src-tauri 目录下执行，产物位于 target/release/

# Android 首次初始化（需要 NDK 就绪后执行一次）
npm run tauri android init

# Android 开发 / 构建
npm run tauri android dev
npm run tauri android build
```

开发模式固定访问 `http://127.0.0.1:1420`（vite 已固定监听 IPv4 回环，规避 WebView2 走 IPv4 连不上 `[::1]` 的兼容问题）。

## 项目结构

```
ai-image/
├── src/                     # Vue 3 前端
│   ├── App.vue              # 主界面（生成 / 结果 / 设置 / 日志）
│   └── lib/api.ts           # 设置持久化、尺寸计算、格式转换、保存封装
├── src-tauri/               # Tauri 应用（Rust）
│   └── src/lib.rs           # generate_image / edit_image / save_image 命令
├── plugins/
│   └── tauri-plugin-gallery/  # 相册保存插件（Android MediaStore）
│       ├── src/             # Rust 封装（desktop/mobile 分层）
│       └── android/         # Kotlin 实现（写入 Pictures/ai-image）
├── .npmrc                   # 前端依赖镜像
├── .cargo/config.toml       # crates 下载镜像
└── .mise.toml               # node 版本（20）
```

## 日志

完整运行日志位于程序同目录：

```
logs/ai-image.log
```

内容包括：每次生成请求的接口地址与参数、HTTP 状态码、失败时的响应原文摘要（截断保存）、图片保存路径等，便于定位接口问题。

## 版本历史

- **0.2.0**
  - 新增 Android 相册保存插件（`tauri-plugin-gallery`，保存命令按平台自动分流）
  - 保存设置完善：PNG/JPG 格式选择、JPG 质量调节、自定义保存目录、批量下载
  - 修复开发模式下 WebView2 无法访问 dev server 的地址监听问题
- **0.1.0**
  - 首个版本：文生图 / 图生图、多供应商管理、参数化生成、结果展示与下载、中文运行日志
