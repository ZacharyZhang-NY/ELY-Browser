# PRD.md — ELY Browser by Elydora

文档日期：2026-05-07  
状态：完整产品规格  
产品名称：ELY Browser  
品牌归属：Elydora  
产品形态：跨平台 Native Rust 桌面浏览器  
核心技术：Rust、GPUI、Servo WebView、gpui-component、awesome-gpui 生态组件  
云端基础设施：Cloudflare Workers、Cloudflare D1、Cloudflare R2、Cloudflare Workers KV、Better Auth  
核心能力：垂直标签页、Spaces、Profiles、Split View、登录、端到端加密 Sync、自有插件系统、隐私与性能控制  
产品定位：clean browser；默认安静、界面克制、行为透明、数据可控

---

## 1. 产品结论

ELY Browser by Elydora 是一款纯净、快速、低打扰的 Native Rust 桌面浏览器。它以左侧垂直标签页作为默认信息架构，以 Spaces 承载任务上下文，以 Profiles 隔离账号、Cookie、历史与站点权限，以 Split View 支持同窗多网页并排工作，以端到端加密 Sync 保持多设备状态一致，以自有 `.rplug` Wasm 插件系统提供可审计、可撤销、可签名发布的扩展能力。

浏览器引擎使用 Servo。桌面 Shell 使用 GPUI。主组件库使用 gpui-component，并通过 awesome-gpui 生态中的 adabraka-ui、ferrum-flow、gpui-flow、gpui-form、gpui-hooks、gpui-nav、gpui-router、gpui-storybook、gpui-symbols、gpui-d3rs、gpui-px、gpui-video-player、plotters-gpui 完成完整桌面产品能力建设。

登录和 Sync 使用 Elydora Cloud，运行在 Cloudflare Workers。Better Auth 负责身份、账号、会话、OAuth、设备登录与会话安全；Cloudflare D1 负责结构化账号数据、设备索引、Sync 对象索引、变更日志、插件注册表元数据；Cloudflare R2 负责加密后的大对象、Sync 快照、头像、插件包、崩溃附件和导出文件；Cloudflare Workers KV 负责读多写少的全局配置、会话缓存、插件市场索引缓存、公开密钥缓存、更新清单缓存和短期同步游标缓存。

产品对标方向是 Zed 式 Native Rust/GPUI 高性能软件体验、Arc 的 Spaces / Profiles / Favorites / Split View / Auto Archive、Dia 的清爽任务表面与分屏入口、Zen Browser 的垂直标签与工作区、Vivaldi 的深度标签管理、Edge 与 Chrome 的主流化垂直标签体验。产品边界始终围绕 clean browser：界面克制、用户可控、隐私优先、插件安全、默认安静。

完整交付边界：

- 桌面端 macOS、Windows、Linux 原生安装包、签名、自动更新、崩溃恢复。
- Servo WebView 嵌入、导航、输入、缩放、截图、站点权限、下载、历史、书签、阅读模式、DevTools 入口。
- 左侧垂直标签系统、Spaces、Profiles、Favorites、Pinned Tabs、Tab Groups、Split View、自动归档、会话恢复。
- 登录、设备管理、端到端加密 Sync、本地加密数据库、冲突处理、离线可用。
- 自有 `.rplug` 插件系统、Wasm Component Model、WIT 接口、权限弹窗、插件管理器、插件市场、签名发布、审计日志。
- GPUI 设计系统、Storybook、设置中心、命令面板、键盘快捷键、菜单栏、托盘/通知、无障碍、国际化。
- Cloudflare Workers API、Better Auth、D1 schema、R2 bucket、KV namespace、密钥轮换、备份、监控、限流。
- 浏览器安全模型、隐私控制、遥测最小化、站点兼容性面板、性能任务管理器、发布验收矩阵。

本 PRD 只定义完整产品形态和完整实现要求。研发任务可以并行拆解，但产品规格本身保持单一完整交付边界。

---

## 2. Deep Research 依据

### 2.1 垂直标签页已经进入主流浏览器体验

Google Chrome 在 2026-04-07 发布的产品更新中推出“Show Tabs Vertically”，官方说明侧边标签可以读取完整页面标题并更容易管理标签组，尤其适合双位数标签场景。[R1]

Microsoft Edge 官方将 Vertical Tabs 定义为让用户在侧边栏中更容易扫描、组织和管理大量标签页的功能，覆盖 Windows 与 macOS。[R2]

Vivaldi 长期提供 Workspaces、Tab Stacks 与 Tab Tiling。官方说明 Workspaces 可以按类别分组标签和标签堆，Tab Tiling 可以把多个标签平铺到同一窗口内，减少反复切换。[R5][R6]

结论：垂直标签本身已经成为主流浏览器能力。ELY 的差异需要落在“垂直标签 + 任务空间 + 隐私同步 + 原生性能 + 插件安全 + 低打扰体验”的组合上。

### 2.2 Arc、Dia、Zen、Vivaldi 的可采纳体验

Arc 的 Favorites 是跨所有 Spaces 常驻的顶部标签；Auto Archive 会按用户设置清理 idle unpinned tabs，并支持按 Profile 调整归档时机。[R3][R4]

Vivaldi 的 Workspaces 让用户切换工作区时只看到该类别下的标签；Tab Tiling 支持垂直、水平、网格和自定义布局，并支持拖拽调整。[R5][R6]

Dia 的产品体验可作为清爽任务表面和分屏入口的参考：导航区尽量轻，内容区优先，分屏操作应在当前任务中就地完成。ELY 采纳其布局克制和任务连续性，产品能力仍保持 clean browser 边界。

结论：ELY 的 clean 浏览体验需要把 Arc 的上下文模型、Dia 的低噪声表面、Zen 的侧栏优先和 Vivaldi 的深度标签管理统一成一套更克制的 Native Rust 桌面交互。

### 2.3 GPUI 与 Native Rust 桌面 Shell

GPUI 官方定义为来自 Zed 创建者的快速、高生产力 Rust UI 框架；docs.rs 将 GPUI 描述为 hybrid immediate/retained mode、GPU accelerated 的 Rust UI 框架。[R7][R8]

Zed 团队公开说明，Zed 以类似游戏渲染管线的方式使用 Rust 和 GPU 构建高响应 UI；这为 ELY 的浏览器 Shell 提供性能和交互基准。[R9]

gpui-component 提供 60+ 跨平台桌面 UI 组件，包含虚拟化 Table/List、Dock 布局、主题系统、输入控件、弹窗、菜单等能力，适合作为浏览器设置页、侧栏、弹窗、列表、下载管理、历史记录、插件管理器的主组件库。[R10]

awesome-gpui 收录了 GPUI 应用、组件库、路由、表单、Hooks、图表、Storybook、符号、视频播放器和 Plotters 后端等生态项目。指定库均可映射到浏览器的实际产品模块。[R11]

结论：GPUI 承担原生 Shell，Servo 承担网页渲染，gpui-component 承担主要 UI 组件，awesome-gpui 生态补齐路由、表单、可视化、Storybook、图标、视频、图表与状态组织。

### 2.4 Servo 嵌入能力与约束

Servo 是 Rust 编写的 Web 渲染引擎，支持 WebGL/WebGPU，面向桌面、移动和嵌入式，并提供 WebView API 让应用嵌入 Web 内容。[R12]

Servo 在 2026-04-13 发布 `servo` crate，官方说明这是第一个允许 Servo 作为 library 使用的 crates.io release，同时说明该 release 仍处在 `1.0` 前，embedding API 仍会演进，并提供 LTS 轨道承接安全修复与迁移指导。[R13]

结论：ELY 需要把 Servo 当作浏览器引擎子系统集成。GPUI 与 Servo 之间需要专门的 WebView Host、输入桥、渲染桥、焦点桥、IME 桥、权限桥、下载桥和崩溃恢复桥。

### 2.5 Cloudflare + Better Auth 适合登录与 Sync 后端

Cloudflare D1 是托管 serverless SQL database，使用 SQLite 语义，支持 Workers 和 HTTP API 访问，并提供 Time Travel 与 read replication 能力。[R14]

Cloudflare 官方数据产品说明中，D1 适合持久化、关系型用户数据和账号数据；R2 是 S3-compatible blob storage，适合大对象和静态资源，并提供 per-object strong consistency；Workers KV 是 eventually consistent 的低延迟键值存储，适合高读取量配置、会话和分布式配置。[R15]

Workers KV 的一致性模型是 eventually consistent，跨地区可见性可能需要 60 秒或更久，因此 ELY 把 KV 用于缓存和读多写少数据，把 D1 作为结构化事实源，把 R2 作为加密大对象事实源。[R16]

Better Auth 1.5 已将 Cloudflare D1 作为 first-class database option，允许直接传入 D1 binding；官方也说明 D1 不支持 interactive transactions，Better Auth 使用 D1 `batch()` API 处理原子性。[R17]

第一版需要支持 Google，Github，Email OTP signin。

D1 当前单数据库上限为 Workers Paid 10 GB / Free 500 MB，账号级上限和数据库数量需要按 Cloudflare 限制设计；ELY 的 Sync schema 因此按账户、地区和对象类型预留拆分策略。[R18]

结论：ELY Cloud 使用 Cloudflare Workers + Better Auth + D1/R2/KV 可覆盖登录、设备、安全会话、同步索引、加密对象存储、插件市场和发布资产，同时需要严格区分事实源、缓存和对象存储。

### 2.6 自有插件系统采用 Wasm Component Model

WebAssembly Component Model 提供可移植、跨语言组合的组件架构，并使用 WIT 描述组件与 host 之间的接口；Wasmtime 提供 Component Model 嵌入 API。[R19][R20]

Wasmtime 安全文档说明 WebAssembly 的目标之一是以沙箱方式运行不受信代码，Wasm 默认需要显式 import 才能获得 host 能力。[R21]

结论：ELY 插件系统采用 `.rplug` 包、`plugin.toml` 清单、WIT 接口、Wasmtime Host、能力授权和签名发布。插件 API 完全由 ELY 定义，面向浏览器 UI、标签、书签、历史、下载、命令、侧栏、页面桥接和设置扩展。

