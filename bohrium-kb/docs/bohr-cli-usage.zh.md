# bohr CLI（Bohrium CLI）调研与使用手册

> 调研日期：2026-08-14 | 状态：已安装并验证（v1.1.0），待配置 ACCESS_KEY 即可提交云端任务
> 官方文档：https://bohrium-doc.dp.tech/docs/bohrctl/about/ | 平台：https://bohrium.dp.tech

## 1. 结论速览

- **bohr = Bohrium CLI**，深势科技官方命令行工具，用于把科学计算作业提交到 Bohrium 云端算力（任务/节点/数据集/镜像/机型/项目/任务组）。
- 本地已安装：`C:\Users\XKZ\.bohrium\bohr.exe`（v1.1.0），已加入用户 PATH，`OPENAPI_HOST`/`TIEFBLUE_HOST` 已设置。
- **唯一缺口：ACCESS_KEY**。bohr 需要 Bohrium 平台 AccessKey（https://bohrium.dp.tech/settings/user 生成），未配置时所有 API 报 `AccessKey Invalid!`。
- Playground 的 `playground` CLI（@paper2arm/playground-cli）负责**提交 attempt/评分**；bohr 负责**云算力执行**。两者互补。

## 2. 安装（Windows）

```powershell
# 官方安装脚本（等价手动步骤）：
# 1. 下载二进制
curl.exe -fsSL -o "$HOME\.bohrium\bohr.exe" "https://dp-public.oss-cn-beijing.aliyuncs.com/bohrctl/1.1.0/bohr-windows.exe"
# 2. 设置环境变量（持久化）
setx OPENAPI_HOST "https://openapi.dp.tech"
setx TIEFBLUE_HOST "https://tiefblue.dp.tech"
# 3. 加入用户 PATH（%USERPROFILE%\.bohrium），重开终端生效
```

## 3. 配置 ACCESS_KEY（已完成 ✅）

1. 登录 https://bohrium.dp.tech → 个人中心/设置 → AccessKey → 创建（重新创建会使旧 key 失效）
2. Windows 持久化：`setx ACCESS_KEY <key>` 后重开终端；当前会话 `$env:ACCESS_KEY="<key>"`（或从注册表加载：`$env:ACCESS_KEY=[Environment]::GetEnvironmentVariable('ACCESS_KEY','User')`，pwsh 新进程不继承 setx 值）
3. 验证：`bohr job list -n 5` → 无报错即认证通过
4. **账号体系**：Bohrium 云平台与 Playground 同账号（userid 1179613 / 谢铠舟）

### 非交互/防挂起技巧

- `bohr machine list --yaml`、`bohr project list --csv`、`bohr job describe --yaml` 等**非交互输出**不挂起；
  默认表格模式（如 `bohr machine list`、`bohr image list -t "Basic Image"`）是交互式表格，会挂住等待 Ctrl+C —— 脚本里务必用 `--yaml/--json/--csv`。

## 4. 重型计算任务提交流程

### 冒烟测试实录（2026-08-14，已通过 ✅）

作业 `bohr-smoke-test`（JobId 23200340）：`registry.dp.tech/dptech/ubuntu:ubuntu24.04-py3.12` + `c2_m2_cpu`，
命令 `python run.py > out.log 2>&1`，backward_files `[out.log, result.txt]`。
提交 20:12:55 → Finished 20:13:46（约 51 秒含排队），exitcode 0，费用 ~0。
云端环境：Python 3.12.4 / numpy 2.3.2 / Linux x86_64；结果经 `bohr job download -j <id> -o <dir>` 取回为 zip。
模板位于 `bohrium-kb/tools/bohr-smoke/`（input/run.py + job.json）。

### 4.1 准备 job.json（推荐方式）

```json
{
  "job_name": "reproduce-chen2011",
  "command": "python reproduce.py > out.log 2>&1",
  "log_file": "out.log",
  "backward_files": ["results.npz", "fig2.png"],
  "project_id": 0,
  "machine_type": "c32_m64_cpu",
  "image_address": "registry.dp.tech/dptech/deepmd-kit:3.0.0-cuda12.1",
  "input_directory": "./input",
  "max_reschedule_times": 2,
  "max_run_time": 60,
  "nnode": 1
}
```

