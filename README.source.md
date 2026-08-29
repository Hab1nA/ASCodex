# ASCLocal —— Bohrium Agentic Science 竞赛工作区

S4 Round-3 参赛工作区（2026-08-14 20:00 ~ 08-15 20:00 UTC+8，最终 761.42 分第 7/54 名）。
S4 Round-4 三个作战区已整目录归档。**当前无活跃题**（`work/` 仅保留 `_template`），已完赛/遗留题在 `archive/`。

## 目录结构

```
ASCLocal/
├── README.md                 # 本文件：架构总览
├── .gitignore
├── skills-lock.json
├── work/                     # 当前无活跃题：仅 _template（见 work/README.md）
│   ├── README.md
│   └── _template/
├── archive/                  # 已完赛 / 遗留 / 只读协作 / 历史桶
│   ├── README.md             # 搬迁对照表（旧 work/<slug> 路径）
│   ├── challenges/           # 完赛与遗留题（含 S4 Round-4 三个作战区），目录名未改
│   ├── collab/               # jarvis / ultron 只读协作区
│   ├── historical/           # race、deepham_research 等根目录旧桶
│   └── misc/
├── bohrium-kb/               # 知识库：平台文档、数据、工具、复盘
│   ├── docs/
│   ├── data/                 # 公开数据（含 muon-public-data）
│   ├── tools/                # API 工具；work-index/ 为原 work/ 根散落索引
│   ├── creds/                # 凭据（勿提交）
│   ├── round3_prep/          # Round-3 备战与复盘
│   └── study/
├── _artifacts/               # 提交产物 zip（ARM bundle）
├── _logs/                    # 运行日志；job-logs/ 为编号作业日志
├── _research/                # 调研草稿 / 论文摘录
└── scratch/                  # 一次性转储（终端输出、来源不明的 outputs）
```

工具隐藏目录（`.git` `.agents` `.dsh` `.claude` `.wenyon`）留在根上，不要往里面塞题目文件。

## 常用入口

- **智能体目录约定**：`AGENTS.md`
- **作战手册**：`bohrium-kb/round3_prep/OPERATIONS_PLAYBOOK.md`
- **复盘四件套**：`bohrium-kb/round3_prep/{HARBOR_LAW,TRACE_LAW,JARVIS_METHOD,LESSONS_24H}.md`
- **身份池**：`bohrium-kb/round3_prep/IDENTITY_POOL.md`
- **工具**：`bohrium-kb/tools/`（poll/scorecards/提交/榜单）
- **旧题路径**：`archive/README.md`

## 约定

- `work/` **只放当前还在打的题**；当前无活跃题，仅保留 `_template`。完赛或不再跟的整目录搬到 `archive/challenges/`，**不重命名 slug**。
- `archive/collab/` 下 `jarvis-*` / `*-jarvis` / `ultron-*` **只读，禁止写入**。
- 新文件先归入对应桶，禁止在根目录散落。
- 提交 zip → `_artifacts/`；运行日志 → `_logs/`；一次性垃圾 → `scratch/`；文献/抓取 → `_research/`。
- 凭据只放 `bohrium-kb/creds/` 或用户主目录，已在 `.gitignore`。