---

## 3. 产品定位

### 3.1 目标用户

核心用户是长期依赖浏览器完成知识工作的人群，包括软件工程师、产品经理、设计师、研究人员、运营、创作者、学生和重度网页应用用户。共同特征是常开 20–200 个标签页，经常在项目、账号、文档、工具、资料之间切换，并需要低干扰、可恢复、可同步的浏览环境。

### 3.2 价值主张

用户通过左侧垂直工作台管理大量网页，通过 Spaces 将标签按任务上下文聚合，通过 Profiles 分离工作/个人/客户/项目账号，通过 Split View 在同一窗口并排处理资料，通过端到端加密 Sync 在多设备间恢复状态，通过自有插件系统扩展工作流，同时保持界面清爽、动作透明和数据可控。

### 3.3 产品原则

- Clean first：默认界面克制，空白、层级、动效、通知和入口数量受控。
- Local first：标签、历史、书签、会话、设置、插件配置先写本地数据库，云端承担同步与备份。
- Privacy first：Sync 数据端到端加密，遥测默认最小化，敏感数据使用系统钥匙串保存。
- Keyboard first：核心浏览动作均可通过命令面板和快捷键完成。
- Vertical first：垂直标签页是默认信息架构，顶部区域只保留地址栏、命令入口和必要状态。
- Profile strict：Cookie、Storage、历史、权限、证书例外、下载策略按 Profile 明确隔离。
- Plugin safe：插件通过显式权限、Wasm 沙箱、签名、审计、资源限额和可撤销授权运行。
- Engine honest：Servo 兼容性以可见状态、反馈入口和站点诊断面板展示。
- Desktop native：窗口、菜单、快捷键、拖拽、文件、通知、系统主题、IME 和无障碍遵循平台习惯。

---

## 4. 品牌与命名

产品正式名称为 ELY Browser by Elydora。界面中主名称使用 “ELY Browser”，品牌页、关于页、法律页和安装包发行方使用 “Elydora”。短名称使用 “ELY”。

命名规范：

- App name：ELY Browser
- Company / brand：Elydora
- Short display name：ELY
- macOS bundle id：`com.elydora.ely-browser`
- Windows app id：`Elydora.ELYBrowser`
- Linux desktop id：`com.elydora.ely-browser.desktop`
- Sync service name：Elydora Cloud
- Plugin package suffix：`.rplug`
- Plugin registry name：Elydora Plugin Registry
- Protocol handler：`ely://`
- Auth callback：`ely://auth/callback`
- Plugin deep link：`ely://plugin/<plugin_id>`
- Settings deep link：`ely://settings/<section>`

视觉方向：ELY 使用冷静、轻量、低饱和的桌面应用气质。品牌表达偏专业，不使用过度拟物、重渐变和高频动效。默认主题遵循系统浅色/深色，允许用户设置 Space 级强调色。

---

## 5. 用户画像与关键场景

### 5.1 软件工程师

工程师在浏览器中频繁打开 GitHub、GitLab、Linear、Jira、文档、本地开发地址、日志平台和云控制台。浏览器需要把一个项目的所有网页放在同一 Space 中，并允许使用 Split View 同时查看本地页面、PR、接口文档和日志。

关键需求：

- 每个项目一个 Space，每个客户或公司账号一个 Profile。
- Localhost、PR、Issue、文档、Dashboard 可固定在同一 Space。
- 地址栏支持命令、URL、书签、历史和打开标签搜索。
- 标签支持按域名、项目、标题规则自动归组。
- 下载、证书例外、站点权限与 Profile 绑定。
- 插件可提供本地开发面板、API 收藏、Mock 切换、路由查看、日志链接解析。

### 5.2 产品经理 / 项目管理者

产品经理需要在任务系统、文档、表格、会议记录、设计稿和数据看板间切换。浏览器需要减少标签噪声，并保留每个项目上下文。

关键需求：

- Space 支持图标、颜色、顺序、固定页、分组、自动归档。
- Split View 支持文档 + 看板、需求 + 设计、表格 + 数据页。
- 书签支持项目级集合，历史支持按 Space 过滤。
- 同步可恢复其他设备上未完成的工作空间。
- 插件可扩展项目看板入口、状态徽标、页面右键操作。

### 5.3 UI/UX 设计师

设计师需要浏览竞品、Figma、文档、原型、视频素材和评论。浏览器需要更好的分屏、截图、媒体预览和页面信息收集。

关键需求：

- Split View 支持最多四个网页区域，并可保存为复合标签。
- 侧栏支持页面截图、颜色采样、字体信息、可访问性摘要。
- 内置媒体预览支持视频文件和网页下载内容。
- 工作区图谱可显示竞品页、参考页、设计稿与备注之间的关系。
- 插件可扩展页面审查、截图导出、链接收集和设计资源标记。

### 5.4 研究人员 / 内容创作者

研究人员需要保存大量资料、阅读长文、管理引用、记录来源并跨设备继续阅读。浏览器需要高质量阅读模式、书签集合、摘录、历史搜索和 Sync。

关键需求：

- 阅读模式保留标题、作者、时间、正文、图片、链接来源。
- 书签集合支持标签、备注、排序、导出。
- 历史搜索支持域名、标题、正文索引和 Space 过滤。
- 侧栏支持当前页面备注和摘录。
- 插件可扩展文献管理、RSS、导出器和外部笔记系统。

---

## 6. 信息架构

```text
ELY Browser
├─ Identity
│  ├─ Account
│  ├─ Sessions
│  ├─ Devices
│  ├─ Recovery Keys
│  └─ Sync Vault
├─ Windows
│  └─ Browser Window
│     ├─ Title Bar / Traffic Controls
│     ├─ Command Bar
│     ├─ Vertical Workspace
│     │  ├─ Favorites
│     │  ├─ Spaces
│     │  ├─ Tab Groups
│     │  ├─ Pinned Tabs
│     │  ├─ Unpinned Tabs
│     │  └─ Archived Tabs
│     ├─ WebView Canvas
│     │  ├─ Single WebView
│     │  ├─ Split View
│     │  └─ Error / Recovery View
│     ├─ Side Panels
│     │  ├─ Bookmarks
│     │  ├─ History
│     │  ├─ Downloads
│     │  ├─ Reading List
│     │  ├─ Notes
│     │  ├─ Plugin Panels
│     │  └─ Task Manager
│     └─ Status Bar
├─ Browser Data
│  ├─ Profiles
│  ├─ Site Data
│  ├─ Permissions
│  ├─ Certificates
│  ├─ Bookmarks
│  ├─ History
│  ├─ Downloads
│  ├─ Reading List
│  ├─ Notes
│  └─ Sessions
├─ Plugin System
│  ├─ Plugin Registry
│  ├─ Wasm Host
│  ├─ Permission Broker
│  ├─ UI Contributions
│  ├─ Page Bridge
│  ├─ Package Store
│  ├─ Signature Verifier
│  └─ Audit Log
├─ Elydora Cloud
│  ├─ Cloudflare Workers API
│  ├─ Better Auth
│  ├─ Cloudflare D1
│  ├─ Cloudflare R2
│  ├─ Cloudflare Workers KV
│  └─ Observability
└─ Platform Services
   ├─ Keychain
   ├─ Notifications
   ├─ Menus
   ├─ Clipboard
   ├─ File Picker
   ├─ Protocol Handler
   ├─ Auto Update
   └─ Accessibility
```

---

## 7. 核心 UI/UX

### 7.1 主窗口布局

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ ELY  [Space: Work ▾]  [Search or enter address........................]  ⋯  │
├───────────────┬──────────────────────────────────────────────────────────────┤
│ ★ Favorites   │                                                              │
│  ○ Mail       │                                                              │
│  ○ Calendar   │                                                              │
│  ○ Docs       │                                                              │
│───────────────│                                                              │
│ Work          │                                                              │
│  ▾ Project A  │                                                              │
│    ● PR #241  │                      Servo WebView Canvas                    │
│    ● API Docs │                                                              │
│    ● Logs     │                                                              │
│  ▸ Design     │                                                              │
│───────────────│                                                              │
│ Spaces        │                                                              │
│  Work         │                                                              │
│  Personal     │                                                              │
│  Research     │                                                              │
│───────────────│                                                              │
│ + New Tab     │                                                              │
└───────────────┴──────────────────────────────────────────────────────────────┘
```

布局要求：

- 左侧侧栏默认宽度 280px，可收缩到 56px 图标态。
- 顶部地址栏固定在内容区顶部，避免顶部标签挤压。
- 垂直标签列表使用虚拟化渲染，支持 1,000 个标签对象仍保持流畅滚动。
- 当前标签、悬浮标签、未读变化、加载中、错误态、静音态、固定态、归档态都有清晰视觉标识。
- 在全屏和专注模式下，侧栏可自动隐藏，鼠标靠近左缘或快捷键唤起。
- 所有浏览动作均可通过命令面板执行。

### 7.2 Split View

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ [Address / Command Bar]                                         [Split ▾]   │
├───────────────┬─────────────────────────────┬────────────────────────────────┤
│ Vertical Tabs │                             │                                │
│               │        WebView A            │          WebView B             │
│               │                             │                                │
│               ├─────────────────────────────┴────────────────────────────────┤
│               │  Split Controls: swap | duplicate | detach | save | close    │
└───────────────┴──────────────────────────────────────────────────────────────┘
```

Split View 要求：

- 支持 2、3、4 个 WebView。
- 支持横向、纵向、网格和自定义拖拽布局。
- Split View 作为一个复合标签保存在垂直侧栏中。
- 复合标签可固定、归档、移动到其他 Space、参与 Sync。
- 每个子 WebView 独立导航、刷新、下载、站点权限和错误恢复。
- 地址栏对当前聚焦子 WebView 生效。
- 链接右键支持“在 Split View 中打开”。
- 标签拖入内容区边缘时显示 Split Drop Target。

### 7.3 Command Bar

