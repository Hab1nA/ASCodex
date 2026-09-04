---
name: bohrium-bohr
description: "使用玻尔官方 bohr CLI（Bohrium CLI）把重型科学计算任务提交到 Bohrium 云端算力：任务提交/监控/下载、节点、数据集、镜像、机型管理。当本地计算太重（DFT/MD/燃烧/大网格/长时限）或需要 GPU/多核机型时，用 bohr 提交云端作业，结果下载回本地后再走 Playground 提交流程。触发词：'用 bohr 提交'、'提交云算力'、'bohr job'、'重型计算'、'云端跑'、'Bohrium 算力'、'长训接力'。"
version: 1.1.0
author: friday-team
tags: [bohrium, cloud, compute, hpc, long-training]
---

# bohrium-bohr — Bohrium 云端算力提交

bohr（Bohrium CLI）是深势科技（DeepModeling）官方的命令行工具，用于把科学计算作业提交到 Bohrium 云平台执行，支持 CPU/GPU 机型、任务组、数据集与交互式节点。

## 铁律：重型计算必须一律走 bohr（本地只许冒烟测试）

**科研智能体解题时，凡本地单次运行超过 2 分钟、或题目注明需要重型算力（DFT/MD/燃烧/大网格/长时限/GPU 训练）的计算，一律通过本技能提交到 Bohrium 云端执行——禁止在本地硬跑重型计算。**

- **本地只允许**：读题面/数据、写脚本与调试、小规模冒烟测试（单次 <2 分钟）、结果下载后的画图对比与分析。
- **云端负责**：完整重型计算、收敛性扫描、长训练（数分钟以上的一切计算）。
- **判断标准**：拿不准就上云——"本地试一下"若超过冒烟规模（几分钟级），立即转 bohr 提交，不拖沓。
- 违反后果：本地长跑既阻塞代理推进，又浪费本机资源与时间——重型一律云端是硬性操作纪律（见 OPERATIONS_PLAYBOOK §9.6）。

## 本机安装状态（已验证）

- 二进制：`C:\Users\XKZ\.bohrium\bohr.exe`（v1.1.0，官方下载自 `https://dp-public.oss-cn-beijing.aliyuncs.com/bohrctl/1.1.0/bohr-windows.exe`）
- PATH：已加入用户 PATH（新开终端直接可用 `bohr`；当前会话用全路径 `$HOME\.bohrium\bohr.exe`）
- 环境变量已设置：`OPENAPI_HOST=https://openapi.dp.tech`、`TIEFBLUE_HOST=https://tiefblue.dp.tech`
- 验证：`bohr version` → `1.1.0`

## 已验证的资源配置（2026-08-14 冒烟测试）

- **ACCESS_KEY**：已配置（用户环境变量持久化）。注意 pwsh 每次调用是新进程，需先执行
  `$env:ACCESS_KEY=[Environment]::GetEnvironmentVariable('ACCESS_KEY','User')`（OPENAPI_HOST/TIEFBLUE_HOST 同理）
  或用 `~/.bohrium/bohr.ps1` 包装脚本自动加载。
- **默认项目**：`System-created default project`，ID `1185008`（另一个：LLM科研课堂 2025秋 = 1106774）
- **机型参考**：CPU 从 `c2_m2_cpu`（2核2G）到 `c32_m64_cpu`（32核64G）多档可选；
  GPU 机型用 `bohr machine list -c "gpu" --yaml` 查询。**选型原则：按题目需要选，不挑最便宜的**
- **基础镜像**：`registry.dp.tech/dptech/ubuntu:ubuntu24.04-py3.12`（CPU，Python 3.12）、
  `registry.dp.tech/dptech/ubuntu:22.04-py3.10`（CPU）；学科镜像：DeePMD-kit/ABACUS/CP2K/LAMMPS/GROMACS/Quantum Espresso/Uni-Mol/Amber 等（`bohr image list -t "类别名"`）
- **端到端已验证**：提交 → Pending → 运行 → 结果下载 全流程通过（bohrium-kb/tools/bohr-smoke/ 下有冒烟作业模板）
- **防挂起**：默认表格输出是交互式的（会挂住等 Ctrl+C）；脚本里一律用 `--yaml/--json/--csv`

## 前置条件：ACCESS_KEY