必填：`machine_type`（-t）、`image_address`（-m）。命令行 `--` 参数覆盖 JSON。

### 4.2 提交与监控

```powershell
bohr job submit -i job.json -p ./input     # 提交（记录 Job ID）
bohr job list -n 20                        # 状态（-r 运行/-i 完成/-f 失败/-p 排队/--json）
bohr job describe -j <id> -l               # 详情
bohr job log -j <id> -o ./                 # 日志
bohr job download -j <id> -o ./results     # 结果（Windows 下 -r 自动下载不支持，用手动 download）
bohr job terminate <id> | kill <id> | delete <id>
```

### 4.3 其他

```powershell
bohr machine list | image list | project list   # 查机型/镜像/项目 ID（勿臆造机型名）
bohr job_group create                           # 任务组（与作业生成后的 JobGroupId 不同）
bohr node create                                # 交互式节点（免密 ssh）
bohr dataset create -l dir1,dir2                # 数据集（无大小限制、断点续传）
bohr update                                     # 自更新
```

> **节点恢复（CLI 无 start 命令，2026-08-15 实测）**：`bohr node stop` 把节点置为 Paused 后，
> CLI（v1.1.0）**没有** start/resume 命令（二进制内仅有 create/stop/delete/list/connect）。
> 恢复需直接调 OpenAPI（Bearer 认证，accessKey 走 query 参数）：
> ```powershell
> # 端点模式：POST https://openapi.dp.tech/openapi/v1/node/restart/{nodeId}?accessKey=<key>
> # 实测 POST /openapi/v1/node/restart/1434795 → {"code":0}，节点从 Paused 回到 Pending（重启中）
> # 探测记录：node/start、node/resume、node/run、node/recover 均为 404，仅 node/restart 有效
> # 其他已知端点：GET /openapi/v1/node/list、POST /openapi/v1/node/stop/{id}（均由 CLI 调用）
> # 调试技巧：$env:GODEBUG='http2debug=2' 运行 bohr 可打印实际请求的 :method/:path
> ```

## 5. 与 Playground 解题流程的配合

| 阶段 | 位置 | 工具 |
|------|------|------|
| 读题/参数提取/冒烟测试 | 本地 | API / bohrium_client.py |
| 完整重型计算（DFT/MD/燃烧/大网格/GPU/长时限） | **Bohrium 云端** | **bohr job submit** |
| 画图/SSIM 对比 | 本地 | Python |
| attempt 提交/评分 | Playground | playground submit / submit-attempt 技能 |

**作业自包含原则**：输入目录含全部脚本；选对镜像（容器预装常见软件）；结果列 `backward_files`；command 重定向日志到 `log_file`；先小样验证再全量；ACCESS_KEY 不进作业目录。

**预算政策（用户明确授权，2026-08-14）**：用户派任务即代表预算已准备，不必吝啬算力开销。机型按题目需要选（多核 CPU / GPU，不挑最便宜的），`max_run_time` 按任务实际需要放宽（长任务可设数小时），不为省钱牺牲收敛性、网格密度或计算精度。小样本冒烟验证仅为节省排错时间。

## 6. 故障排查

| 现象 | 处理 |
|------|------|
| `AccessKey Invalid! Visit https://bohrium.dp.tech/settings/user` | 未配置/失效 key → 官网生成后 setx |
| `unsupported protocol scheme ""` | OPENAPI_HOST 未设置 |
| 401 | key 被重新生成 |
| submit 报参数错误 | 机型/镜像名不对 → machine list / image list |
| 下载为空 | backward_files 与容器实际输出不一致 → 先看日志 |

## 7. 相关技能（已安装到 DSH）

- `bohrium-bohr`（~/.dsh/skills/bohrium-bohr/SKILL.md）—— 本手册的技能版
- `submit-attempt`（~/.dsh/skills/submit-attempt/SKILL.md）—— 平台官方 ARM 提交技能
- Playground CLI：`playground`（@paper2arm/playground-cli 0.1.26，已认证 play.bohrium.com）