Command Bar 是 ELY 的统一入口，支持 URL、搜索、命令、书签、历史、打开标签、设置项、插件命令。

输入行为：

- `github.com`：导航到 URL。
- `? rust async book`：使用默认搜索引擎。
- `>`：进入命令模式。
- `@tabs`：只搜已打开标签。
- `@bookmarks`：只搜书签。
- `@history`：只搜历史。
- `@settings`：只搜设置项。
- `@plugins`：只搜插件命令。

命令示例：

- `New Space`
- `Switch Profile`
- `Open Downloads`
- `Clear Site Data for This Profile`
- `Save Split View`
- `Move Tab to Space`
- `Archive Idle Tabs`
- `Install Plugin from File`
- `Open Sync Status`

### 7.4 Settings

Settings 使用 `gpui-router` 管理路由，使用 `gpui-form` 管理表单，使用 gpui-component 提供 Tabs、Form、Switch、Select、Table、Dialog、Toast、List、Sidebar、Search。

设置页结构：

```text
Settings
├─ General
├─ Appearance
├─ Sidebar & Tabs
├─ Spaces
├─ Profiles
├─ Search
├─ Privacy & Security
├─ Sync
├─ Downloads
├─ Site Permissions
├─ Plugins
├─ Shortcuts
├─ Updates（随自动更新子系统交付；无更新器时不得展示空策略页）
├─ Advanced（并入所属分区；仅当出现无归属的真实高级设置时恢复）
└─ About ELY Browser
```

设置页要求：

- 每个设置项必须有即时校验、保存状态、恢复默认值。
- 参与 Sync 的设置必须显示同步状态。
- 高风险设置必须二次确认。
- 设置搜索支持标题、说明、关键词、快捷键。
- About 页展示 ELY Browser、Elydora、构建号、Servo 构建信息、GPUI 构建信息、许可证入口。

---

## 8. 浏览器功能规格

### 8.1 垂直标签系统

标签对象字段：

| 字段 | 说明 |
|---|---|
| `tab_id` | 本地唯一 ID |
| `space_id` | 所属 Space |
| `profile_id` | 所属 Profile |
| `parent_tab_id` | opener 来源 |
| `title` | 页面标题 |
| `url` | 当前 URL |
| `display_url` | 脱敏展示 URL |
| `favicon_key` | favicon 本地缓存 key |
| `state` | loading / ready / crashed / discarded / archived |
| `is_pinned` | 是否固定 |
| `is_favorite` | 是否跨 Space 常驻 |
| `group_id` | 所属 Tab Group |
| `split_id` | 所属 Split View |
| `last_active_at` | 最近使用时间 |
| `created_at` | 创建时间 |
| `sort_key` | CRDT ordered list 排序键 |
| `sync_enabled` | 是否参与 Sync |

标签行为：

- 新标签默认插入当前标签下方。
- 从当前页面打开的新标签保留 opener 关系。
- 支持拖拽排序、拖拽到 Space、拖拽成 Split View、拖拽成 Tab Group。
- 支持批量选择、批量移动、批量归档、批量关闭。
- 支持搜索当前窗口、当前 Space、全部窗口中的打开标签。
- 支持标签休眠，休眠后释放 Servo WebView 资源，仅保留会话数据。
- 支持崩溃恢复，崩溃标签保留 URL、标题、favicon 和表单恢复提示。

### 8.2 Spaces

Space 是任务上下文。每个 Space 拥有自己的标签、固定区、分组、自动归档策略、主题强调色、默认 Profile 和侧栏状态。

Space 字段：

| 字段 | 说明 |
|---|---|
| `space_id` | 全局唯一 ID |
| `name` | 名称 |
| `icon` | 图标 |
| `accent_color` | 强调色 |
| `default_profile_id` | 默认 Profile |
| `archive_policy` | 自动归档策略 |
| `sidebar_width` | 侧栏宽度 |
| `sort_key` | 排序键 |
| `created_at` | 创建时间 |
| `updated_at` | 更新时间 |

Space 行为：

- Space 切换只改变可见标签集合，不强制打开新窗口。
- 一个窗口可以持有多个 Space。
- 同一 Space 可在多个窗口打开，但编辑冲突必须可恢复。
- Space 可导出为 `.elyspace` 文件。
- Space 可导入，导入时可选择是否保留 Profile 映射。
- Space 删除进入本地回收站，保留 30 天。

### 8.3 Profiles

Profile 是站点数据隔离边界。Profile 隔离 Cookie、Storage、Cache、History scope、Permissions、Certificates、Downloads policy、Search engine preference。

Profile 字段：

| 字段 | 说明 |
|---|---|
| `profile_id` | 全局唯一 ID |
| `name` | 名称 |
| `color` | 颜色 |
| `icon` | 图标 |
| `cookie_store_id` | Cookie 容器 |
| `history_policy` | 历史策略 |
| `download_policy` | 下载策略 |
| `permission_policy` | 权限策略 |
| `is_private` | 是否隐私 Profile |
| `sync_policy` | 同步策略 |

Profile 行为：

- 每个 Space 绑定一个默认 Profile。
- 单个标签可覆盖 Profile。
- Profile 切换需要明确展示视觉标识，避免用户在错误账号环境下操作。
- 隐私 Profile 关闭后清理 Cookie、Storage、Cache、临时下载索引和页面会话。
- Profile 可暂停 Sync。
- Profile 删除需要先展示受影响站点数据、历史、权限和下载策略。

### 8.4 Favorites / Pinned / Archived

Favorites 是跨所有 Spaces 常驻的顶部标签，适合邮箱、日历、任务系统、常用文档和音乐。Pinned Tabs 是 Space 内常驻标签。Archived Tabs 是被关闭或自动归档但可恢复的标签。

要求：

- Favorites 上限默认 12，可在设置中调整。
- Favorites 始终显示 favicon，悬浮显示标题和 Profile。
- Pinned Tabs 展示在当前 Space 顶部。
- Unpinned Tabs 按使用顺序和分组展示。
- 自动归档只作用于 Unpinned Tabs。
- 归档记录保留 URL、标题、favicon、Space、Profile、关闭时间、来源。
- 归档搜索支持域名、标题、Space、Profile、日期。

### 8.5 Tab Groups

Tab Group 用于 Space 内的二级组织。

能力：

- 支持名称、颜色、折叠、排序。
- 支持拖拽标签进出 Group。
- 支持 Group 内批量刷新、关闭、休眠、归档。
- 支持按域名自动归组。
- 支持把 Group 转成 Split View。
- 支持把 Split View 解散成 Group。

### 8.6 History

历史记录按 Profile 隔离，并可按 Space 建立上下文索引。

能力：

- 记录 URL、标题、访问时间、来源标签、Space、Profile、favicon、访问次数。
- 支持全文索引页面标题和 URL。
- 阅读模式页面支持正文索引，用户可关闭。
- 支持按域名清理、按时间清理、按 Profile 清理。
- 支持历史 Sync，默认关闭正文索引同步。
- 隐私 Profile 不写入持久历史。

### 8.7 Bookmarks / Reading List / Notes

书签支持集合、标签、备注和导入导出。Reading List 用于稍后阅读。Notes 是页面侧栏轻量备注。

能力：

- 书签集合可绑定 Space。
- 书签可保存页面截图缩略图。
- Reading List 保存阅读进度、页面标题、来源 URL、添加时间。
- Notes 绑定 URL 或具体标签对象。
- Notes 支持 Markdown 子集。
- 书签、Reading List、Notes 均参与端到端加密 Sync。

### 8.8 Downloads

下载管理器提供下载生命周期控制和安全提示。

能力：

- start / pause / resume / cancel / retry / open / reveal。
- 下载路径按 Profile 设置。
- 危险扩展名提示。
- 文件 checksum 可选计算。
- 下载历史可按 Profile 清理。
- 下载文件本体默认不参与 Sync，下载元数据可参与 Sync。
- 下载完成后可触发插件事件。

### 8.9 Site Permissions

站点权限按 Profile 隔离。

权限类型：

- Camera
- Microphone
- Screen capture
- Location
- Notifications
- Clipboard read
- Clipboard write
- Downloads
- Popups
- Autoplay
- WebUSB / WebHID / WebSerial
- Storage persistence
- Insecure content
- Certificate exception

要求：

- 权限弹窗显示站点、Profile、权限类型、风险说明。
- 用户可选择一次允许、始终允许、始终拒绝。
- 权限变更写入审计日志。
- Site Settings 可按站点查看所有授权。

### 8.10 Reading Mode

Reading Mode 提供低干扰阅读体验。

能力：

- 提取标题、作者、发布时间、正文、图片、链接。
- 支持字体、字号、行高、页面宽度、主题。
- 支持目录导航。
- 支持保存到 Reading List。
- 支持复制引用信息。
- 支持导出 Markdown。
- 支持按 Profile 和 Space 记录阅读进度。

### 8.11 Private Windows

Private Window 使用临时 Profile。

要求：

- 不写持久历史。
- 不持久化 Cookie、Storage、Cache。
- 下载文件保留在用户选择位置，下载索引关闭窗口后清理。
- 插件默认禁用，用户可为单个插件开启 Private Window 权限。
- Sync 在 Private Window 中默认禁用。

---

## 9. 登录与 Sync

### 9.1 总体原则

ELY 使用账户登录建立设备身份，使用端到端加密保护 Sync 内容。身份系统和 Sync 加密系统职责独立：Better Auth 确认“谁登录”，Sync Vault 决定“谁能解密”。Elydora Cloud 永远只保存密文 payload、索引、游标和必要元数据。

本地优先写入流程：

```text
User Action
  ↓
Local Encrypted Store
  ↓
Snapshot Merge + XChaCha20-Poly1305 Encryption
  ↓
Cloudflare Workers Sync API
  ↓
D1 Global Head + Vault Metadata + R2 Ciphertext
  ↓
Exact Head Token Download
  ↓
Decrypt + Merge + Apply
```

