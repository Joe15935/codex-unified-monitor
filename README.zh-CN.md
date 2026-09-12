# Codex Unified Monitor / Codex 统一用量监控

[English](README.md) · **简体中文**

一个在本机运行的 macOS 菜单栏与 Windows 托盘应用，集中查看 Codex 官方额度、Token、缓存表现、API 等价值，以及套餐容量的长期观察结果。应用采用 Tauri 2 和 SQLite，随应用运行一个官方 Codex app-server 子进程；安装版不启动网页服务器，不收集遥测，也不把数据同步到云端。

**[下载 v0.3.1 · macOS 与 Windows 测试版](https://github.com/Joe15935/codex-unified-monitor/releases/tag/v0.3.1)** · [套餐额度审计](AUDITOR.zh-CN.md) · [验证记录](TEST_REPORT.md) · [隐私说明（英文）](PRIVACY.md)

Windows 支持目前为测试版。构建、自动测试与 Windows 上的界面和真实账户验收分别记录在 [验证记录](TEST_REPORT.md)，不会将构建成功当成全部功能已经实测。套餐审计仍为预览功能，需要独立受控数据验证分类方法。

## 安装与切换语言

| 平台 | 安装包与要求 |
| --- | --- |
| macOS | 下载 `.dmg` 或 `.app.zip`，将 **Codex Unified Monitor.app** 放到“应用程序”。需要 Apple Silicon Mac 和 macOS 12 或更新版本；此版本未提供 Intel 安装包。 |
| Windows 测试版 | Windows 10/11 x64，下载 NSIS 安装版 `.exe`。按当前用户安装，可选择中文或英文；需要 Microsoft WebView2，缺少时安装程序可下载其引导安装器。 |

先在官方 Codex CLI 或 ChatGPT/Codex 桌面应用中登录，本组件不会要求你输入账号密码。如果 Windows 上找不到原生 `codex.exe`，请安装官方 Windows Codex CLI；仅安装在 WSL 内的 CLI 不能作为原生 Windows 额度读取进程。

默认显示中文。点击主界面右上角、设置页或菜单栏小窗中的 **中文 / English** 即可切换；选择会保存在本机，重新启动后仍然生效。主界面、小窗、托盘菜单和随后导出的 HTML 报告都会跟随语言。

点击菜单栏或系统托盘图标可打开小窗。关闭主窗口后监控继续运行；可从小窗或托盘菜单退出。登录时自动启动需要在设置中自行开启。

macOS 安装包使用临时签名，**尚未通过 Apple 公证**；Windows 测试版尚无商业代码签名。Gatekeeper 或 SmartScreen 可能要求批准。请核对发布页来源与校验和，不要全局关闭系统保护；也可按下文从源码构建。

## 可以查看什么

- 官方额度的已用与剩余比例、重置时间，以及接口提供的重置次数和独立模型额度。
- 原始输入、缓存输入、未缓存输入、输出、推理 Token、总 Token 与缓存命中率。
- 今日、最近 5 小时、本周、本月、30 天、90 天、本年、全部及自定义日期；按设置中的时区计算。
- 模型和会话明细，以及按 Token、等价值、缓存命中率、时长和输出排序的会话列表。
- 分开的公开 API 与 Codex / Work 价目表、自定义模型别名和价格；未知价格明确显示“未定价”。
- 数据充足时的额度消耗速度和实测效率，以及可选的订阅费用对比。
- 额度使用节奏参考，以及可调节的界面内低额度提示，默认剩余 10%，设为 0 关闭。它们用于日常安排，不是官方扣费规则或套餐审计证据，也不会发送系统通知。
- 默认开启自适应刷新，活跃时保持设置的间隔，空闲时降低读取频率；受控审计期间保持固定间隔。本地日志采集独立运行，监听异常会自动恢复。
- 本地 HTML、CSV、JSON 导出，以及 Pro Tier Auditor 套餐额度审计。

**API 等价值是按 Token 和公开基础价格计算的估值，不是订阅账单，也不是 OpenAI 的成本。** 历史价格、长上下文、缓存写入、服务档位和请求级价格调整未被还原。详情见 [价格口径（英文）](PRICING.md) 与 [计数口径（英文）](ACCOUNTING.md)。

额度窗口按实际持续时间识别，`primary` 字段不一定代表 5 小时。接口没有返回的字段显示“不可用”，不会当成零。官方读取失败后，旧额度明确标为“缓存”或“过期”，本地 Token 仍可查看。

默认额度读取间隔为 90 秒，自适应模式在持续空闲时最长放宽到 5 分钟；用户设置更长间隔时予以保留。关闭自适应即可固定间隔，读取失败后的退避仍然有效。切换语言、主题或价格不会额外触发账户请求。

v0.3.1 减少使用中的重复工作：倒计时只更新所在组件，合并重叠刷新请求并顺序读取，会话详情使用索引查询，数字与日期格式按语言和时区复用有上限的缓存。保留原有布局、计数口径与 Tauri/SQLite 架构，没有新增生产依赖。具体依据及尚未完成的验收见 [调研记录](docs/RESEARCH-2026-09-12.md) 和 [验证记录](TEST_REPORT.md)；局部基准不能代表整个应用提速了多少倍。

## Pro Tier Auditor / 套餐额度审计

审计页将每次观察到的周额度变化，与两个时间点之间的本机 Token 对齐，展示每消耗 1 个百分点对应的 API 等价值，并保留被排除的区间及原因。

开始受控观察前，选择预期套餐、模型、推理强度、输入长度范围和任务类型，确认 Fast、子代理及其他设备等控制条件。模型价格会在观察开始时冻结。与独立、可比的 Pro 5x 测量基准对照后，结果可能为：

| 原始标识 | 中文含义 |
|---|---|
| `PRO-5X-LIKE` | 实测容量表现更接近 Pro 5x 基准 |
| `INCONCLUSIVE` | 证据不足，或条件不具备可比性 |
| `PRO-20X-LIKE` | 实测容量表现更接近 Pro 20x 名义参考范围 |

软件不会编造 Pro 5x 的固定美元容量，也不会把容量相似性当成后台权益配置错误的证明。缺少独立基准或足够受控样本时，显示 `INCONCLUSIVE` 是预期行为。审计分类方法仍为预览，需要长期、独立的现场观察验证。

可导出脱敏证据报告供自己检查或提交 Support；软件不会替你发送。完整操作步骤和阈值见 [套餐额度审计说明](AUDITOR.zh-CN.md)。

## 双语报告与数据兼容

导出前选择语言，HTML 报告使用相应的中文或英文。日期与缩写数字跟随语言，时区保持原设置。模型标识、用户填写的名称和技术参数保留原文。

JSON 和 CSV 保留稳定的英文字段名与机器可读值，供现有工具处理及审计基准导入。语言切换不会改动 Token、价格、额度、审计结果或证据校验和。无法识别的外部错误信息保留原文。

## 从源码构建

安装 Node.js 22.12 或更新版本、Rust stable 及官方 Codex。macOS 需要 Xcode 命令行工具；Windows 需要 MSVC Rust 工具链、Visual Studio Build Tools 的“使用 C++ 的桌面开发”、Windows SDK 和 WebView2。

```sh
npm ci
npm test
npm run build
```

在目标系统上构建对应安装包：

```sh
# macOS Apple Silicon
npm run package -- --bundles app,dmg

# Windows x64
npm run package -- --target x86_64-pc-windows-msvc --bundles nsis
```

构建结果位于 `src-tauri/target/` 下所选目标的 `release/bundle/` 目录。开发使用 `npm run tauri dev`，开发服务器只监听 `127.0.0.1`。安装版没有 HTTP 服务器。

```sh
npm run data -- --help
```

数据命令会读取真实本机元数据。它的输出和导出文件属于私人数据，请勿提交到公开仓库或公开 issue；问题复现请使用合成样本。

## 数据位置与卸载

只读访问 `CODEX_HOME/sessions` 与 `CODEX_HOME/archived_sessions`，默认根目录为 `~/.codex`，在 Windows 上为 `%USERPROFILE%\.codex`。

组件自己的数据库在 macOS 上位于 `~/Library/Application Support/Codex Unified Monitor/monitor.sqlite3`，Windows 上位于 `%LOCALAPPDATA%\Codex Unified Monitor\monitor.sqlite3`。Windows 数据不进入漫游配置目录。

设置中的“断开账号连接”只停止本组件的额度读取，不会退出官方 Codex 登录。卸载前关闭“登录时启动”，然后移除应用及其自己的数据目录；不会修改 Codex 原始会话或认证文件。

## 开源与复用

采用 MIT 许可证。复用了 [CodexScope](https://github.com/poer2023/CodexScope) 的部分架构与实现，参考 [YUHAO 的用量面板](https://github.com/YUHAO-corn/codex-usage-dashboard) 的报表思路，并将 [zhang-mengjia 的用量面板](https://github.com/zhang-mengjia/codex-usage-dashboard) 的持久额度读取方式改写为 Rust。具体版本、继承许可与复用范围见 [第三方声明](THIRD_PARTY_NOTICES.md)。

[本次开源调研](docs/RESEARCH-2026-09-12.md) 同时比较了 CodexBar 和 ccusage。v0.3.0 保留已有技术栈，补充经官方格式核验的 `thread_settings_applied` 兼容；缺失或未知的速度信息不会被认定为 Fast 已关闭。这些改动没有增加生产依赖。

本项目独立维护，与 OpenAI 无隶属关系。欢迎通过公开仓库获取源码或贡献翻译；翻译结构见 [LOCALIZATION.md](LOCALIZATION.md)。
