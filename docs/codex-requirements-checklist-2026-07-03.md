# Codex PR / Issue 需求确认清单

整理时间：2026-07-03

目的：把 `docs/github-pr-issue-requirements-2026-07-02.md` 中 Codex 相关需求拆成“要确认什么、要改什么、怎么改、怎么验收、发宿主还是发 Codex zip”，并记录本轮实现状态、验证结果和仍需单独立项的边界。

## 当前架构判断

- Codex 业务主路径已经迁到平台包 sidecar：前端调用宿主 command，宿主通过 `src-tauri/src/modules/platform_adapter.rs` 调用已安装的 `cockpit-codex-adapter`。
- 账号、API 服务、本地网关、模型供应商、会话、唤醒等核心逻辑主要在 `crates/cockpit-core/src/modules/codex_*.rs`，由 `crates/cockpit-codex-adapter` 编译进 Codex 平台包。
- 只改 `crates/cockpit-core` 中被 `cockpit-codex-adapter` 使用的 Codex 业务逻辑、`crates/cockpit-codex-adapter`、`platform-packages/codex/*`，且不新增 Host API / 协议能力时，可以只发 Codex zip。
- 修改 `src-tauri/` 宿主 bridge、Host API、平台包生命周期、通用下载/安装/校验、`src/` 宿主前端页面、主应用版本或 updater 时，必须发宿主包。
- 平台 zip 依赖新宿主能力时，必须先发宿主包，再发 Codex zip，并提高 `minCoreVersion`。

## 当前实现状态总览

本节用于避免后续把“需求池”误当成“全部待开发”。状态以当前本地代码和已有未提交改动为准，发布前仍要按最新代码和真实客户端环境复核。

### 本轮已落地

- `REQ-004` 中的 `bound_oauth_use_local_gateway=false` 持久化问题：本地已在 `crates/cockpit-core/src/models/codex.rs` 和 `crates/cockpit-core/src/modules/codex_account.rs` 修复核心原因，显式 `false` 不再被保存时省略，旧数据迁移只在字段缺失时生效。
- `REQ-004` 中的 API Key provider patch 保存问题：本地已修复编辑 API Key 凭据时 `apiModelCatalog`、`apiSupportsVision`、`apiModelVisionSupport`、`apiWireApi` 等可选字段缺省导致旧 provider 元数据被清空的问题，并补单元测试。
- `REQ-004` 中的 provider 切换清理 `model_catalog_json` 问题：本地已改为只删除 Cockpit 管理的 `cockpit-provider-model-catalog.json`，切到官方或自定义 provider 时保留用户手写 catalog、插件配置和非托管 provider 表，并补回归测试。
- `REQ-CODEX-001` 账号敏感备注字段：本地已实现账号备注结构化字段、TOTP 展示、OAuth 绑定账号区域展示、导出敏感提示、导出排除敏感备注开关和 patch 保存测试。发布边界是宿主包。
- `REQ-003` 中的切号落盘读回校验：本地已在 Codex 账号 bundle 写入入口增加 `auth.json` 读回校验，API Key 账号额外校验 `codex_local_access` provider 的 key/base URL，失败会在更新 `current_account_id` 前返回错误，并保留已有 managed projection/token 轮换同步路径。
- `REQ-006` 中的 API 服务错误分类：本地已为上游 403/502/503/504/408 增加明确兜底分类；sidecar 仅上报通用 `request_failed` 时会按 HTTP status 归类，避免日志和统计继续落成固定通用错误。
- `REQ-022` 中的第三方 `/v1/models` 解析：本地已把模型列表解析扩展为统一支持 `data[].id`、`models[].id`、`models[].slug`、`models[].name` 和数组根节点，并让模型发现、连接测试摘要、模型下拉列表复用同一解析逻辑。
- `REQ-009` 用量、价格、日志：本地已补 daily/weekly/monthly 独立窗口测试，验证 SQLite `request_logs` 保留用户 API key 名称和完整错误文本；价格表已增加 `model_pricing_version`，usage event / request_logs 会保存价格版本快照，并新增“重算历史估值”入口按当前价格版本刷新请求日志和统计金额。
- `REQ-007` 会话修复 dry-run 与跨设备迁移：本地已新增会话可见性修复 `dryRun` 参数，预览只扫描和统计，不创建备份、不写 SQLite/rollout/session_index、不触发 metadata rebuild；前端修复弹框已改成“预览变更 -> 确认修复”两步。数据迁移导入实例配置时不再原样写入旧机器 `userDataDir` / `workingDir`，会把实例数据目录映射到本机 Cockpit 数据目录下的 `data-transfer/instances/<platform>/<instanceId>`。配置备份已新增 Codex 会话 Bundle，导出 rollout 内容、session_index 条目和 workspace root，导入时按实例映射写回本机会话目录、补 session_index/global state，并触发官方 metadata rebuild；导入时拒绝绝对路径、`..` 和非 rollout 文件，且不覆盖本机更新的同 session 内容。
- `REQ-008` 调度策略和唤醒任务：本地已新增 `single_account` 固定首个账号策略，legacy 和 sidecar 均只尝试一个账号，UI 策略下拉已补“固定首个账号”；当前代码已有 Codex 唤醒任务系统，支持 `quota_reset` 调度、账号选择、模型选择、确认模式、手动测试和执行历史。
- `REQ-010` 批量导入和批量删除：当前已有 adapter 后台扫描 session、quota 检测、取消/继续和 preview；本地已把关闭弹框改为隐藏任务而不是取消任务，新增页面内任务条恢复入口，并把 session 快照落盘到 Codex 数据目录，adapter/宿主重启后可通过原 session id 恢复 preview 并继续任务。批量删除已改为 adapter 后台 job 逐个删除并落盘到 `codex_batch_delete_jobs/<job_id>.json`，Tauri command 轮询 job 结束后才让现有确认弹框关闭，失败会带账号 id 和原始错误回到弹框；账号页会显示批量删除进度条、失败数和前几条失败详情，并支持暂停、继续和重试失败项。adapter 重启后运行中任务恢复为 paused，避免破坏性删除自动继续。
- `REQ-025` 测试隔离：本地已补 `TestEnvGuard` 隔离断言，覆盖 `HOME`、`CODEX_HOME`、`COCKPIT_TOOLS_DATA_DIR` 临时目录和还原。
- `REQ-026` switcher preview：当前代码未发现独立 Codex switcher preview route/command/DTO 残留，只有批量导入 preview、provider 保存 preview 和 session 修复 UI，不需要删除 preview 入口。
- Cursor 原始错误链增强不属于 Codex，已单独归入 Cursor 平台 zip 范围，不计入本 Codex 目标。

### 已有基础能力，但还需要真实环境回归

- `REQ-022` 第三方模型 `/v1/models`：当前已有 `codex_model_provider` 相关解析、模型列表、usage 查询和测试基础，不是从零开发。待确认点是多 provider 返回结构覆盖、缓存策略、拉取失败 fail-soft、保存后不清空 catalog。
- `REQ-007` 会话恢复：当前已有 `codex_session_visibility` 会话可见性修复、废纸篓恢复、跨实例修复、dry-run 预览、备份和失败回滚。待确认点主要剩跨设备路径映射的产品化入口。

### 仍需真实客户端或 UI 回归

- `REQ-004` provider/API Key 持久化：已修 provider patch、显式 false 和托管 catalog 清理；仍需真实 UI 回归 API key 名称、Base URL、插件配置在完整流程中不被覆盖。
- `REQ-003` 真实切号、token 刷新和官方 auth 兼容：已补写入后读回校验；仍需在 macOS/Windows 官方客户端真实运行态回归。
- `REQ-006` API 服务错误分类：已补主要 HTTP status 分类；stream 阶段细粒度 UI 展示仍可后续增强。

### 未纳入本轮完成项

- 当前代码层面未发现仍需在本轮继续实现的 Codex 需求；剩余项主要是真实官方客户端、真实 OAuth、真实第三方 provider 和大账号量 UI 的回归验证。

## 本轮验证记录

