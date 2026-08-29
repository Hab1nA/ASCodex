# DSH 工作纪律（全局）

> 本文件为 DeepSeek Harness 用户全局指令，由 dsh-agent-instructions 在每次会话的基线上下文中注入。
> 来源：社区已知问题调研（2026-08-15，dsh 0.1.0-rc.6，官方 Discussions + dsh-handbook 第 12 章）。
> **复核记录（2026-08-26，本机核心 @deepseek-ai/dsh 0.1.1-rc.2）**：已逐条核对 #107/#758/#2001/#328/#880/#2034/#589/#313/#2030/#717 十个讨论及 rc.7→0.1.1-rc.2 全部官方 release notes，上述问题**均未在上游修复**（社区修复仅存于个人 fork 分支如 fix/win32-directory-picker、feat/boot-dangling-*、fix/tool-runtime-scheduler-symbol-for，皆未合入官方）。下述守则全部继续有效。

## 一、环境操作纪律（防崩溃、防事故）

1. **工作区路径不使用中文/特殊字符**。DSH 在 Windows 上有 readUtf16 单字节截断 bug（#107 家族 17+ 帖），含中文路径的工作区会创建失败（ENOENT）。空格可用，中文/全角字符不可。
2. **绝不清理 dsh 沙箱临时目录**（#758，社区标 P0）。清理一次 = 服务永久崩溃、不自愈。已触发只能重启进程。
3. **装新工具（winget/yt-dlp/ffmpeg 等）后必须重启 dsh 进程**，不要依赖沙箱进程内 PATH 刷新（Windows PATH 是进程启动时快照，外部修改不广播，#2001）。
4. **装插件/改配置前先确认快照**：dsh-undo-savepoint 已自动为 6 个配置文件做快照；遇到启动失败（plugin tree failed to load，#328/#880）用 `dsh plugin --profile web remove <包>` 移除问题插件，或从 undo 快照回滚。
5. **不要清理 dsh 的 sessions/ 目录**：会话日志是 JSONL(zstd) 持久化，误删不可恢复；管理会话用 dsh-shelf（verify/rescue/export），删除走 Web UI 或 dsh-shelf 的 trash。

## 二、事故处置流程（按优先级）

1. **会话第一条消息 400 或"永久不可恢复"**（#2034 家族）：这是悬挂 tool call 导致的日志损坏。处置：
   - `npx dsh-shelf verify` 找到损坏会话，`npx dsh-shelf rescue <id>` 导出内容（Markdown 抢救）；
   - 有备份则用 dsh-backup 恢复；配置类损坏用 dsh-undo-savepoint 回滚。
2. **dsh web 启动报 EACCES / 端口被占**（#589）：3080 撞上 Hyper-V 保留区间（3070-3169）时换端口（如 13080），或查占用进程。
3. **Web UI 打开但 /api/* 全部 403**（#313/#2030）：这是信任围栏在起作用，不是后端故障。排查顺序：
   - 关闭浏览器扩展（Page Assist、Ollama 的 "Automatic Ollama CORS Fix" 等会改写请求的扩展）；
   - 关闭系统代理/TUN 对 loopback 的拦截；
   - 反代/远程场景装 dsh-trusted-host-proxy-403-fix 插件，且必须正确配置 trustedHosts。
4. **子进程杀不干净**（#717）：Windows 上手动 `taskkill /T /PID <pid>` 清理进程树。
5. **环境体检**：跑 `dsh-doctor`（node/pnpm/PATH/端口 3080/DSH_HOME/profile 完整性/会话日志 zstd 解码，一次查完）。

## 三、版本与升级纪律

1. **锁版本线**：2026-08-26 实测核心已是 dsh 0.1.1-rc.2（dsh-doctor 0.4.1 按 0.1.0 线测试，其 mutating repairs 对新版停用）。rc 阶段 API 可能随时破坏性变更（rc.1→rc.6 已断过一次依赖）。不主动升级核心；必须升级时先看 release 说明。
2. **升级 dsh 核心前**：先验证 26+ 个已装插件（@deepseek-ai/dsh-tool-*、@liustack/modlens、@liustack/modsearch、dsh-* 系列）与新版兼容性，或先备份 profile（package.json + pnpm-lock.yaml + cordis.patch.yml）。
3. **安装新插件前**：优先选 npm 发布的稳定版本；GitHub 源先看 package.json 生命周期脚本（prepare/postinstall 为空更安全）与 peerDeps 是否覆盖 rc.6；装完必须验证 `dsh --profile web --dump-config` 能正常组合。
4. **GitHub 源插件用固定 commit/分支**，不用漂移的默认分支（除非作者明确要求）。

## 四、安全纪律

1. **插件是第三方代码**：以你的权限运行，可读文件、用凭据、联网。不装来源不明/无人维护的插件；装前审源码。
2. **dsh-backup 的备份归档含明文凭据**（.credentials.yaml 等）：只存本地，绝不同步到公开/不受信位置，不配置 githubRepo 指向非私有仓库。
3. **不把 dsh web 暴露到公网**：CLI 禁止 --host 0.0.0.0 是有意的安全设计（远程代码执行风险）。远程访问用 SSH 隧道（ssh -L 3080:127.0.0.1:3080），浏览器看到的仍是 localhost，设置才能落盘。
4. **凭据文件**（~/.dsh/.credentials.yaml、agent*_credentials.txt）不外传、不打进任何交付物。

## 五、性能与上下文纪律

1. **优先用确定性工具**：dsh-tool-*（calculator/csv/json/regex/markdown/diff/stat/time/schema/encoding）是零依赖本地工具，优先于 shell 拼命令。
2. **工具输出大时用 dsh-funnel 语义**：保留错误/警告行 + 头尾，全文落盘可回读，避免撑爆上下文。
3. **web_search 走 modsearch**（已接管 searchProvider），失败时检查 ~/.modsearch/config.json 引擎配置，勿退回裸 curl 抓整页。
4. **图片读取用 modlens**（modlens_read_image），不要用 shell 转 base64 塞进上下文。

## 六、会话与消息纪律

1. **长时间任务**：dsh-client-auto-continue 会在网络/崩溃后自动继续；不手动重复发送相同请求。
2. **ask_user_question 有 300s 超时保护**（dsh-ask-guard），问题会以 ASK_TIMEOUT 返回，不要在同一问题上反复死等。
3. **跨会话消息**用 dsh-agent-message 的 send_agent_message；目标离线会自动留言。