当前 Cloud Sync 数据面使用 encrypted snapshot v3。`/api/sync/push` 与 `/api/sync/pull` 保留为认证后的退役端点，固定返回 `410 sync_object_protocol_retired`。D1 是会话、设备、Vault、head 与 GC 状态的权威源；KV 承担公开配置和历史数据清理。

### 9.2 登录能力

登录入口：

- Email + password。
- Email OTP：5 分钟有效、3 次尝试、重发轮换、服务端加密存储。
- OAuth provider：当前支持按环境配置 Google、GitHub。
- 目标能力：Apple、Passkey、设备二维码登录、Recovery Key 恢复。

登录 UX：

- 首次打开 ELY 可跳过登录，浏览器完整离线可用。
- 登录后进入 Sync 设置向导。
- 用户可选择同步范围。
- 登录会话保存在系统钥匙串。
- 本地浏览器数据库密钥保存在系统钥匙串。
- 账号注销时可以选择保留本地数据或清理本地数据。

当前 Better Auth 职责：

- Email/password、Email OTP、可选 Google/GitHub OAuth。
- D1 authoritative bearer session，存储用户、账号、会话和验证记录。
- `first-primary` session validation 与 exact current-session revoke。
- ELY 自定义 device trust、Vault、Sync reset 与账号删除协议。
- `better_auth_session_device_context` 将会话绑定到经过证明的设备身份。

Desktop auth callback：

```text
ELY Browser → Open auth page in WebView or system browser
Auth completed on Elydora Cloud
Cloudflare Worker redirects to ely://auth/callback?code=...
ELY exchanges code for session
Session token stored in platform keychain
Sync setup starts inside ELY settings
```

### 9.3 Sync 数据对象

| 对象 | 默认同步 | 加密 | 冲突策略 |
|---|---:|---:|---|
| Account profile display info | 是 | 部分 | Last writer wins |
| Devices | 是 | 否 | Server authority |
| Spaces | 是 | 是 | CRDT ordered list |
| Tabs | 是 | 是 | CRDT ordered list + focus timestamp |
| Split Views | 是 | 是 | Object merge |
| Tab Groups | 是 | 是 | CRDT ordered list |
| Favorites | 是 | 是 | CRDT ordered list |
| Pinned Tabs | 是 | 是 | CRDT ordered list |
| Archived Tabs | 是 | 是 | Append-only log |
| Profiles metadata | 是 | 是 | Object merge |
| Cookies | 否 | 是 | 用户显式开启 |
| Site permissions | 是 | 是 | Last writer wins + audit |
| Bookmarks | 是 | 是 | CRDT tree |
| Reading List | 是 | 是 | Object merge |
| Notes | 是 | 是 | CRDT text |
| History | 可选 | 是 | Append-only log |
| Downloads metadata | 可选 | 是 | Append-only log |
| Browser settings | 是 | 是 | Last writer wins |
| Shortcuts | 是 | 是 | Last writer wins |
| Plugin list | 是 | 是 | Object merge |
| Plugin settings | 可选 | 是 | Plugin-defined merge |

### 9.4 Sync 加密模型

当前密钥结构：

```text
Approved Device
  ├─ Ed25519 Signing Key
  └─ X25519 Wrapping Key
       ↓ HPKE envelope per approved device
AccountKey generation N
       ↓ HKDF-SHA256
Snapshot Encryption Key + Content Authentication Key
```

要求：

- 32-byte AccountKey 在客户端生成，服务器保存 `(key_id, generation)` 与每设备 envelope。
- Envelope suite 固定为 `HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305`。
- Snapshot 使用 XChaCha20-Poly1305；AAD 绑定 user、generation、snapshot、schema、logical clock、device 和完整 head lineage。
- 新设备先进入 pending；approved v2 device 使用 recent Ed25519 proof 与 current-generation envelope 完成批准。
- 撤销 approved device 时，同一 D1 事务推进 Vault generation、写入剩余 approved v2 devices 的完整 envelope 集合、撤销目标会话并建立旧 R2 manifest。
- 新写入固定 `encryption_version=2`；legacy v1 仅承担历史解密与迁移。
- Better Auth 密码、OAuth 账号和服务器会话均不能直接解密 Sync payload。
- 新设备加入时，需要已登录设备批准、Recovery Key 或 Passkey + Recovery Key 组合。
- 密文 payload 使用 AEAD 加密。
- 每个对象保存 `object_id`、`object_type`、`encrypted_payload`、`payload_hash`、`schema_rev`、`created_at`、`updated_at`。
- D1 只存索引和小型密文；大型密文写入 R2。
- R2 object key 不直接暴露 URL、标题、站点名等敏感信息。
- 服务端日志不得记录 URL 明文、标题明文、书签明文、历史明文。
- 每个账户使用一个全局 snapshot head；上传通过 base head token 执行 CAS 推进。
- R2 写入先在 D1 建立带租约的写入记录，head 提交与引用状态在同一 D1 事务完成。

### 9.5 Sync 冲突处理

冲突来源：

- 两台设备同时移动同一标签。
- 同一 Space 在多设备同时重命名。
- 同一书签树节点同时移动。
- 同一设置项同时修改。
- 某设备离线数天后重新上线。

处理方式：

- 每个账户只有一个 global snapshot head。
- Genesis 固定 `head_revision=1` 与 `base_head=null`。
- 后继提交满足 `head_revision=base.revision+1`，base 的 `(revision,snapshot_id,payload_hash)` 精确匹配 current head，logical clock 严格递增。
- CAS 冲突返回 `{version:1,error,current_head}` 的 structured `409`。
- 完全相同的提交 replay 返回原 `201`，R2 put 次数保持为零。
- GET 使用 `(snapshot_id,head_revision,payload_hash)` exact token；历史 token 返回携带 current head 的 `409`。
- 有序列表使用 CRDT ordered list，保留稳定排序键。
- 普通对象使用 `updated_at + device_id + logical_clock` 解决。
- 不可自动合并的冲突进入 Conflict Center。
- Conflict Center 展示对象类型、设备、时间、差异和操作按钮。
- 用户可保留本机、保留远端、手动合并或复制副本。

### 9.6 Sync 状态 UI

```text
Settings / Sync
┌────────────────────────────────────────────────────┐
│ Account: zachary@example.com                       │
│ Device: MacBook Pro                                │
│ Sync: Connected                                    │
├────────────────────────────────────────────────────┤
│ Spaces              [on]  Last synced: just now    │
│ Tabs                [on]  Last synced: just now    │
│ Bookmarks           [on]  Last synced: 2 min ago   │
│ History             [off] Privacy controlled       │
│ Plugin settings     [on]  Last synced: just now    │
├────────────────────────────────────────────────────┤
│ Devices                                            │
│  • MacBook Pro       current                       │
│  • Windows Desktop   active                        │
│  • Linux Laptop      last seen yesterday           │
├────────────────────────────────────────────────────┤
│ [Add Device] [View Recovery Key] [Reset Sync Data] │
└────────────────────────────────────────────────────┘
```

状态要求：

- 顶栏只在 Sync 异常时显示小型状态图标。
- 设置页展示最近同步时间、队列长度、失败对象数量。
- Sync 失败不阻塞本地浏览。
- 用户可以按对象类型暂停同步。
- 用户可以导出加密 Sync 备份。
- 用户可以从云端删除全部 Sync 数据。
- Sync reset 清除云端 Sync 对象、快照、head 和变更记录，保留当前 Vault generation、设备信任和设备密钥 envelope。
- `/api/sync/status` response v2 在一个 `first-primary` D1 batch 中读取 cursor、对象聚合、snapshot count/head 与 device summary；head/count 或存储元数据失配时 fail closed。

---

## 10. Elydora Cloud 后端规格

### 10.1 架构

```text
ELY Desktop Client
  ↓ HTTPS
Cloudflare Workers API
  ├─ /api/auth/*             Better Auth
  ├─ /api/session/logout     Exact bearer session revoke
  ├─ /api/sync/push|pull     Authenticated retired endpoints
  ├─ /api/sync/snapshot      Encrypted snapshot v3 + global head CAS
  ├─ /api/sync/vault/*       AccountKey envelope bootstrap/read
  ├─ /api/sync/reset         Signed destructive action
  ├─ /api/devices/*          Device management
  ├─ /api/account/delete     Signed account deletion
  ├─ /api/plugins/*          Plugin registry
  ├─ /api/releases/*         Update manifest
  └─ /api/telemetry/*        Minimal diagnostics
       ↓
Cloudflare D1
  ├─ better_auth_* tables
  ├─ user_devices
  ├─ user_device_keys + device trust
  ├─ sync_snapshots + sync_snapshot_heads
  ├─ sync_vault_* + rotation manifests
  ├─ sync_r2_gc_candidates + inventory cursors
  ├─ plugin_registry
  ├─ plugin_reviews
  └─ audit_events
       ↓
Cloudflare R2
  ├─ sync-payloads/
  ├─ sync-snapshots/
  ├─ plugin-packages/
  ├─ user-avatars/
  ├─ crash-attachments/
  └─ exports/
       ↓
Cloudflare Workers KV
  ├─ public_config
  ├─ auth_session_cache        legacy cleanup namespace
  ├─ plugin_registry_cache
  ├─ public_signing_keys
  ├─ release_manifest_cache
  └─ sync_cursor_cache
```

### 10.2 Cloudflare D1 使用边界

D1 是结构化事实源。

D1 存储：

- Better Auth 用户、账号、会话、验证相关表。
- 设备记录：设备 ID、设备公钥、设备名称、平台、最后活跃时间。
- Sync 对象索引：对象 ID、对象类型、owner、payload 位置、hash、schema、时间戳。
- Sync 变更日志：用于增量拉取。
- Sync 快照索引：指向 R2 snapshot object。
- Snapshot encryption metadata 与 global head。
- AccountKey Vault generation、per-device HPKE envelope 与 rotation manifest。
- R2 write lease、引用状态、删除状态与 inventory cursor。
- 插件注册表元数据：插件 ID、名称、作者、权限声明、签名状态、包位置。
- 审计事件：登录、设备加入、设备撤销、权限变更、插件安装、Sync reset。