- `cargo fmt --all`
- `gofmt -w sidecars/cockpit-cliproxy/main.go`
- `cargo test -p cockpit-core single_account_routing -- --nocapture`
- `cargo test -p cockpit-core test_env_guard_isolates_home_codex_home_and_data_dir -- --nocapture`
- `cargo test -p cockpit-core recomputes_usage_stats_windows_independently -- --nocapture`
- `cargo test -p cockpit-core request_log_db_preserves_api_key_label_and_full_error_message -- --nocapture`
- `go test ./...`（目录：`sidecars/cockpit-cliproxy`）
- `npm run typecheck`
- `node scripts/check_locales.cjs`
- `cargo check -p cockpit-codex-adapter`
- `git diff --check`
- `npm run typecheck`（导出排除敏感备注开关补充后）
- `node scripts/check_locales.cjs`（导出排除敏感备注开关补充后）
- `cargo test -p cockpit-core batch_import_preview_restores_from_disk_snapshot -- --nocapture`
- `cargo check -p cockpit-core`（批量导入 session 快照补充后）
- `cargo test -p cockpit-core dry_run_reports_planned_changes_without_writing_files_or_backups -- --nocapture`
- `npm run typecheck`（会话修复 dry-run UI 补充后）
- `node scripts/check_locales.cjs`（会话修复 dry-run UI 补充后）
- `cargo check -p cockpit-codex-adapter`（会话修复 dry-run adapter payload 补充后）
- `git diff --check`（会话修复 dry-run 补充后）
- `cargo test -p cockpit-core request_log_reprice_updates_cost_and_pricing_version -- --nocapture`
- `cargo test -p cockpit-core request_log_db_preserves_api_key_label_and_full_error_message -- --nocapture`（历史估值重算补充后）
- `cargo check -p cockpit-codex-adapter`（历史估值重算和数据迁移路径映射补充后）
- `npm run typecheck`（历史估值重算和数据迁移路径映射补充后）
- `node scripts/check_locales.cjs`（历史估值重算补充后）
- `git diff --check`（历史估值重算和数据迁移路径映射补充后）
- `cargo fmt --all`（Codex 会话 Bundle / 批量删除 job 补充后）
- `cargo test -p cockpit-core transfer_record -- --nocapture`（会话 Bundle 路径安全与快照转换）
- `cargo check -p cockpit-codex-adapter`（会话 Bundle RPC 与批量删除持久化 job、暂停/继续/失败重试 RPC）
- `npm run typecheck`（数据迁移 Codex 会话反馈、批量删除任务条、暂停/继续/失败重试操作）
- `node scripts/check_locales.cjs`（会话导入反馈和批量删除任务条文案）
- `git diff --check`（会话 Bundle / 批量删除 job 补充后）

## 本轮发布边界判断

- 因为本轮修改了 `src/` 前端账号页、OAuth 绑定展示、批量导入弹框、批量删除任务条、会话修复 dry-run 弹框、价格历史重算 UI、数据迁移导入路径映射和会话 Bundle 反馈，`src-tauri` Codex / data transfer command，以及 Codex core/adapter，所以整体发布需要宿主包。
- 单独看 `crates/cockpit-core`、`crates/cockpit-codex-adapter`、`sidecars/cockpit-cliproxy` 内的 Codex API 服务和保存逻辑改动，理论上属于 Codex zip 可覆盖的业务边界。
- 账号敏感备注 UI、路由策略下拉、批量导入隐藏任务条、批量删除任务条、会话修复 dry-run UI、历史估值重算按钮、数据迁移路径映射、Codex 会话 Bundle 导入导出 command、导出敏感提示等前端体验依赖宿主 `src/` / `src-tauri`，不能只发 Codex zip。
- 发布前仍需在真实 macOS/Windows 官方 Codex 客户端回归账号切换、API Key provider、Base URL、OAuth 绑定备注展示和批量导入弹框恢复。

## 原始链接索引

### REQ-003 Codex 账号切换、token 刷新与官方 auth 兼容