bohr 需要 `ACCESS_KEY` 环境变量（Bohrium 平台个人中心生成的 AccessKey，形如长串 token）：
- 生成：登录 https://bohrium.dp.tech → 个人中心 → AccessKey → 创建（**再次创建会使旧 key 失效**）
- 配置（Windows）：仅在当前进程设置 `$env:ACCESS_KEY="..."`；禁止使用 `setx` 持久化凭据，禁止回显 key。

> 若 ACCESS_KEY 缺失，`bohr job list` 会报 `accessKey=` 或 401 —— 这是第一排查项。不要自己猜测 key，提示用户从官网生成。

## 核心工作流：提交重型计算任务

### 1. 准备输入目录与作业配置

把作业所需文件（脚本、输入数据、参数文件）整理到一个输入目录，如 `workdir/bohr_jobs/run1/`。推荐用配置文件方式提交：

```json
// job.json 字段说明（可只用命令行参数，两者混用则命令行覆盖 JSON）
{
  "job_name": "reproduce-chen2011",          // 作业名
  "command": "python reproduce.py > out.log 2>&1",  // 容器内执行的命令
  "log_file": "out.log",                     // 日志文件名（用于 bohr job log）
  "backward_files": ["results.npz", "fig2.png"],  // 需要回传的文件（结果）
  "project_id": 0,                           // 项目 ID（bohr project list 查询）
  "machine_type": "c32_m64_cpu",             // 机型（bohr machine list 查询）
  "image_address": "registry.dp.tech/dptech/deepmd-kit:3.0.0-cuda12.1",  // 镜像（bohr image list 查询）
  "input_directory": "./input",              // 输入目录（相对当前目录）
  "job_group_id": 0,                         // 任务组 ID（bohr job_group create 创建，非必填）
  "result_path": "/personal",                // 结果自动下载路径（Windows 不支持 -r 自动下载，跳过即可）
  "dataset_path": ["/bohr/xxx/v1"],          // 挂载已有数据集（可选）
  "max_reschedule_times": 2,                 // 失败自动重试次数
  "max_run_time": 60,                        // 最大运行时长（分钟）
  "nnode": 1                                 // 计算节点数（并行）
}
```

### 2. 提交

```powershell
# 配置文件方式（推荐）：
bohr job submit -i job.json -p ./input

# 全命令行方式（-- 参数覆盖 JSON）：
bohr job submit --job_name "reproduce-chen2011" `
  --command "python reproduce.py > out.log 2>&1" `
  --log_file "out.log" --backward_files "results.npz,fig2.png" `
  --project_id 0 --machine_type "c32_m64_cpu" `
  --image_address "registry.dp.tech/dptech/deepmd-kit:3.0.0-cuda12.1" `
  --input_directory "./input" --max_run_time 60 --nnode 1