D1 不存储：

- URL 明文。
- 页面标题明文。
- 书签明文。
- 历史明文。
- Notes 明文。
- Cookie 明文。
- 插件私有配置明文。

D1 schema 设计要求：

- 用户作用域表使用 `user_id`；账号删除后的 R2 ledger 使用不可逆 `owner_hash` 收尾。
- Snapshot/Vault/head/GC 表使用各自的严格 key、generation、revision 与状态约束。
- Snapshot 删除使用 hard delete + durable R2 ledger；对象协议表保留历史 migration compatibility。
- D1 batch 提供原子 CAS 与 destructive action gate；`first-primary` session 提供顺序一致读取。
- 所有 D1 写入必须可幂等重放。
- 单个 D1 数据库接近容量阈值时按地区或账户拆分。

### 10.3 Cloudflare R2 使用边界

R2 是加密大对象事实源。

R2 存储：

- 大型 Sync payload。
- 周期性 Sync snapshot。
- 插件 `.rplug` 包。
- 插件图标、截图和说明文件。
- 用户头像。
- 加密导出文件。
- 用户主动提交的崩溃附件。

R2 object key 规范：

```text
sync-payloads/{region}/{user_hash}/{object_type}/{object_id}/{payload_hash}.bin
sync-snapshots/{region}/{user_hash}/{snapshot_id}/{payload_hash}.bin
plugin-packages/{plugin_id}/{package_hash}.rplug
plugin-assets/{plugin_id}/{asset_hash}
user-avatars/{user_hash}/{avatar_hash}
crash-attachments/{report_id}/{attachment_hash}
exports/{user_hash}/{export_id}.bin
```

要求：

- 所有 Sync payload 上传前在客户端加密。
- R2 metadata 不写敏感明文。
- Worker 经认证 API 直接执行 checksum-verified R2 put/get。
- 插件包必须通过签名验证后才进入 registry 可见状态。
- 用户删除账号时触发 R2 对象清理任务。
- Sync R2 对象使用 D1 ledger 跟踪 `pending`、`referenced`、`ready`、`deleting`、`deleted` 状态。
- 定时任务扫描 `sync-payloads/` 与 `sync-snapshots/` 历史对象并重试幂等删除。
- 账号删除提交后清除 ledger 中的原始 user ID，使用不可逆 owner hash 继续完成 R2 清理。
- Snapshot 写入先领取 64-hex write token 与 10 分钟 lease；candidate、encryption、head、referenced state、exact head SELECT 在五语句 D1 batch 中提交。
- Hourly cron `17 * * * *` 分别执行 legacy KV purge、R2 prefix inventory、bounded GC 与 rotation cleanup；完整 prefix 重扫周期为 24 小时。

### 10.4 Cloudflare Workers KV 使用边界

KV 是缓存和读多写少配置存储。

KV 存储：

- `public_config`：公开运行配置、服务端开关、地区路由提示。
- `auth_session_cache`：legacy namespace，进入分页清理流程。
- `plugin_registry_cache`：插件市场列表缓存。
- `public_signing_keys`：插件签名公钥、服务端公钥。
- `release_manifest_cache`：自动更新清单缓存。
- `sync_cursor_cache`：规划中的短期同步游标缓存。

KV 使用规则：

- KV 不作为 Sync 事实源。
- KV 不存储端到端加密密钥。
- KV 不存储需要强一致的对象状态。
- KV 缓存可随时失效，所有关键数据必须可从 D1/R2 重建。
- KV key 设计必须包含 namespace 和环境前缀。
- KV TTL 必须按用途设置，默认不得无限期保存短期状态。
- 定时任务分页清理历史 `auth_session_cache` key。
- 所有认证请求直接读取 Better Auth D1 session；KV 不参与 session authority。

### 10.5 Better Auth 集成

Better Auth 在 Cloudflare Workers 中初始化，D1 binding 作为 database 传入。

当前能力：

- Email/password 注册登录。
- Encrypted Email OTP。
- Google/GitHub OAuth 按环境启用。
- D1 session validation 与自定义 device/session binding。
- Bearer logout 精确删除当前 D1 session，并级联清理 session device context 与 rebind challenge。
- Desktop bearer 以 stable `ProfileId` 作为系统凭据 account，Windows 使用 Local persistence；旧明文文件在 credential read-back 与 durable marker 提交后清理，系统凭据不可用时阻断设备加载与 Sync upload。
- Desktop sign-out closes the authenticated-operation gate, drains active leases, revokes the exact server session, and conditionally clears the captured native credential; generation-stamped async results cannot restore stale auth or Sync state.
- Runtime `session_not_found` and `session_expired` responses conditionally clear the exact captured bearer inside `SyncEngine`; replacement credentials survive, stale Profile work converges through credential reprobe, and active Cloud Sync/device work resets before session-state reconciliation.
- One immutable Ely `user_id` owns the complete browser data root and its multi-Profile encrypted snapshot. Interactive OTP sign-in claims an unowned root after generation validation; every background device or Sync operation verifies the owner through `GET /api/devices` before device registration or cloud data access.
- 设备注册、rebind、批准、撤销与 Vault rotation。
- Signed Sync reset 和 signed account deletion。
- 管理所有 `/api/auth/*` 路由。

目标能力包括 Apple OAuth、Passkey、Recovery Key 与完整 session 管理 UX。

会话策略：

- Access session 短期有效。
- Refresh session 存放在系统钥匙串。
- 服务端可以撤销单设备或全部设备会话。
- Desktop client 每次启动执行 session validation。
- 高风险操作要求重新验证。
- Sync reset 与账号删除 request v2 要求 5 分钟内的 Ed25519 action proof；proof 绑定 action、user、session、device、confirmation、idempotency key 与创建时间。
- Destructive D1 batch 首条执行 live session/device/key gate；guard failure 与 concurrent replay 触发事务 abort，后续通过 exact audit marker 收敛。

### 10.6 API 规格

Auth：

| Endpoint | Method | 说明 |
|---|---|---|
| `/api/auth/*` | Any | Better Auth handler |
| `/api/session/logout` | POST | 精确撤销当前 bearer session |
| `/api/devices` | GET | 当前账号设备列表 |
| `/api/devices/register` | POST | 注册当前设备公钥 |
| `/api/devices/rebind/challenge` | POST | 为未绑定的新会话签发短期 challenge |
| `/api/devices/rebind` | POST | 用现有 Ed25519 device key 绑定新会话 |
| `/api/devices/approve` | POST | 批准新设备加入 |
| `/api/devices/revoke` | POST | 撤销设备 |
| `/api/account/delete` | POST | request v2 recent proof + atomic gate 删除账号 |

Sync：

| Endpoint | Method | 说明 |
|---|---|---|
| `/api/sync/push` | POST | 认证后返回 `410` 的退役对象协议 |
| `/api/sync/pull` | GET | 认证后返回 `410` 的退役对象协议 |
| `/api/sync/snapshot` | POST | request/response v3 encrypted snapshot CAS |
| `/api/sync/snapshot` | GET | exact head token 下载 encrypted snapshot |
| `/api/sync/vault/bootstrap` | POST | request v2 proof 创建 generation 1 Vault |
| `/api/sync/vault` | GET | 读取当前或 exact historical device envelope |
| `/api/sync/reset` | POST | request v2 recent proof + atomic gate 删除云端 Sync 数据 |
| `/api/sync/status` | GET | response v2 cursor/object/snapshot/device summary |

`/api/sync/reset` 与账号删除响应中的 `deleted.r2_objects` 表示已识别并进入持久清理队列的 R2 对象数；物理删除由请求内回收和定时重试共同完成。

Plugin：

| Endpoint | Method | 说明 |
|---|---|---|
| `/api/plugins` | GET | 插件列表 |
| `/api/plugins/:id` | GET | 插件详情 |
| `/api/plugins/:id/package` | GET | 获取签名包下载信息 |
| `/api/plugins/publish` | POST | 发布插件包 |
| `/api/plugins/revoke` | POST | 撤销插件包 |

Update：

| Endpoint | Method | 说明 |
|---|---|---|
| `/api/releases/manifest` | GET | 自动更新清单 |
| `/api/releases/signature` | GET | 发布包签名信息 |

### 10.7 后端数据表

Better Auth 表由 Better Auth schema 管理。ELY 自定义表如下：

| 表 | 用途 |
|---|---|
| `user_devices` | 用户设备、公钥、平台、活跃状态 |
| `user_device_keys` | Ed25519/X25519 public key 与 protocol version |
| `device_approvals` | 新设备加入批准记录 |
| `device_rebind_challenges` | 短期 session rebind challenge 与消费状态 |
| `better_auth_session_device_context` | Better Auth session 与 device identity 绑定 |
| `sync_objects` | Sync 对象索引 |
| `sync_change_log` | Sync 增量日志 |
| `sync_snapshots` | Sync 快照索引 |
| `sync_snapshot_encryption` | Snapshot encryption version、generation、key/content hash |
| `sync_snapshot_heads` | 每账户唯一 global snapshot head |
| `sync_tombstones` | 删除标记 |
| `sync_vault_accounts` | 当前 AccountKey id 与 generation |
| `sync_vault_envelopes` | Per-device HPKE AccountKey envelope |
| `sync_vault_rotations` | Device revoke Vault rotation 状态 |
| `sync_vault_rotation_envelopes` | Rotation recipient exact set |
| `sync_vault_rotation_r2_objects` | Rotation 旧 R2 manifest |
| `pending_device_revocations` | Pending device revoke 幂等记录 |
| `sync_r2_gc_candidates` | R2 write lease、reference 与 GC state machine |
| `sync_r2_inventory_cursors` | R2 prefix inventory cursor |
| `plugin_registry` | 插件注册表 |
| `plugin_packages` | 插件包和签名 |
| `plugin_reviews` | 插件审核记录 |
| `audit_events` | 安全审计事件 |
| `release_manifests` | 发布清单索引 |