- Issues: [#891](https://github.com/jlcodes99/cockpit-tools/issues/891), [#952](https://github.com/jlcodes99/cockpit-tools/issues/952), [#1038](https://github.com/jlcodes99/cockpit-tools/issues/1038), [#1020](https://github.com/jlcodes99/cockpit-tools/issues/1020), [#1126](https://github.com/jlcodes99/cockpit-tools/issues/1126), [#1167](https://github.com/jlcodes99/cockpit-tools/issues/1167), [#1178](https://github.com/jlcodes99/cockpit-tools/issues/1178), [#1182](https://github.com/jlcodes99/cockpit-tools/issues/1182), [#1328](https://github.com/jlcodes99/cockpit-tools/issues/1328), [#1335](https://github.com/jlcodes99/cockpit-tools/issues/1335), [#666](https://github.com/jlcodes99/cockpit-tools/issues/666), [#836](https://github.com/jlcodes99/cockpit-tools/issues/836), [#877](https://github.com/jlcodes99/cockpit-tools/issues/877), [#919](https://github.com/jlcodes99/cockpit-tools/issues/919), [#907](https://github.com/jlcodes99/cockpit-tools/issues/907), [#1050](https://github.com/jlcodes99/cockpit-tools/issues/1050), [#1058](https://github.com/jlcodes99/cockpit-tools/issues/1058), [#827](https://github.com/jlcodes99/cockpit-tools/issues/827), [#828](https://github.com/jlcodes99/cockpit-tools/issues/828)
- PRs: [PR #1325](https://github.com/jlcodes99/cockpit-tools/pull/1325), [PR #994](https://github.com/jlcodes99/cockpit-tools/pull/994), [PR #980](https://github.com/jlcodes99/cockpit-tools/pull/980)

### REQ-004 Codex API Key / provider 配置持久化不被覆盖

- Issues: [#1365](https://github.com/jlcodes99/cockpit-tools/issues/1365), [#1278](https://github.com/jlcodes99/cockpit-tools/issues/1278), [#1346](https://github.com/jlcodes99/cockpit-tools/issues/1346), [#1216](https://github.com/jlcodes99/cockpit-tools/issues/1216), [#1194](https://github.com/jlcodes99/cockpit-tools/issues/1194), [#1060](https://github.com/jlcodes99/cockpit-tools/issues/1060), [#1044](https://github.com/jlcodes99/cockpit-tools/issues/1044), [#1053](https://github.com/jlcodes99/cockpit-tools/issues/1053), [#1029](https://github.com/jlcodes99/cockpit-tools/issues/1029), [#1271](https://github.com/jlcodes99/cockpit-tools/issues/1271), [#1043](https://github.com/jlcodes99/cockpit-tools/issues/1043), [#1289](https://github.com/jlcodes99/cockpit-tools/issues/1289), [#1285](https://github.com/jlcodes99/cockpit-tools/issues/1285), [#1292](https://github.com/jlcodes99/cockpit-tools/issues/1292), [#1332](https://github.com/jlcodes99/cockpit-tools/issues/1332)
- PRs: [PR #1334](https://github.com/jlcodes99/cockpit-tools/pull/1334)

### REQ-006 API 服务协议、路由与错误兼容

- Issues: [#1324](https://github.com/jlcodes99/cockpit-tools/issues/1324), [#1176](https://github.com/jlcodes99/cockpit-tools/issues/1176), [#1301](https://github.com/jlcodes99/cockpit-tools/issues/1301), [#1321](https://github.com/jlcodes99/cockpit-tools/issues/1321), [#1230](https://github.com/jlcodes99/cockpit-tools/issues/1230), [#981](https://github.com/jlcodes99/cockpit-tools/issues/981), [#896](https://github.com/jlcodes99/cockpit-tools/issues/896), [#1172](https://github.com/jlcodes99/cockpit-tools/issues/1172), [#1204](https://github.com/jlcodes99/cockpit-tools/issues/1204), [#900](https://github.com/jlcodes99/cockpit-tools/issues/900), [#870](https://github.com/jlcodes99/cockpit-tools/issues/870), [#1109](https://github.com/jlcodes99/cockpit-tools/issues/1109), [#1125](https://github.com/jlcodes99/cockpit-tools/issues/1125), [#912](https://github.com/jlcodes99/cockpit-tools/issues/912), [#967](https://github.com/jlcodes99/cockpit-tools/issues/967), [#795](https://github.com/jlcodes99/cockpit-tools/issues/795), [#1047](https://github.com/jlcodes99/cockpit-tools/issues/1047), [#1051](https://github.com/jlcodes99/cockpit-tools/issues/1051), [#961](https://github.com/jlcodes99/cockpit-tools/issues/961), [#728](https://github.com/jlcodes99/cockpit-tools/issues/728), [#1215](https://github.com/jlcodes99/cockpit-tools/issues/1215), [#856](https://github.com/jlcodes99/cockpit-tools/issues/856), [#1070](https://github.com/jlcodes99/cockpit-tools/issues/1070)
- PRs: [PR #932](https://github.com/jlcodes99/cockpit-tools/pull/932)

### REQ-007 Codex 会话恢复、可见性、迁移与隔离

- Issues: [#1347](https://github.com/jlcodes99/cockpit-tools/issues/1347), [#1344](https://github.com/jlcodes99/cockpit-tools/issues/1344), [#1049](https://github.com/jlcodes99/cockpit-tools/issues/1049), [#1171](https://github.com/jlcodes99/cockpit-tools/issues/1171), [#1159](https://github.com/jlcodes99/cockpit-tools/issues/1159), [#1030](https://github.com/jlcodes99/cockpit-tools/issues/1030), [#712](https://github.com/jlcodes99/cockpit-tools/issues/712), [#754](https://github.com/jlcodes99/cockpit-tools/issues/754), [#998](https://github.com/jlcodes99/cockpit-tools/issues/998), [#916](https://github.com/jlcodes99/cockpit-tools/issues/916), [#1307](https://github.com/jlcodes99/cockpit-tools/issues/1307), [#987](https://github.com/jlcodes99/cockpit-tools/issues/987), [#957](https://github.com/jlcodes99/cockpit-tools/issues/957), [#993](https://github.com/jlcodes99/cockpit-tools/issues/993), [#1329](https://github.com/jlcodes99/cockpit-tools/issues/1329), [#1023](https://github.com/jlcodes99/cockpit-tools/issues/1023), [#1161](https://github.com/jlcodes99/cockpit-tools/issues/1161), [#1181](https://github.com/jlcodes99/cockpit-tools/issues/1181), [#1258](https://github.com/jlcodes99/cockpit-tools/issues/1258), [#811](https://github.com/jlcodes99/cockpit-tools/issues/811)
- PRs: [PR #1212](https://github.com/jlcodes99/cockpit-tools/pull/1212)

### REQ-008 Codex API 账号调度、轮询与额度启动策略

- Issues: [#1342](https://github.com/jlcodes99/cockpit-tools/issues/1342), [#1270](https://github.com/jlcodes99/cockpit-tools/issues/1270), [#1274](https://github.com/jlcodes99/cockpit-tools/issues/1274), [#1222](https://github.com/jlcodes99/cockpit-tools/issues/1222), [#1195](https://github.com/jlcodes99/cockpit-tools/issues/1195), [#801](https://github.com/jlcodes99/cockpit-tools/issues/801), [#796](https://github.com/jlcodes99/cockpit-tools/issues/796), [#1039](https://github.com/jlcodes99/cockpit-tools/issues/1039), [#1155](https://github.com/jlcodes99/cockpit-tools/issues/1155), [#1007](https://github.com/jlcodes99/cockpit-tools/issues/1007), [#1006](https://github.com/jlcodes99/cockpit-tools/issues/1006), [#1357](https://github.com/jlcodes99/cockpit-tools/issues/1357), [#718](https://github.com/jlcodes99/cockpit-tools/issues/718), [#969](https://github.com/jlcodes99/cockpit-tools/issues/969)

### REQ-009 用量、价格、日志与额度透明化

- Issues: [#660](https://github.com/jlcodes99/cockpit-tools/issues/660), [#990](https://github.com/jlcodes99/cockpit-tools/issues/990), [#802](https://github.com/jlcodes99/cockpit-tools/issues/802), [#1137](https://github.com/jlcodes99/cockpit-tools/issues/1137), [#1193](https://github.com/jlcodes99/cockpit-tools/issues/1193), [#1042](https://github.com/jlcodes99/cockpit-tools/issues/1042), [#1036](https://github.com/jlcodes99/cockpit-tools/issues/1036), [#700](https://github.com/jlcodes99/cockpit-tools/issues/700), [#706](https://github.com/jlcodes99/cockpit-tools/issues/706), [#1239](https://github.com/jlcodes99/cockpit-tools/issues/1239), [#1317](https://github.com/jlcodes99/cockpit-tools/issues/1317), [#1302](https://github.com/jlcodes99/cockpit-tools/issues/1302), [#1318](https://github.com/jlcodes99/cockpit-tools/issues/1318), [#1306](https://github.com/jlcodes99/cockpit-tools/issues/1306), [#988](https://github.com/jlcodes99/cockpit-tools/issues/988), [#1004](https://github.com/jlcodes99/cockpit-tools/issues/1004), [#1169](https://github.com/jlcodes99/cockpit-tools/issues/1169), [#904](https://github.com/jlcodes99/cockpit-tools/issues/904), [#963](https://github.com/jlcodes99/cockpit-tools/issues/963), [#859](https://github.com/jlcodes99/cockpit-tools/issues/859), [#1263](https://github.com/jlcodes99/cockpit-tools/issues/1263), [#1267](https://github.com/jlcodes99/cockpit-tools/issues/1267), [#1358](https://github.com/jlcodes99/cockpit-tools/issues/1358), [#722](https://github.com/jlcodes99/cockpit-tools/issues/722)
- PRs: [PR #1315](https://github.com/jlcodes99/cockpit-tools/pull/1315), [PR #1312](https://github.com/jlcodes99/cockpit-tools/pull/1312), [PR #1319](https://github.com/jlcodes99/cockpit-tools/pull/1319)

### REQ-010 批量导入、批量操作、筛选和大规模账号管理

- Issues: [#1286](https://github.com/jlcodes99/cockpit-tools/issues/1286), [#1185](https://github.com/jlcodes99/cockpit-tools/issues/1185), [#1166](https://github.com/jlcodes99/cockpit-tools/issues/1166), [#1165](https://github.com/jlcodes99/cockpit-tools/issues/1165), [#1000](https://github.com/jlcodes99/cockpit-tools/issues/1000), [#717](https://github.com/jlcodes99/cockpit-tools/issues/717), [#1148](https://github.com/jlcodes99/cockpit-tools/issues/1148), [#1110](https://github.com/jlcodes99/cockpit-tools/issues/1110), [#1133](https://github.com/jlcodes99/cockpit-tools/issues/1133), [#992](https://github.com/jlcodes99/cockpit-tools/issues/992), [#645](https://github.com/jlcodes99/cockpit-tools/issues/645), [#999](https://github.com/jlcodes99/cockpit-tools/issues/999), [#730](https://github.com/jlcodes99/cockpit-tools/issues/730), [#681](https://github.com/jlcodes99/cockpit-tools/issues/681), [#1156](https://github.com/jlcodes99/cockpit-tools/issues/1156), [#1059](https://github.com/jlcodes99/cockpit-tools/issues/1059)
- PRs: [PR #1286](https://github.com/jlcodes99/cockpit-tools/pull/1286)

### REQ-022 Codex 第三方模型下拉与模型 catalog

- Issues: [#1356](https://github.com/jlcodes99/cockpit-tools/issues/1356), [#1326](https://github.com/jlcodes99/cockpit-tools/issues/1326), [#955](https://github.com/jlcodes99/cockpit-tools/issues/955), [#1168](https://github.com/jlcodes99/cockpit-tools/issues/1168), [#910](https://github.com/jlcodes99/cockpit-tools/issues/910), [#936](https://github.com/jlcodes99/cockpit-tools/issues/936), [#985](https://github.com/jlcodes99/cockpit-tools/issues/985), [#1187](https://github.com/jlcodes99/cockpit-tools/issues/1187)
- PRs: [PR #1338](https://github.com/jlcodes99/cockpit-tools/pull/1338)

### REQ-CODEX-001 Codex 账号敏感备注字段

- 来源：用户新增需求，2026-07-03。

### REQ-025 Codex 测试隔离与开发质量

- PRs: [PR #816](https://github.com/jlcodes99/cockpit-tools/pull/816)

### REQ-026 Codex switcher preview

- PRs: [PR #950](https://github.com/jlcodes99/cockpit-tools/pull/950)

## REQ-003 Codex 账号切换、token 刷新与官方 auth 兼容

来源：#891, #952, #1038, #1020, #1126, #1167, #1178, #1182, #1328, #1335, #666, #836, #877, #919, #907, #1050, #1058, #827, #828, PR #1325, PR #994, PR #980

### 用户需求

- Codex 切号不能只改前端状态，必须让官方 Codex 客户端真实识别当前账号。
- 导入当前账号新 token 后，应立即重新激活，不需要用户再手动切一次。
- 兼容官方 `auth.json`、`config.toml`、profile 状态、`auth_mode=chatgpt`、Team/Plus 多账号。
- 处理官方客户端或 CPA 刷新 token 后的 refresh token 轮换，避免 `refresh_token_reused`。
- access-token-only 绑定 OAuth 的 API 服务 auth 文件要可用。
- 默认 Codex 实例识别不能强制要求 `CODEX_HOME`，默认官方主进程应按默认上下文识别。

### 需要确认

- 当前官方 Codex 版本的 `auth.json`、`config.toml`、profile 文件真实字段和更新时机。
- OAuth、API Key、本地网关 provider gateway、多开实例分别写哪些目录。
- 官方客户端刷新 token 后，刷新结果写回哪里；Cockpit 应从哪里同步。
- 默认实例与受管多开实例的 PID、窗口定位、`CODEX_HOME` 匹配规则。

### 需要改什么

- 统一账号切换落盘路径：避免账号总览、provider 绑定、多开实例启动各自写一套 auth/config。
- 切号成功判定改为“写入后读回 + 必要时检查运行态”，不能只看函数返回。
- 导入当前账号 token 后，走同一套重新激活逻辑，确保运行态使用新 token。
- token 刷新后要 merge 回 Cockpit 账号存储，保留新 refresh token。
- access-token-only 绑定 OAuth 时，生成 API 服务需要的 auth 文件，且不要误覆盖完整 OAuth 账号。
- 默认实例相关逻辑保持 `None` / 默认上下文，不把默认实例改成受管目录精确匹配。

### 可能涉及文件

- `crates/cockpit-core/src/modules/codex_account.rs`
- `crates/cockpit-core/src/modules/codex_oauth.rs`
- `crates/cockpit-core/src/modules/codex_local_access.rs`
- `crates/cockpit-core/src/modules/codex_instance.rs`
- `crates/cockpit-codex-adapter/src/main.rs`
- `src-tauri/src/commands/codex.rs`
- `src-tauri/src/commands/codex_instance.rs`
- `src-tauri/src/modules/platform_adapter.rs`

### 怎么改

1. 先做官方落盘 fixture：准备 OAuth、API Key、Team/Plus、access-token-only 的样例 `auth.json` / `config.toml`。
2. 抽出或收敛一个 Codex auth/config 写入入口，所有切号和 provider gateway 准备都走同一路径。
3. 写入后立即读回并校验：当前账号 id、auth mode、token 字段、profile 目录都应匹配目标账号。
4. token refresh 同步时只更新 token 字段，不重置备注、标签、provider 配置和额度缓存。
5. 默认实例相关代码单独加回归测试，验证无 `CODEX_HOME` 的官方主进程能被识别。

### 验收

- 导入当前账号新 token 后，官方 Codex 立即使用新账号。
- OAuth / API Key / Team / Plus 账号切换后，官方文件读回一致。
- refresh token 轮换后，Cockpit 不再继续使用旧 refresh token。
- 默认桌面实例无 `CODEX_HOME` 时仍可识别、聚焦和判断运行态。
- Windows 和 macOS 默认实例都要验证。

### 发布边界

- 只改 `cockpit-core` / `cockpit-codex-adapter` 内部业务逻辑时，可发 Codex zip。
- 涉及 `src-tauri` 默认实例识别、窗口定位、Host command、platform adapter 错误链时，需要宿主包。

## REQ-004 Codex API Key / provider 配置持久化不被覆盖

来源：#1365, #1278, #1346, #1216, #1194, #1060, #1044, #1053, #1029, #1271, #1043, #1289, #1285, #1292, #1332, PR #1334

### 用户需求

- 保存、切换或启动 API 服务时，不覆盖用户自定义 API key 名称。
- Base URL 不因重启、切号、API 服务启动被清空或回滚官方地址。
- `model_provider`、插件配置、`model_catalog_json` 不被错误重写。
- 显式 `false` 必须持久化，不能被迁移逻辑当成旧数据重置。
- 切回非 API_KEY provider 时，只清理 Cockpit 自己管理的 catalog 指针，不删除用户自定义配置。

### 当前本地线索

- 本地已有 `bound_oauth_use_local_gateway=false` 持久化相关改动：
  - `crates/cockpit-core/src/models/codex.rs`
  - `crates/cockpit-core/src/modules/codex_account.rs`
  - `src-tauri/src/models/codex.rs`
- 其中 adapter 侧改动可通过 Codex zip 生效；宿主 DTO 同步改动需要宿主包才会更新到用户宿主。

### 需要确认

- API Key 账号保存、provider 保存、OAuth 绑定、本地接入启动分别调用哪些更新函数。
- 哪些字段是 Cockpit 管理字段，哪些字段是用户手写配置，哪些字段来自官方 Codex。
- 迁移旧数据时，缺字段和显式 `false` 的语义差异。

### 需要改什么

- API Key/provider 更新改成 patch 语义：只更新用户提交字段，不用默认值重建整个账号。
- `bound_oauth_use_local_gateway=false` 必须写入 JSON。
- 缺失 `bound_oauth_use_local_gateway` 的旧绑定账号可按兼容逻辑迁移为 `true`，但显式 `false` 必须保留。
- provider 保存时保留 API key label、Base URL、wire API、provider id/name、model catalog、vision 配置。
- 切换 provider 或启动 API 服务时，不得清空用户自定义 `model_provider` 或 plugin 配置。

### 可能涉及文件

- `crates/cockpit-core/src/models/codex.rs`
- `crates/cockpit-core/src/modules/codex_account.rs`
- `crates/cockpit-core/src/modules/codex_model_provider.rs`
- `crates/cockpit-core/src/modules/codex_local_access.rs`
- `crates/cockpit-codex-adapter/src/main.rs`
- `src/services/codexModelProviderService.ts`
- `src/components/codex/CodexModelProviderManager.tsx`

### 怎么改

1. 列出 CodexAccount 中 provider/API Key 相关字段，标注字段归属。
2. 为保存函数补“旧值 + patch -> 新值”的测试，覆盖字段不丢失。
3. 对显式布尔字段取消 `skip_serializing_if = "is_false"`，或在保存层强制写入。
4. 对迁移逻辑加 raw JSON 判断：字段缺失才迁移，字段存在则尊重用户值。
5. provider 切换只清理 Cockpit 自动生成的 catalog 引用，保留用户手写 catalog/plugin 配置。

### 验收

- API key 名称保存后重启不变。
- Base URL 保存后切号、启动 API 服务、重启 App 都不回滚。
- `bound_oauth_use_local_gateway=false` 保存、重开页面、重启 App 后仍是 false。
- provider catalog 拉取失败时不会清空上一次可用缓存。
- 单元测试覆盖显式 false、字段缺失迁移、provider patch 保存。

### 发布边界

- adapter 内保存/迁移逻辑可发 Codex zip。
- 如果前端表单状态、宿主 DTO、Host command 参数需要调整，则需要宿主包。

## REQ-006 API 服务协议、路由与错误兼容

来源：#1324, #1176, #1301, #1321, #1230, #981, #896, #1172, #1204, #900, #870, #1109, #1125, #912, #967, #795, #1047, #1051, #961, #728, #1215, #856, #1070, PR #932

### 用户需求

- 兼容 OpenAI Responses、Chat Completions、Anthropic/Claude Code、WSL/LAN 访问等常见客户端。
- 错误要区分上游 401/403/429/502/504、本地网关错误、stream 中断、请求体过大、超时。
- 图片生成尺寸和能力要按上游真实响应处理，不能虚报 2K/4K 能力。
- 支持误拼路径，例如把 `/v1/responses` 拼到 `/v1/chat/completions` 后面。

### 需要确认

- 当前 API 服务入口支持哪些 path、method、body schema。
- 错误日志和前端日志页现在记录的是原始错误、分类错误还是固定文案。
- image_generation 能力由哪里判断：账号、provider、模型 catalog、上游错误还是本地开关。

### 需要改什么

- 路由归一化：兼容常见错误 path，但不能吞掉不明确的未知路径。
- 建立统一错误分类：`upstream_auth`、`upstream_quota`、`upstream_timeout`、`gateway_transport`、`stream_incomplete` 等。
- stream 过程记录中断原因和已收到 token/字节，避免只显示通用失败。
- image_generation 尺寸和能力以 provider/model 配置和上游真实错误为准。
- LAN/WSL 访问错误提示要区分 bind host、端口、防火墙、代理问题。

### 可能涉及文件

- `crates/cockpit-core/src/modules/codex_local_access.rs`
- `crates/cockpit-core/src/modules/codex_protocol.rs`
- `src/types/codexLocalAccess.ts`
- `src/services/codexLocalAccessService.ts`
- `src/components/codex/*LocalAccess*`

### 怎么改

1. 先加 path normalize 和错误分类单元测试。
2. 把上游响应状态、错误 body、transport 错误链写入 usage event。
3. 前端日志页显示分类、状态码、原始错误摘要和可展开详情。
4. image_generation 能力失败后更新账号或模型健康状态，但不要永久隐藏模型。

### 验收

- 401/403/429/502/504 能在日志和 UI 中明确区分。
- stream 中断不再只显示“请求失败”，能看到中断阶段和上游/本地分类。
- 错误拼接 path 有兼容结果，未知 path 仍返回明确 404/400。
- 图片尺寸能力按 provider 实测结果展示，不写死夸大。

### 发布边界

- API 服务核心逻辑在 Codex adapter 内，可优先发 Codex zip。
- 前端日志页新增字段展示需要宿主包，除非 Codex remote UI 已完全承载对应页面。

## REQ-007 Codex 会话恢复、可见性、迁移与隔离

来源：#1347, #1344, #1049, #1171, #1159, #1030, #712, #754, #998, #916, #1307, #987, #957, #993, #1329, #1023, #1161, #1181, #1258, #811, PR #1212

### 用户需求

- 切换中转/个人账号后，会话不能消失。
- 恢复删除会话要重建官方 metadata、SQLite thread、`session_index.jsonl` 和 sidebar 状态。
- 跨设备 Bundle 导入要保留会话、项目、实例 root 和必要 metadata。
- 多开实例会话互不污染。
- 永久删除与恢复备份要有二次确认和可逆路径。

### 需要确认

- 当前官方 Codex 会话相关文件：metadata、SQLite thread、session index、sidebar 状态。
- 不同官方版本的会话字段是否兼容。
- 多开实例的 data dir / project root / session index 如何隔离。
- 跨设备导入时哪些本机路径应该映射，哪些路径不能强写。

### 需要改什么

- 建立会话扫描和修复管线：发现缺 metadata、缺 SQLite row、缺 index entry 时按规则修复。
- 跨设备导入要做路径映射层，不把原机器绝对路径直接写入当前机器。
- 恢复删除前先备份现有状态，永久删除二次确认。
- 多实例操作必须带实例上下文，避免默认实例和多开实例共用 session index。

### 可能涉及文件

- `crates/cockpit-core/src/modules/codex_session_visibility.rs`
- `crates/cockpit-core/src/modules/codex_account.rs`
- `crates/cockpit-core/src/modules/codex_instance.rs`
- `crates/cockpit-core/src/modules/data_transfer.rs`
- `src-tauri/src/commands/codex_instance.rs`
- `src/components/codex/*Session*`

### 怎么改

1. 先做官方会话文件 inventory 文档和 fixture。
2. 把“扫描、预览、修复、备份、回滚”拆成独立步骤，避免一次性直接写。
3. 修复前输出 dry-run 结果：会新增/修改/跳过哪些记录。
4. Bundle 导入增加路径映射策略和冲突处理策略。
5. 多实例会话读写必须显式传实例目录。

### 当前实现状态

- 当前已有 `codex_session_visibility` 修复链路，支持 quick/deep 模式、跨实例扫描、官方 state SQLite 修复、会话时间校正、`session_index.jsonl` 补写/刷新和 metadata rebuild。
- 修复写入前会调用 `backup_instance_files` 备份目标 rollout、SQLite 和 `session_index.jsonl`；写入失败时会尝试 `restore_instance_files_from_backup` 回滚。
- `CodexSessionManager` 已提供会话修复入口、废纸篓恢复入口、恢复二次确认和恢复进度展示，恢复会补回 rollout、`session_index` 和文件时间并触发官方索引重建。
- 多实例修复入口按实例 data dir 操作，summary 会记录每个实例的 mutated count、backup dir 和 metadata rebuild 失败数。
- 会话可见性修复已暴露用户可见 dry-run 模式：`dryRun=true` 时只扫描并返回预计改动数量，`backup_dirs` 为空，不写 SQLite/rollout/session_index、不创建备份、不触发 metadata rebuild；前端弹框必须先“预览变更”，有预计改动后才能“确认修复”。
- 数据迁移导入实例 store 时已做跨设备路径安全映射：导入的 `userDataDir` 不再原样使用旧机器绝对路径，而是映射到本机 Cockpit 数据目录下的 `data-transfer/instances/<platform>/<instanceId>`；导入的 `workingDir` 不跨设备恢复，避免把不存在的旧项目路径写入本机配置。
- 数据迁移配置 Bundle 已包含 Codex 会话迁移块：导出每个实例的 rollout 内容、`session_index.jsonl` 条目和 workspace root；导入时先恢复实例 store，再按实例 id/default 实例映射到本机数据目录，写回缺失或更旧的 rollout，补 `session_index.jsonl` 和 `.codex-global-state.json`，并触发官方 metadata rebuild。导入不直接搬旧机器 SQLite 数据库，而是依赖 rollout/session_index/global state 让官方索引重建，避免跨设备 SQLite 和绝对路径污染。
- 会话 Bundle 导入带路径安全校验：拒绝绝对路径、`..`、非 `sessions` / `archived_sessions` 下的路径和非 `rollout-*.jsonl` 文件；同一 session 若本机更新则跳过，不覆盖本机较新的内容。

### 验收

- 删除恢复后官方客户端可见，sidebar 状态正确。
- 跨设备导入不会写入不存在的旧机器路径。
- 多开实例 A 的会话不会出现在实例 B。
- 修复失败可回滚到修复前备份。

### 发布边界

- 纯 adapter/core 会话修复可发 Codex zip。
- dry-run 的 core/adapter 统计逻辑可随 Codex zip 生效；本轮新增的宿主 command 参数、TS 类型和弹框两步 UI 需要宿主包。
- 涉及宿主导入导出 UI、文件选择、系统 opener、全局备份入口时需要宿主包。

## REQ-008 Codex API 账号调度、轮询与额度启动策略

来源：#1342, #1270, #1274, #1222, #1195, #801, #796, #1039, #1155, #1007, #1006, #1357, #718, #969

### 用户需求

- API 服务账号池支持关闭轮询、固定单账号、按优先级/额度调度、多个账号池、自动切号阈值。
- 定时轻量请求启动 5h 额度窗口，但必须明确会消耗额度，并可关闭、限频、按账号选择。
- 刷新配额或调度不能阻塞正在服务的 API 请求。

### 需要确认

- 当前账号选择策略、配额刷新策略、错误冷却策略分别在哪里实现。
- 5h 额度窗口启动请求的真实消耗和官方行为是否稳定。
- 用户要的是全局策略、API key 级策略，还是 provider gateway/profile 级策略。

### 需要改什么

- 路由策略配置持久化：固定账号、轮询、额度优先、优先级、失败回退。
- 账号池分组：按 API key、provider、标签或手动池选择。
- 额度启动任务：可关闭、限频、明确消耗提示、失败冷却。
- 调度原因可观测：日志中记录为什么选中/跳过某账号。

### 可能涉及文件

- `crates/cockpit-core/src/modules/codex_local_access.rs`
- `crates/cockpit-core/src/modules/codex_account.rs`
- `src/types/codexLocalAccess.ts`
- `src/components/codex/CodexLocalAccessModal.tsx`

### 怎么改

1. 先定义策略数据结构并做迁移默认值。
2. 调度器返回选中账号时附带 reason，写入 usage event。
3. 配额刷新放后台，API 请求只读取最近可用快照。
4. 额度启动任务单独队列，不和用户请求抢锁。

### 当前实现状态

- 当前已有 `auto`、高/低配额优先、高/低订阅优先、近到期优先和自定义权重调度策略，并持久化到本地接入 collection。
- 本地已新增 `single_account` 固定首个账号策略，用于关闭账号轮询；legacy 请求派发和 websocket 派发都会把凭据尝试数限制为 1。
- Codex API 服务 sidecar manifest 已传递 `single_account`，并在 sidecar config 中把 `max-retry-credentials` 强制为 1，避免 sidecar 失败后继续轮到其他账号。
- 前端 `CodexLocalAccessModal` 和 `CodexApiServicePage` 策略下拉已新增“固定首个账号”，18 个 locale 已补对应 key。
- 已补 `single_account_routing_keeps_first_account_without_rotation` 和 `single_account_routing_limits_credential_attempts_to_one` 回归测试。
- 当前已有 Codex 唤醒任务系统，支持创建 `quota_reset` 调度任务，按账号选择执行，配置模型和 reasoning effort，支持确认模式、启动后延迟、手动测试、取消测试 scope、执行历史和页面内结果进度。
- 账号页已有“唤醒账号”入口，默认按 5h 额度相关排序辅助选择账号；唤醒执行文案已提示会发起真实请求并可能消耗额度。
- 待真实环境回归点：官方 5h reset 时间识别是否稳定、唤醒请求的真实额度消耗是否符合提示、失败冷却和多账号并发下是否影响正在服务的 API 请求。

### 验收

- 可关闭轮询并固定当前账号。
- 自动切号能显示触发原因：额度不足、账号冷却、401、用户优先级等。
- 额度启动任务可关闭，开启时有消耗提醒。
- 高并发 API 请求不被批量刷新配额阻塞。

### 发布边界

- 调度核心可发 Codex zip。
- 新增/调整宿主前端设置 UI 需要宿主包，除非该 UI 已迁到 Codex remote UI。

## REQ-009 用量、价格、日志与额度透明化

来源：#660, #990, #802, #1137, #1193, #1042, #1036, #700, #706, #1239, #1317, #1302, #1318, #1306, #988, #1004, #1169, #904, #963, #859, #1263, #1267, #1358, #722, PR #1315, PR #1312, PR #1319

### 用户需求

- 请求日志账号列显示用户设置的账号/key 名称。
- 悬浮错误详情显示完整文本，不截断关键信息。
- 支持日/周/月筛选，修正周用量和月用量相同的问题。
- 支持模型 price book、长上下文/priority 档位、历史估算价值重算。
- 展示 token input/output、单次高消耗标记、账号负载对比、模型 latency/连通性测试。

### 需要确认

- 当前 usage event schema 是否保存账号名快照、API key 名称快照、模型价格快照。
- 日/周/月统计窗口是否按时间过滤，还是复用同一缓存。
- price book 来源和更新方式：内置、用户配置、provider 拉取或手动导入。

### 需要改什么

- usage event 增加或确认保存 account label / API key label 快照。
- 统计查询按窗口独立计算，避免周/月复用。
- price book 支持版本化和历史重算。
- 错误详情保留完整文本，UI 做折叠展示而不是后端截断。

### 可能涉及文件

- `crates/cockpit-core/src/modules/codex_local_access.rs`
- `src/types/codexLocalAccess.ts`
- `src/components/codex/*LocalAccess*`
- `src/locales/*.json`

### 怎么改

1. 先补统计窗口测试：daily、weekly、monthly 使用不同时间范围。
2. usage event 写入时保存可读名称快照。
3. price book 单独版本字段，历史重算时记录使用的价格版本。
4. UI 表格列宽和错误详情要做窄宽度检查。

### 当前实现状态

- 后端 usage event 已保存 `accountId`、`email`、`apiKeyId`、`apiKeyLabel`、`modelId`、HTTP status、错误分类、完整错误文本、token 和价格快照。
- SQLite `request_logs` 已持久化 `api_key_label` 和完整 `error_message`，新增 `request_log_db_preserves_api_key_label_and_full_error_message` 回归测试防止写入层截断。
- daily、weekly、monthly 已按独立时间窗口从事件重算，不复用同一缓存；新增 `recomputes_usage_stats_windows_independently` 回归测试覆盖周/月统计不同。
- price book 已增加 `model_pricing_version`，保存价格表时仅在价格实际变化时递增版本；usage event 和 SQLite `request_logs` 都会保存请求发生时的价格版本和价格快照。
- 模型价格弹框已显示当前价格版本，并新增“重算历史估值”按钮；点击后会按当前价格版本重写请求日志中的估算金额、价格版本和价格快照，并从日志重建统计窗口。
- 已补 `request_log_reprice_updates_cost_and_pricing_version` 回归测试覆盖历史日志重算。
- 更丰富的 UI 负载对比、单次高消耗标记和 latency 分析仍可作为后续展示增强，不影响当前日志准确性和历史估值重算闭环。

### 验收

- 周用量和月用量在不同数据下不同。
- 日志账号列显示用户设置名称，而不是内部 ID。
- 错误 hover / 展开区能看到完整错误详情。
- 历史估算价值重算前后有明确提示。

### 发布边界

- 日志写入和统计核心可发 Codex zip。
- UI 表格、悬浮详情和筛选控件需要宿主包，除非对应页面已 remote 化。

## REQ-010 批量导入、批量操作、筛选和大规模账号管理

来源：#1286, #1185, #1166, #1165, #1000, #717, #1148, #1110, #1133, #992, #645, #999, #730, #681, #1156, #1059, PR #1286

### 用户需求

- 批量导入任务离开弹框后继续执行，可恢复进度。
- 导入前可检查额度、过滤 0 额度/授权失败账号，并批量打标签。
- 支持按额度、错误状态、标签过滤。
- 删除标签、批量删除账号等破坏性操作必须二次确认。
- 大量账号场景不能卡死 UI。

### 需要确认

- 当前批量导入是在前端循环还是 adapter 后台执行。
- 账号列表缓存、分页、筛选和刷新是否会一次性渲染所有账号。
- 错误状态字段是否统一：过期、401、授权撤销、刷新失败、0 额度。

### 需要改什么

- 批量导入改后台 job：进度、错误、取消、恢复都在 adapter/core 持久化。
- 前端只订阅 job 状态，不负责长循环。
- 筛选条件统一用账号状态字段，避免前端临时推断。
- 大批量删除/导出走后台任务并二次确认。

### 可能涉及文件

- `crates/cockpit-core/src/modules/codex_account.rs`
- `crates/cockpit-codex-adapter/src/main.rs`
- `src/stores/useCodexAccountStore.ts`
- `src/pages/CodexAccountsContent.tsx`
- `src/components/codex/*`

### 怎么改

1. 增加 batch job 数据结构和持久化文件。
2. adapter methods 提供 create/list/get/cancel job。
3. 导入/检查/打标签/删除都复用同一进度模型。
4. 前端账号列表加分页/虚拟化或至少避免全量重渲染。

### 当前实现状态

- 当前已有 `CodexBatchImportSession` 后台扫描 session，支持 `startFromFiles`、`preview`、`cancel`、`resume`、`confirm`，扫描过程由 adapter/core 后台任务执行，前端订阅事件进度。
- 批量导入已支持文件解析失败项、逐条账号 preview、quota 检测、异常/已存在/无效分组、选择正常账号、取消和继续扫描。
- 本地已修复关闭弹框会取消后台任务的问题：关闭只隐藏弹框，任务继续执行；显式“取消扫描/取消解析”才会调用 cancel。
- 页面顶部已新增隐藏任务条，用户关闭弹框后可以重新打开查看最新进度；通过 `localStorage` 记录 active session id，并在页面恢复时重新拉取 preview。
- 当前 session 已在 Codex 数据目录写入 `codex_batch_import_sessions/<session_id>.json` 快照，保存 source values、扫描进度、preview、draft 和 quota 快照；adapter 或宿主重启后，`preview` / `resume` / `confirm` 命令会在内存缺失时从磁盘恢复 session，确认导入后删除快照。
- 大规模账号列表已有搜索、标签/分组/状态筛选、`paginatedAccounts` 分页渲染和布局切换；当前不会在 overview 主列表一次性渲染全部过滤结果。
- 批量删除账号已通过通用账号页 hook 做二次确认，失败会留在确认弹框内展示，不会静默关闭。
- 批量删除底层已改成 adapter 后台 job：`accounts.batchDelete.start` 创建 job，后台逐个删除账号并同步清理本地接入账号池，`accounts.batchDelete.get` 可查询 total / completed / failed / errors；现有 `delete_codex_accounts` command 会轮询 job 完成再返回，从而保持当前确认弹框行为不回退。
- 批量删除 job 已落盘到 Codex 数据目录 `codex_batch_delete_jobs/<job_id>.json`，每删除一个账号都会更新 completed/failed/errors/next_index；adapter 重启后 running 任务恢复为 paused，不会自动继续破坏性删除。
- 账号页已新增批量删除进度条，展示完成数、总数、失败数和前几条失败详情；失败仍会回到确认弹框内展示完整账号 id + 原始错误。
- 页面任务条已支持用户主动暂停/继续，以及对失败项重新发起删除。

### 验收

- 1000 个账号导入时，关闭弹框后任务继续。
- 任务失败项可导出或重试。
- 批量删除必须二次确认，失败后弹框保持打开并展示错误。
- 账号列表筛选不卡死主线程。

### 发布边界

- 后台 job 核心可发 Codex zip。
- 前端 job UI、列表虚拟化、弹框交互需要宿主包，除非 Codex 页面已 remote 化。

## REQ-022 Codex 第三方模型下拉与模型 catalog

来源：#1356, #1326, #955, #1168, #910, #936, #985, #1187, PR #1338

### 用户需求

- Codex API_KEY / 第三方 provider 能通过 `/v1/models` 生成本地 `model_catalog_json`。
- 模型下拉可选，不再只能填“自定”。
- 支持不同 provider 返回结构：`data[].id`、`models[].id`、`models[].slug`、`models[].name`、数组根节点。
- 拉取失败 fail-soft 使用缓存，不阻塞启动。
- 未命中官方模型缓存时补齐 Codex 所需字段，避免启动解析失败。

### 需要确认

- 当前 provider 配置存在前端 localStorage、账号 JSON、还是 Codex 官方 config。
- `/v1/models` 拉取应该用用户 API key、provider API key，还是本地网关 key。
- Codex 官方需要的 `model_catalog_json` 最小字段集合。

### 需要改什么

- provider model fetcher：支持多种返回结构和错误分类。
- catalog cache：按 provider/baseURL/API key 维度缓存，记录更新时间和错误。
- fail-soft：拉取失败时保留旧缓存并提示，不清空模型列表。
- catalog 写入时补齐 Codex 所需字段，避免官方启动失败。
- 前端模型下拉支持刷新、使用缓存、手动输入兜底。

### 可能涉及文件

- `crates/cockpit-core/src/modules/codex_model_provider.rs`
- `crates/cockpit-core/src/modules/codex_account.rs`
- `crates/cockpit-codex-adapter/src/main.rs`
- `src/services/codexModelProviderService.ts`
- `src/components/codex/CodexModelProviderManager.tsx`

### 怎么改

1. 先定义 catalog entry 标准格式，确保能写入 Codex 官方需要的字段。
2. 增加 `/v1/models` parser 单元测试，覆盖常见 provider shape。
3. fetch 失败只更新错误状态，不删除旧 catalog。
4. UI 中把“刷新模型”“使用缓存”“手动输入”分开。

### 验收

- OpenRouter / OpenCode / DeepSeek 等常见 `/v1/models` 返回能解析。
- provider 断网或 401 时，旧模型下拉仍可用。
- 未知模型也能补齐 Codex 启动所需字段。
- 保存 provider 后重启不丢 catalog。

### 发布边界

- parser/cache/write 核心可发 Codex zip。
- 模型下拉 UI 调整需要宿主包，除非模型供应商页已 remote 化。

## REQ-CODEX-001 Codex 账号敏感备注字段

来源：用户新增需求，2026-07-03

### 用户需求

- Codex 账号备注需要支持结构化字段：2FA secret、密码、手机号、其他备注。
- 现有的备注字段继续保留语义，但迁移为“其他备注”。
- 用户输入 2FA secret 后，相当于该账号绑定这个 2FA secret。
- 在账号备注/账号详情中可以看到这些字段。
- 在 OAuth 授权操作时，也能看到该账号绑定的 2FA secret 和对应的当前动态验证码，方便登录、验证和账号操作。
- 2FA secret 和动态验证码都需要显示，并提供复制能力。

### 需要确认

- 这些字段只用于 Codex 账号，还是未来要抽成通用账号字段。
- 2FA secret 是否只支持标准 TOTP Base32，是否允许空格、分组、大小写混输。
- 动态验证码默认算法：通常为 TOTP 6 位、30 秒周期、SHA-1；如果要支持其它算法，需要额外字段。
- 密码、2FA secret、手机号是否参与导入/导出；如果参与，是否需要单独确认和加密说明。
- OAuth 授权弹框里展示这些信息的位置：账号选择列表、账号详情展开区，还是授权确认区。
- 是否默认直接显示敏感值，还是默认折叠/点击显示。用户需求要求“能看到”，但实现时仍需避免误暴露和误截图。

### 需要改什么

- Codex 账号模型新增结构化备注字段：
  - `two_factor_secret`
  - `account_password`（导入兼容 `password`）
  - `phone_number`
  - `account_note` / `notes` 作为“其他备注”
- 账号新增/编辑/详情 UI 支持录入和展示上述字段。
- 2FA secret 保存后，本地生成当前 TOTP 动态验证码，并展示剩余有效时间。
- OAuth 授权流程中，账号相关区域展示该账号的 2FA secret、当前动态验证码、密码、手机号和其他备注。
- 敏感字段不得写入普通日志、错误日志、usage event、剪贴板历史或远端请求。
- 账号导出/备份如果包含这些字段，必须在 UI 上明确提示包含敏感信息；如支持不导出敏感字段，需要提供选项。
- 如果当前账号文件未加密，新增字段落盘前必须确认是否先做账号详情加密；至少不能在文案中宣称已加密。

### 可能涉及文件

- `crates/cockpit-core/src/models/codex.rs`
- `crates/cockpit-core/src/modules/codex_account.rs`
- `crates/cockpit-codex-adapter/src/main.rs`
- `src/types/codex.ts`
- `src/stores/useCodexAccountStore.ts`
- `src/services/codexService.ts`
- `src/pages/CodexAccountsContent.tsx`
- `src/components/codex/*`
- `src/locales/zh-CN.json`
- `src/locales/en-US.json`
- `src/locales/en.json`

### 怎么改

1. 先设计数据结构和迁移规则：旧 `notes` / `account_note` 迁移到“其他备注”，新增敏感字段默认为空。
2. 增加 TOTP 工具函数：解析 Base32 secret，生成当前验证码，返回剩余秒数；无效 secret 显示字段错误。
3. 保存账号时按 patch 语义更新敏感备注字段，不能重建账号导致 token、provider、标签或额度丢失。
4. 账号详情/编辑弹框增加字段；密码和 2FA secret 支持显示/隐藏、复制；TOTP 动态验证码支持复制和倒计时刷新。
5. OAuth 授权弹框或授权账号选择区域读取同一份账号备注数据，展示 secret、当前验证码、密码、手机号和其他备注。
6. 日志和错误处理增加脱敏：任何包含 `password`、`two_factor_secret`、TOTP code 的 payload 都不能原样打印。
7. 如涉及导入/导出，导出前加敏感信息确认；导入时能恢复这些字段。

### 当前实现状态

- 已在 `CodexAccount` 增加 `two_factor_secret`、`account_password`、`phone_number`，旧 `account_note` 继续作为“其他备注”。
- `accounts.updateNote` 已改为 patch 保存语义，未传字段不会清空原有 token、provider、catalog、标签或额度字段。
- 账号备注弹框已支持 2FA secret、当前 TOTP 动态验证码、密码、手机号和其他备注，支持复制；无效 2FA secret 在字段附近提示并保持弹框打开。
- OAuth 绑定账号列表已显示该账号绑定的 2FA secret、当前动态验证码、密码、手机号和其他备注，并支持复制。
- Codex 导出 JSON 若包含这些敏感备注字段，会在导出弹框提示包含 2FA secret、密码或手机号；Cockpit Tools 导出格式支持切换“包含敏感备注 / 已排除敏感备注”，关闭后导出的 JSON 不包含 `two_factor_secret`、`account_password`、`phone_number`。
- 已补全 18 个 locale 文件 key，并通过 `node scripts/check_locales.cjs`。
- 已补 `update_account_note_patches_sensitive_fields_without_resetting_provider` 单元测试。

### 当前实现边界

- 敏感字段当前随 Codex 账号文件本地落盘；未新增加密能力，也没有在文案中宣称加密。
- 当前实现提供“导出包含敏感信息”的提示，并支持在 Cockpit Tools 导出格式中排除 2FA secret、密码和手机号；`account_note` 作为其他备注继续保留。
- 本次涉及 `src/` 宿主前端和 `src-tauri` command，不是纯 Codex zip 改动，发布需要宿主包。

### 验收

- 旧账号备注升级后显示在“其他备注”里，不丢失。
- 新增或编辑账号可保存 2FA secret、密码、手机号和其他备注。
- 输入有效 2FA secret 后，账号详情能显示 secret 和当前动态验证码，验证码按周期刷新。
- 输入无效 2FA secret 时，字段附近显示错误，弹框不关闭。
- OAuth 授权操作中能看到该账号的 2FA secret、当前动态验证码、密码、手机号和其他备注。
- 敏感字段不出现在 app 日志、platform 日志、usage event 或错误提示里。
- 导出包含敏感字段时有明确确认；如果选择不导出敏感字段，导出结果不包含 secret/password。
- `node scripts/check_locales.cjs` 通过。

### 发布边界

- 仅修改 Codex 账号模型、adapter 保存/读取和 Codex remote UI 时，可发 Codex zip。
- 如果当前 Codex 账号页仍由宿主 `src/` 前端渲染，新增 UI 字段需要宿主包。
- 如果新增通用加密、导入导出协议、Host API 或全局账号字段，需要宿主包，并可能要求先发宿主再发 Codex zip。

## REQ-025 Codex 测试隔离与开发质量

来源：PR #816

### 用户需求

- 测试环境中的 Codex mock 账号不得写入真实用户目录。
- Windows 上也要隔离 `dirs::home_dir()` 解析。

### 需要确认

- app crate 和 `cockpit-core` 是否都有 test-only data dir override。
- 哪些测试仍会读取真实 home、真实 Codex config 或真实账号目录。

### 需要改什么

- 测试统一使用临时数据目录。
- 禁止测试默认落到真实 `dirs::home_dir()`。
- 对危险路径增加断言，发现真实 home 时直接 fail。

### 可能涉及文件

- `crates/cockpit-core/src/modules/account.rs`
- `crates/cockpit-core/src/modules/codex_account.rs`
- `src-tauri/src/*`
- 相关测试 helper

### 怎么改

1. 统一测试环境变量或 test guard。
2. app crate 和 `cockpit-core` 都接入同一隔离逻辑。
3. Windows 路径单独补测试，覆盖 home dir 和 app data。

### 当前实现状态

- `cockpit-core` 的 Codex 账号测试已有 `TEST_ENV_LOCK` 和 `TestEnvGuard`，测试期间会把 `HOME`、`CODEX_HOME`、`COCKPIT_TOOLS_DATA_DIR` 指到临时目录，结束后还原并删除临时目录。
- 本地新增 `test_env_guard_isolates_home_codex_home_and_data_dir`，明确断言测试不会默认落到真实用户目录，并验证 drop 后环境变量还原。
- 现有 app crate / Tauri command 侧是否还有未隔离测试，需要在跑全量 `cargo test --lib` 或新增 app crate 测试时继续检查。

### 验收

- `cargo test --lib` 不会读写真实用户目录。
- 测试失败时能输出实际使用的数据目录。
- Windows 路径隔离测试通过。

### 发布边界

- 测试隔离通常不需要发平台 zip 或宿主包，除非同时修复运行时代码。

## REQ-026 Codex switcher preview

来源：PR #950

### 用户需求

- 评估是否保留 Codex switcher preview 路由和桌面命令，用于安全展示账号切换预览。

### 需要确认

- 当前产品是否还需要 preview 入口。
- preview 是否会加载远程广告、富文本或账号敏感信息。
- preview 是否可能误触发真实切号。

### 两种方案

方案 A：保留。

- DTO 必须脱敏。
- 账号 ID 必须校验。
- 远程广告/富文本必须 sanitization。
- preview 只读，不写落盘，切换必须由明确确认动作触发。

方案 B：删除。

- 删除无入口路由、桌面命令和未使用 DTO。
- 保留必要的安全确认逻辑到正式切号弹框。

### 当前实现状态

- 本地搜索未发现独立的 Codex switcher preview route、desktop command 或专用 DTO。
- 当前存在的 `preview` 主要是批量导入 preview、模型供应商保存 preview、实例页 quota preview 和 2FA 动态码 preview，不属于 PR #950 描述的 switcher preview。
- 因此本轮不删除代码；后续如果重新引入 switcher preview，必须按“只读、脱敏、不写真实 auth/config”的方案 A 约束实现。

### 可能涉及文件

- `src-tauri/src/commands/codex.rs`
- `src/pages/*`
- `src/components/codex/*`
- `src/routes` 或对应路由配置

### 验收

- 保留时：preview 不能写真实 auth/config，敏感字段脱敏。
- 删除时：无死路由、无未使用 command、无前端入口。

### 发布边界

- preview 属于宿主 UI/command，通常需要宿主包。

## 跨需求工程约束

- 所有失败必须在当前弹框或当前操作区展示，不能只显示到弹框外。
- 字段错误显示在字段附近，非字段错误显示在固定错误区。
- 新增 UI 文案必须补 `zh-CN`、`en-US`、`en`，并通过 `node scripts/check_locales.cjs`。
- 账号切换必须按真实落盘结果判定成功，不允许只改前端状态。
- 账号说明文案必须与真实本地读写、加解密、网络请求、上传范围一致。
- 平台 zip 发版不得手动提交 `platform-packages/index.json`、`index.seed.json`、`history/*.json`。
- 单平台/少数平台 zip 发布必须传 `platforms`，例如 `platforms=codex`。