```

- 必填：`-m/--image_address` 与 `-t/--machine_type`；其余可省（有默认值）
- 提交成功后输出 Job ID，记录它

### 3. 监控

```powershell
bohr job list -n 20              # 最近 20 个作业（-r 运行中 / -i 完成 / -f 失败 / -p 排队）
bohr job list --json             # JSON 输出便于解析
bohr job describe -j <job_id> -l # 作业详情
bohr job log -j <job_id> -o ./   # 下载日志到本地
bohr job terminate <job_id>      # 提前结束（正常收尾）
bohr job kill <job_id>           # 强制停止
bohr job delete <job_id>         # 删除
```

### 4. 取回结果

```powershell
bohr job download -j <job_id> -o ./results   # 下载 backward_files 结果到本地
```

> 注意：官方注明 `-r/--result_path` 自动下载**不支持 Windows**；Windows 下用 `bohr job download` 手动拉取。

## 其他常用命令

```powershell
bohr machine list     # 查看可用机型（CPU/GPU，如 c32_m64_cpu、c4_m15_1 * NVIDIA T4）
bohr image list       # 查看可用镜像（deepmd-kit、vasp、lammps、cp2k、gromacs 等）
bohr project list     # 查看项目 ID
bohr job_group create # 创建任务组（多任务归组，注意：与作业生成后的 JobGroupId 不同）
bohr node create      # 创建交互式节点（可 ssh 免密登录调试）
bohr dataset create -l dir1,dir2   # 创建数据集（无大小限制、断点续传）
bohr dataset list     # 数据集列表
bohr update           # 更新 bohr 到最新版
```

## 与 Playground 解题流程的配合（重要）

| 阶段 | 在哪里跑 | 工具 |
|------|---------|------|
| 读题、参数提取、量纲核对 | 本地 | 平台 API / bohrium_client.py |
| **冒烟测试、脚本调试（单次 <2 分钟）** | 本地 | Python |
| **完整重型计算（DFT/MD/燃烧/大网格/长时限/GPU/数分钟以上）** | **Bohrium 云端（一律，禁止本地跑）** | **`bohr job submit`** |
| 结果下载、画图、与论文原图对比 | 本地 | Python + SSIM |
| 提交 attempt 与评分 | Playground | `work/<slug>/submit_bundle.py`（提交门授权下；或 submit-attempt 技能） |

### 提交到 Bohrium 的作业模板要点

1. **自包含**：输入目录要包含全部脚本与依赖安装指令；容器内通常已预装常见科学软件（选对镜像即可），自写 Python 脚本带 `requirements.txt` 并在 command 里 `pip install -r requirements.txt`（如无网络限制）。
2. **结果回传**：把要带回的文件列入 `backward_files`（支持通配），或让脚本把结果写入固定文件名；最后用 `bohr job download -j <id>` 取回。
3. **日志**：command 里重定向日志到 `log_file` 指定的文件，便于 `bohr job log` 排查。
4. **预算政策（用户明确授权）**：用户派任务即代表预算已准备，**不必吝啬算力开销**——
   `max_run_time` 按任务实际需要放宽（DFT/MD/燃烧/大网格等长任务可设数小时），
   机型优先满足精度与时限要求（多核 CPU 或 GPU），**不为省钱牺牲收敛性、网格密度或计算精度**。
   仍保持工程习惯：先小样本验证 command 正确再提交全量（省的是排错时间，不是预算）。
5. **机型选择**：先用 `bohr machine list` 确认可用机型名再填 `machine_type`，不要臆造机型字符串。
6. **ACCESS_KEY 泄露防护**：不要把 key 写进作业目录或提交物；只存环境变量。

## 长训接力纪律（07 实战教训：monitor 死亡空窗 + 接力破壁）

长训练（数小时级）必须防"单 job 一次性跑完"的风险：

1. **ckpt 每 N 步存盘 + backward_files 回传**：训练脚本每 N 步保存 checkpoint 到固定文件名；`backward_files` 列 ckpt+log，job 结束后自动回传，本地保留接力起点。
2. **job 接力**：先跑 30-40k 步验证曲线，再续 50k/100k（07 实证：23228405 止损 → 23228699 importance 长训破壁 0.171@4k）；续跑 job 从上一段 ckpt 加载，不从头训。
3. **monitor 必须可恢复**：ZCode/Codex 用 shell 工具提交 job 后，以轮询脚本与日志文件检查长任务（用户级 bohrium-job 技能自带 `poll_jobs.py`；后台运行可用 `run_in_background` 但状态与日志路径必须落盘可见），会话中断后按日志恢复 monitor。
   - Running → 重启 monitor 继续盯；
   - Finished → 直接 `bohr job download -j <id>`。
4. **kill 语法**：`bohr job kill <id>`（positional，非 `-j`）；**kill 仅额度见底或用户裁决**，不因"时间止损"杀 job（练习轮按额度止损，见 `closure-evidence-standard`）。

## 故障排查

| 现象 | 原因与处理 |
|------|-----------|
| `unsupported protocol scheme ""` / accessKey 为空 | OPENAPI_HOST 或当前进程 ACCESS_KEY 未设置 |
| 401 Unauthorized | ACCESS_KEY 失效/被重新生成 → 官网重新生成 |
| `job submit` 报参数错误 | 机型/镜像名不存在 → `bohr machine list` / `bohr image list` 核对 |
| 作业一直 pending | 排队中或机型资源紧张 → `bohr job describe -j <id> -l` 看状态 |
| 下载为空 | backward_files 文件名与容器内实际输出不一致 → 先 `bohr job log` 确认 |
| 容器内命令找不到 | 镜像不含该软件或 PATH 问题 → 换镜像或在 command 中指定绝对路径 |

## 参考

- 官方文档：https://bohrium-doc.dp.tech/docs/bohrctl/about/ （简介）、`/docs/bohrctl/install/`（安装）、`/docs/bohrctl/job/`（任务命令）
- 平台：https://bohrium.dp.tech （AccessKey 生成、项目/数据集管理）
- 本地调研笔记：`bohrium-kb/docs/bohr-cli-usage.zh.md`