`sync_objects` 字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `object_id` | text | 对象 ID |
| `user_id` | text | 用户 ID |
| `object_type` | text | 对象类型 |
| `payload_inline` | blob nullable | 小型密文 |
| `payload_r2_key` | text nullable | R2 key |
| `payload_hash` | text | payload hash |
| `schema_rev` | integer | schema 修订号 |
| `logical_clock` | integer | 逻辑时钟 |
| `device_id` | text | 来源设备 |
| `created_at` | integer | 创建时间 |
| `updated_at` | integer | 更新时间 |
| `deleted_at` | integer nullable | 删除时间 |

`sync_change_log` 字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `change_id` | integer | 自增变更 ID |
| `user_id` | text | 用户 ID |
| `object_id` | text | 对象 ID |
| `object_type` | text | 对象类型 |
| `operation` | text | upsert / delete |
| `payload_hash` | text | payload hash |
| `logical_clock` | integer | 逻辑时钟 |
| `device_id` | text | 来源设备 |
| `created_at` | integer | 创建时间 |

---

## 11. 自有插件系统

### 11.1 插件定位

ELY 只支持自有 `.rplug` 插件协议。插件用于扩展浏览器命令、侧栏、设置页、上下文菜单、下载处理、书签导出、页面信息读取、开发工具和工作流集成。

插件设计目标：

- 安全：默认无能力，所有能力显式授权。
- 可审计：所有权限声明、运行事件、用户授权和异常都记录。
- 可撤销：用户可以随时停用插件或撤销单项权限。
- 可移植：插件以 Wasm Component Model 作为运行边界。
- 可签名：市场插件必须签名，侧载插件显示风险提示。
- 可恢复：插件崩溃不得影响浏览器主进程和网页渲染。

### 11.2 `.rplug` 包结构

```text
my-plugin.rplug
├─ plugin.toml
├─ component.wasm
├─ wit/
│  └─ ely-browser.wit
├─ assets/
│  ├─ icon.svg
│  └─ preview.png
├─ README.md
├─ LICENSE
└─ signatures/
   └─ ed25519.sig
```

`plugin.toml` 字段：

| 字段 | 说明 |
|---|---|
| `id` | 插件唯一 ID |
| `name` | 插件名 |
| `description` | 简介 |
| `author` | 作者 |
| `homepage` | 主页 |
| `permissions` | 权限声明 |
| `contributes` | UI / command / menu 贡献点 |
| `min_ely_build` | 最低 ELY 构建要求 |
| `checksum` | Wasm checksum |
| `signature` | 签名信息 |

### 11.3 插件权限

权限清单：

| 权限 | 能力 |
|---|---|
| `tabs:read` | 读取标签元数据 |
| `tabs:write` | 创建、移动、关闭标签 |
| `spaces:read` | 读取 Space 元数据 |
| `spaces:write` | 创建、修改、删除 Space |
| `bookmarks:read` | 读取书签 |
| `bookmarks:write` | 写入书签 |
| `history:read` | 读取历史 |
| `downloads:read` | 读取下载列表 |
| `downloads:write` | 控制下载 |
| `page:metadata` | 读取当前页面标题、URL、favicon |
| `page:screenshot` | 获取当前页面截图 |
| `page:script` | 注入受限页面脚本 |
| `clipboard:read` | 读取剪贴板 |
| `clipboard:write` | 写入剪贴板 |
| `filesystem:read` | 读取用户选择的文件 |
| `filesystem:write` | 写入用户选择的位置 |
| `network:fetch` | 插件发起网络请求 |
| `settings:read` | 读取插件设置 |
| `settings:write` | 写入插件设置 |
| `sync:plugin` | 插件配置参与 Sync |
| `ui:panel` | 注册侧栏 Panel |
| `ui:command` | 注册命令 |
| `ui:context_menu` | 注册右键菜单 |

授权要求：

- 首次安装展示权限列表。
- 高风险权限单独确认。
- 页面脚本权限必须按域名授权。
- 网络权限必须声明目标域名或使用用户确认。
- 文件权限必须通过系统文件选择器授予。
- 插件权限变更必须重新确认。

### 11.4 插件 UI 贡献点

贡献点：

- Command Bar commands。
- Tab context menu。
- Page context menu。
- Sidebar panels。
- Settings pages。
- Status bar indicators。
- Download actions。
- Bookmark actions。
- Reading Mode exporters。

插件 UI 使用 GPUI Host 提供的声明式组件接口，不允许直接访问主进程内部对象。插件 UI 只能通过 WIT host calls 请求数据和提交事件。

### 11.5 插件市场

插件市场提供：

- 列表、搜索、分类、详情页。
- 权限摘要。
- 签名状态。
- 作者信息。
- 安装量。
- 最近更新。
- 用户评分。
- 安全审核状态。
- 安装、禁用、卸载、更新。

插件详情页必须展示：

- 插件名称、作者、描述。
- 权限清单和解释。
- 数据访问范围。
- 是否参与 Sync。
- 包 checksum。
- 签名状态。
- 举报入口。

---

## 12. 技术架构

### 12.1 本地架构

```text
ELY Desktop
├─ App Shell (Rust + GPUI)
│  ├─ Window Manager
│  ├─ Command Bar
│  ├─ Sidebar
│  ├─ Settings
│  ├─ Panels
│  └─ Notifications
├─ Browser Core
│  ├─ Tab Manager
│  ├─ Space Manager
│  ├─ Profile Manager
│  ├─ Session Manager
│  ├─ Permission Manager
│  ├─ Download Manager
│  ├─ History Manager
│  ├─ Bookmark Manager
│  └─ Reading Mode
├─ Servo Host
│  ├─ WebView Host
│  ├─ Rendering Bridge
│  ├─ Input Bridge
│  ├─ Focus Bridge
│  ├─ IME Bridge
│  ├─ Permission Bridge
│  ├─ Download Bridge
│  └─ DevTools Bridge
├─ Data Layer
│  ├─ Local Encrypted SQLite
│  ├─ Object Store
│  ├─ Search Index
│  ├─ Keychain Adapter
│  └─ Migration Engine
├─ Sync Core
│  ├─ Queue
│  ├─ Crypto
│  ├─ Merge Engine
│  ├─ Conflict Center
│  └─ Cloud API Client
├─ Plugin Host
│  ├─ Wasmtime Runtime
│  ├─ WIT Bindings
│  ├─ Permission Broker
│  ├─ UI Host
│  ├─ Event Bus
│  └─ Audit Log
└─ Platform Adapters
   ├─ macOS
   ├─ Windows
   └─ Linux
```

### 12.2 GPUI 生态映射

| 库 | 用途 | 验收要求 |
|---|---|---|
| gpui-component | 主组件库，设置页、列表、弹窗、菜单、表格、Dock | 所有核心 UI 使用统一主题、间距、焦点态 |
| adabraka-ui | 视觉增强组件、空状态、卡片、按钮组 | 只用于提升 clean UI 表达，不引入视觉噪声 |
| ferrum-flow | 工作区图谱、插件可视化配置、导入导出流程 | 节点编辑场景可拖拽、缩放、保存 |
| gpui-flow | 轻量可视化节点编辑 | 用于规则编辑器和插件工作流图 |
| gpui-form | 设置表单、Profile 表单、插件权限表单 | 表单字段自动校验、错误提示、重置 |
| gpui-hooks | 组件内部状态复用 | Hook 边界清晰，避免跨模块隐式状态 |
| gpui-nav | 设置导航、侧栏导航、面板导航 | 支持键盘导航和焦点恢复 |
| gpui-router | Settings、internal pages、plugin pages | `ely://settings/*` 可直接打开 |
| gpui-storybook | 设计系统和组件验收 | 每个核心组件有 Story |
| gpui-symbols | 平台符号图标 | macOS 使用 SF Symbols 风格，其他平台 fallback |
| gpui-tea | 浏览器 Shell 状态循环 | 适用于 Tab/Space/Window 事件模型 |
| gpui-d3rs | 低层图表 | 性能任务管理器与诊断图表 |
| gpui-px | 高层图表 | 历史趋势、下载速度、Sync 状态 |
| plotters-gpui | plotters 后端 | 高级性能分析图 |
| gpui-video-player | 本地视频下载预览 | 下载完成后可预览视频文件 |

### 12.3 Servo 集成

Servo Host 需要实现：

- WebView 生命周期：create、attach、detach、sleep、restore、destroy。
- 导航：load URL、back、forward、reload、stop、same-document navigation。
- 输入：mouse、keyboard、touch、wheel、drag、drop。
- IME：composition start/update/end、candidate window placement。
- 渲染：texture handoff、resize、damage tracking、frame scheduling。
- 焦点：WebView focus、address bar focus、plugin panel focus。
- 权限：permission request → GPUI dialog → profile policy → Servo response。
- 下载：download start → Download Manager → file picker / policy。
- 截图：tab thumbnail、reading capture、plugin screenshot permission。
- DevTools：打开当前 WebView 调试入口。
- 崩溃恢复：WebView crash event → tab recovery view。

站点兼容性要求：

- 内置 Site Compatibility Panel。
- 用户可复制诊断信息。
- 诊断信息包括 Servo build、URL host、Profile、权限、错误码、控制台摘要。
- 用户可选择提交匿名站点兼容报告。
- 报告默认去除 URL path 和 query。

### 12.4 本地数据

本地数据使用加密 SQLite + 对象文件夹。

本地库：

- `ely_browser.db`：核心关系数据。
- `ely_search.db`：搜索索引。
- `ely_sync.db`：Sync 队列和游标。
- `objects/`：截图、favicon、阅读缓存、插件包。
- `profiles/`：Profile 级 Servo site data。

本地加密要求：

- 数据库密钥存储于系统钥匙串。
- 用户可启用启动密码。
- 隐私 Profile 使用临时密钥。
- 退出时清理隐私 Profile 临时数据。
- 崩溃恢复文件同样加密。

---

## 13. 安全与隐私

### 13.1 安全边界

- GPUI Shell 与 Servo WebView 分离。
- Web 内容与插件运行时分离。
- 插件默认无权限。
- Profile 之间站点数据隔离。
- Sync 密文与账号会话分离。
- 本地密钥与云端会话分离。
- 高风险操作有显式确认和审计日志。
- Sync reset 与账号删除使用 current-device Ed25519 recent proof；D1 commit 原子复核 live session、session-device binding、approved v2 device 与 exact signing key。

### 13.2 隐私默认值

- 遥测默认最小化。
- 历史 Sync 默认由用户选择。
- Cookie Sync 默认关闭。
- 插件网络权限默认关闭。
- 插件页面脚本权限默认关闭。
- 站点通知默认询问。
- 位置权限默认询问。
- 第三方插件市场安装默认显示风险提示。

### 13.3 审计日志

本地审计日志记录：

- 登录和注销。
- 新设备加入。
- 设备撤销。
- Sync reset。
- 插件安装、启用、禁用、卸载。
- 插件权限授权和撤销。
- 站点权限授权和撤销。
- 证书例外。
- 私有数据清理。

审计日志默认只保存在本地，可选择端到端加密同步。

服务端 `audit_events` 记录 device approval/revoke、Sync reset 与账号删除。账号删除保留 anonymized success marker；marker 绑定 exact proof hash，使已认证的并发请求在 device/key 清理后仍能安全收敛。

---

## 14. 性能指标

| 指标 | 要求 |
|---|---:|
| 冷启动到首窗可交互 | P95 < 1.5s |
| 新建标签响应 | P95 < 80ms |
| Command Bar 打开 | P95 < 50ms |
| 侧栏滚动 | 60fps 目标 |
| 1,000 标签侧栏内存增量 | < 80MB |
| 标签切换 Shell 响应 | P95 < 80ms |
| WebView 崩溃恢复 UI | < 300ms |
| Sync 10k 对象增量应用 | P95 < 2s |
| 插件启动 | P95 < 300ms |
| 插件单次 host call | P95 < 20ms |
| 设置页路由切换 | P95 < 80ms |
| 下载列表 10k 项滚动 | 60fps 目标 |

性能工具：

- 内置 Task Manager。
- Tab memory usage。
- WebView CPU / memory。
- Plugin CPU / memory / calls。
- Sync queue length。
- Download throughput。
- GPUI frame time。
- Servo frame time。

---

## 15. 跨平台要求

### 15.1 macOS

- Universal binary。
- Notarization。
- Native menu bar。
- Traffic lights integration。
- Keychain。
- Notification Center。
- Services menu。
- Drag/drop files and URLs。
- Fullscreen and Spaces behavior。
- Touchpad gestures。
- IME support。

### 15.2 Windows

- x64 installer。
- Code signing。
- Windows Credential Manager。
- Jump List。
- System notifications。
- Snap layout support。
- High contrast mode。
- Per-monitor DPI。
- IME support。
- Protocol handler registration。

### 15.3 Linux

- AppImage / deb / rpm。
- Secret Service integration。
- Wayland + X11。
- Desktop file。
- Portal file picker。
- Notifications。
- System theme detection。
- IME support。
- Protocol handler registration。

---

## 16. 可访问性与国际化

### 16.1 可访问性

要求：

- 所有可交互控件提供 accessible name。
- 侧栏标签可键盘遍历。
- Command Bar 支持屏幕阅读器。
- 弹窗焦点 trap。
- Dialog 关闭后焦点返回触发元素。
- 高对比主题。
- Reduced motion。
- 可配置字号和 UI density。
- Split View 子区域有明确焦点边框。
- 错误提示使用文本和图标双重表达。

### 16.2 国际化

默认语言：English、简体中文。  
后续语言通过资源文件扩展。

要求：

- 文案不硬编码。
- 日期、时间、数字格式本地化。
- 快捷键显示按平台本地化。
- RTL 布局预留。
- 插件市场支持插件多语言说明。

---

## 17. 内置页面

内置页面使用 `ely://` 协议。

| 路由 | 页面 |
|---|---|
| `ely://new-tab` | 新标签页 |
| `ely://settings` | 设置 |
| `ely://settings/sync` | Sync 设置 |
| `ely://settings/profiles` | Profiles 设置 |
| `ely://settings/plugins` | 插件设置 |
| `ely://history` | 历史 |
| `ely://bookmarks` | 书签 |
| `ely://downloads` | 下载 |
| `ely://reading-list` | Reading List |
| `ely://archive` | Archived Tabs |
| `ely://task-manager` | 任务管理器 |
| `ely://plugins` | 插件市场 |
| `ely://plugin/<id>` | 插件详情 |
| `ely://sync/status` | Sync 状态 |
| `ely://about` | 关于 ELY Browser |
| `ely://crash/<tab_id>` | 标签崩溃恢复 |
| `ely://site/<origin>` | 站点设置 |

---

## 18. 键盘快捷键

| 动作 | macOS | Windows/Linux |
|---|---|---|
| Command Bar | `Cmd+L` / `Cmd+T` | `Ctrl+L` / `Ctrl+T` |
| 命令模式 | `Cmd+Shift+P` | `Ctrl+Shift+P` |
| 新建标签 | `Cmd+T` | `Ctrl+T` |
| 关闭标签 | `Cmd+W` | `Ctrl+W` |
| 恢复关闭标签 | `Cmd+Shift+T` | `Ctrl+Shift+T` |
| 切换下一个标签 | `Cmd+Option+↓` | `Ctrl+Alt+↓` |
| 切换上一个标签 | `Cmd+Option+↑` | `Ctrl+Alt+↑` |
| 切换 Space | `Cmd+Option+←/→` | `Ctrl+Alt+←/→` |
| Toggle Sidebar | `Cmd+B` | `Ctrl+B` |
| Split Right | `Cmd+\` | `Ctrl+\` |
| Open Downloads | `Cmd+Shift+J` | `Ctrl+Shift+J` |
| Open History | `Cmd+Y` | `Ctrl+H` |
| Open Settings | `Cmd+,` | `Ctrl+,` |
| Task Manager | `Cmd+Esc` | `Shift+Esc` |

快捷键要求：

- 所有快捷键可自定义。
- 冲突检测必须实时提示。
- 插件快捷键必须进入同一冲突检测系统。
- 用户可导入导出快捷键配置。

---

## 19. 验收矩阵

### 19.1 产品验收

| ID | 模块 | 验收标准 |
|---|---|---|
| P-001 | Brand | 应用、安装包、关于页、协议、服务名统一为 ELY Browser by Elydora |
| P-002 | Vertical Tabs | 1,000 标签可滚动、搜索、拖拽、批量操作 |
| P-003 | Spaces | 创建、切换、排序、导出、导入、删除恢复完整可用 |
| P-004 | Profiles | Cookie、Storage、History、Permissions 严格隔离 |
| P-005 | Split View | 2/3/4 WebView 支持保存为复合标签 |
| P-006 | Favorites | 跨 Space 常驻并可 Sync |
| P-007 | Archive | Unpinned Tabs 自动归档并可搜索恢复 |
| P-008 | Command Bar | URL、搜索、命令、书签、历史、标签统一搜索 |
| P-009 | Downloads | 生命周期控制、Profile 策略、安全提示完整 |
| P-010 | Reading Mode | 正文提取、样式设置、阅读进度、导出完整 |
| P-011 | Settings | 所有设置项可搜索、校验、保存、恢复默认 |
| P-012 | Login | Better Auth 登录、OAuth、Passkey、设备管理可用 |
| P-013 | Sync | Spaces、tabs、bookmarks、settings、plugin settings 可端到端加密同步 |
| P-014 | Cloudflare | D1/R2/KV 边界清晰，所有 API 可观测、可限流、可恢复 |
| P-015 | Plugins | `.rplug` 安装、授权、运行、停用、卸载、市场完整 |
| P-016 | Privacy | 用户可查看、导出、删除本地和云端数据 |
| P-017 | Cross-platform | macOS、Windows、Linux 构建、签名、安装、更新可用 |

### 19.2 安全验收

| ID | 模块 | 验收标准 |
|---|---|---|
| S-001 | Sync encryption | 服务端无法解密 Sync payload |
| S-002 | Keychain | session token 按 stable ProfileId 隔离存入原生系统凭据；明文 legacy token 完成可恢复的一次迁移后删除；凭据不可用时进入独立错误状态并暂停云端动作 |
| S-003 | Device revoke | 撤销设备后目标 sessions 失效、后续 Sync API 被拒绝、approved revoke 原子轮换 Vault generation |
| S-004 | Plugin sandbox | 插件无声明权限时无法访问 tabs/bookmarks/history/page |
| S-005 | Plugin signature | 市场插件必须签名验证通过 |
| S-006 | Site permissions | 所有站点权限按 Profile 隔离 |
| S-007 | Private Window | 关闭后无持久 Cookie、Storage、History |
| S-008 | Audit log | 高风险操作全部写入本地审计日志 |
| S-009 | Cloud logs | 服务端日志不得记录敏感明文 URL、标题、历史、书签 |
| S-010 | Account deletion | Signed request 原子删除 D1 权威数据；R2 ledger 与 legacy KV cleanup 持久排队并由请求内 drain + scheduled retry 收口 |
| S-011 | Snapshot CAS | 单一 global head、exact base CAS、structured 409、exact replay zero R2 put |
| S-012 | Destructive proof | Stolen bearer 缺少 device private key 时无法执行 Sync reset 或账号删除 |
| S-013 | Session logout | exact bearer 撤销 D1 当前 session；device context 与 rebind challenge 级联删除；sibling sessions 保留 |

### 19.3 性能验收

| ID | 指标 | 验收标准 |
|---|---|---:|
| F-001 | Cold start | P95 < 1.5s |
| F-002 | New tab | P95 < 80ms |
| F-003 | Command Bar | P95 < 50ms |
| F-004 | Sidebar scroll | 60fps 目标 |
| F-005 | Tab switch | P95 < 80ms |
| F-006 | Sync apply | 10k delta P95 < 2s |
| F-007 | Plugin launch | P95 < 300ms |
| F-008 | Settings route | P95 < 80ms |
| F-009 | Downloads list | 10k rows 60fps 目标 |
| F-010 | Split resize | 60fps 目标 |

---

## 20. 工程组织与开发规范

### 20.1 Rust workspace

```text
ely-browser/
├─ crates/
│  ├─ ely_app/
│  ├─ ely_ui/
│  ├─ ely_design_system/
│  ├─ ely_browser_core/
│  ├─ ely_servo_host/
│  ├─ ely_profiles/
│  ├─ ely_tabs/
│  ├─ ely_spaces/
│  ├─ ely_history/
│  ├─ ely_bookmarks/
│  ├─ ely_downloads/
│  ├─ ely_permissions/
│  ├─ ely_sync/
│  ├─ ely_crypto/
│  ├─ ely_plugins/
│  ├─ ely_plugin_wit/
│  ├─ ely_cloud_client/
│  ├─ ely_storage/
│  ├─ ely_search/
│  ├─ ely_platform/
│  └─ ely_telemetry/
├─ cloudflare/
│  ├─ worker/
│  ├─ migrations/
│  ├─ wrangler.toml
│  └─ tests/
├─ plugins/
│  ├─ examples/
│  └─ sdk/
├─ assets/
├─ stories/
├─ tests/
└─ docs/
```

### 20.2 Cloudflare worker workspace

```text
cloudflare/
├─ worker/
│  ├─ src/
│  │  ├─ index.ts
│  │  ├─ auth.ts
│  │  ├─ sync.ts
│  │  ├─ devices.ts
│  │  ├─ plugins.ts
│  │  ├─ releases.ts
│  │  ├─ audit.ts
│  │  └─ bindings.ts
│  ├─ package.json
│  └─ tsconfig.json
├─ migrations/
│  ├─ 0001_auth.sql
│  ├─ 0002_devices.sql
│  ├─ 0003_sync.sql
│  ├─ 0004_plugins.sql
│  └─ 0005_audit.sql
├─ wrangler.toml
└─ tests/
```

Cloudflare 绑定：

```text
D1 binding: ELY_DB
R2 bucket: ELY_STORAGE
KV namespace: ELY_KV
```

环境要求：

- `local`：本地开发。
- `staging`：预发布验证。
- `production`：正式服务。

每个环境必须拥有独立 D1 database、R2 bucket、KV namespace 和 Better Auth secret。

### 20.3 代码规范

- Rust 使用 workspace lint。
- 所有跨模块事件使用 typed event。
- 所有 Sync object 变更必须有 migration、merge、encryption test。
- 所有插件 host call 必须有权限测试。
- 所有设置项必须有 schema、默认值、校验器、UI story。
- 所有 Cloudflare API 必须有 auth、rate limit、schema validation、audit。
- 所有 D1 migration 必须可回放。
- 所有 R2 object write 必须校验 checksum。
- 所有 KV 写入必须设置命名空间前缀。

### 20.4 测试要求

测试类型：

- Unit tests。
- Integration tests。
- Snapshot tests。
- UI story tests。
- Sync conflict tests。
- Encryption tests。
- Plugin sandbox tests。
- Cloudflare Worker API tests。
- D1 migration tests。
- R2 upload/download tests。
- Real SQLite CAS、rollback 与 proof-after-session-revoke interleaving tests。
- R2 write lease、late put、101+ drain、inventory、delete retry tests。
- Legacy encryption metadata backfill 与 structured `409` contract tests。
- KV cache invalidation tests。
- Cross-platform smoke tests。
- Servo site compatibility smoke tests。

测试数据：

- 10 tabs。
- 100 tabs。
- 1,000 tabs。
- 10,000 history rows。
- 10,000 bookmarks。
- 10,000 Sync objects。
- 100 plugins installed but inactive。
- 10 active plugins。
- 4 Split View panes。
- 3 Profiles。
- 20 Spaces。

---

## 21. 运营、监控与发布

### 21.1 遥测最小化

默认收集：

- App 启动成功/失败。
- 崩溃报告。
- WebView crash 类型。
- Sync 错误码。
- 插件崩溃事件。
- 更新成功/失败。

默认不收集：

- URL 明文。
- 页面标题。
- 搜索词。
- 书签内容。
- 历史内容。
- Notes 内容。
- Cookie。
- 表单输入。

用户可在 Settings / Privacy & Security 中关闭诊断上报。

### 21.2 Cloudflare 监控

监控项：

- Worker request rate。
- Worker error rate。
- Auth success/failure rate。
- D1 query latency。
- D1 write failure。
- R2 upload/download failure。
- KV cache hit rate。
- Snapshot upload/download latency。
- Snapshot head conflict rate。
- R2 pending/ready/deleting backlog 与 inventory cursor age。
- Scheduled storage maintenance failure 与 Vault rotation cleanup lag。
- Device revoke events。
- Plugin install failure。
- Plugin signature failure。

告警：

- Auth failure spike。
- Sync 5xx spike。
- D1 query latency spike。
- R2 write failure spike。
- KV stale cache anomaly。
- Plugin signature verification failure spike。

### 21.3 更新系统

更新系统要求：

- 发布包签名验证。
- 更新清单通过 Cloudflare Workers 提供，并缓存到 KV。
- 发布包存储在 R2。
- 客户端下载后验证 checksum 和签名。
- 更新失败回滚到上一可用构建。
- 用户可选择自动更新或手动更新。

---

## 22. 风险与应对

| 风险 | 影响 | 应对 |
|---|---|---|
| Servo 站点兼容不足 | 复杂网页渲染、登录、媒体可能异常 | 站点兼容面板、Top sites smoke、错误恢复、诊断提交 |
| GPUI 生态仍在快速演进 | API 变动影响 UI 工程 | 锁定依赖 commit、封装 UI facade、Storybook 覆盖核心组件 |
| D1 单数据库容量限制 | 大规模 Sync 元数据增长 | 按账户、地区、对象类型拆分；R2 承担大 payload |
| KV eventual consistency | 用户看到短期缓存滞后 | KV 只做缓存，D1/R2 作为事实源 |
| Better Auth 与 Workers 运行差异 | Auth callback、session、cookie 策略复杂 | 独立 Auth 集成测试，Desktop callback smoke |
| 插件滥用权限 | 用户数据风险 | Wasm 沙箱、权限 broker、审计日志、签名市场、资源限额 |
| Sync 冲突 | 多设备状态混乱 | CRDT ordered list、Conflict Center、对象级审计 |
| 跨平台输入法问题 | 中文、日文、韩文输入体验受损 | IME 桥专项测试、平台适配器、候选框定位测试 |
| Split View 资源消耗 | 多 WebView 内存上涨 | 休眠策略、任务管理器、资源提示 |
| 下载安全风险 | 恶意文件执行 | 危险扩展名提示、checksum、下载来源展示 |

---

## 23. 关键决策

| 决策 | 结果 |
|---|---|
| 产品名称 | ELY Browser by Elydora |
| 产品定位 | clean browser |
| UI 框架 | GPUI |
| 主组件库 | gpui-component |
| Web 引擎 | Servo |
| 标签形态 | 左侧垂直标签为默认信息架构 |
| 任务上下文 | Spaces |
| 账号/站点隔离 | Profiles |
| 多网页并排 | Split View |
| 登录 | Better Auth on Cloudflare Workers |
| 数据库 | Cloudflare D1 |
| 对象存储 | Cloudflare R2 |
| 缓存/配置 | Cloudflare Workers KV |
| Sync | 本地优先 + 端到端加密 + D1/R2/KV |
| 插件系统 | 自有 `.rplug` Wasm Component Model |
| 插件市场 | Elydora Plugin Registry |
| 外部浏览器插件运行时 | 不纳入产品接口 |
| 遥测 | 默认最小化，可关闭 |

---

## 24. Reference

[R1] Google Chrome — Get more done with new vertical tabs and immersive reading mode in Chrome — https://blog.google/products-and-platforms/products/chrome/new-chrome-productivity-features/  
[R2] Microsoft Edge — Vertical Tabs — https://explore.microsoft.com/en-us/edge/features/vertical-tabs?form=MT0160 <br>
[R3] Arc Help Center — Favorites: Top Tabs Across Every Space — https://resources.arc.net/hc/en-us/articles/19230755904151-Favorites-Top-Tabs-Across-Every-Space  
[R4] Arc Help Center — Auto Archive: Clean as you go — https://resources.arc.net/hc/en-us/articles/19228855311127-Auto-Archive-Clean-as-you-go  
[R5] Vivaldi — Workspaces — https://vivaldi.com/features/workspaces/  
[R6] Vivaldi Help — Tab Tiling — https://help.vivaldi.com/desktop/tabs/tab-tiling/  
[R7] GPUI official — https://gpui.rs/ <br>
[R8] docs.rs — GPUI — https://docs.rs/gpui  
[R9] Zed Blog — Leveraging Rust and the GPU to render user interfaces at 120 FPS — https://zed.dev/blog/videogame  
[R10] gpui-component — https://github.com/longbridge/gpui-component  
[R11] zed-industries/awesome-gpui — https://github.com/zed-industries/awesome-gpui  
[R12] Servo official — https://servo.org/  
[R13] Servo Blog — Servo is now available on crates.io — https://servo.org/blog/2026/04/13/servo-0.1.0-release/  
[R14] Cloudflare D1 docs — https://developers.cloudflare.com/d1/  
[R15] Cloudflare Workers docs — Choose a data or storage product — https://developers.cloudflare.com/workers/platform/storage-options/  
[R16] Cloudflare Workers KV docs — How KV works — https://developers.cloudflare.com/kv/concepts/how-kv-works/  
[R17] Better Auth 1.5 — Cloudflare D1 Support — https://better-auth.com/blog/1-5  
[R18] Cloudflare D1 limits — https://developers.cloudflare.com/d1/platform/limits/  
[R19] WebAssembly Component Model — https://component-model.bytecodealliance.org/  
[R20] Wasmtime Component Model embedding API — https://docs.wasmtime.dev/api/wasmtime/component/index.html  
[R21] Wasmtime Security — https://docs.wasmtime.dev/security.html
