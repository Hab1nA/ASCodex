# ASCLocal — 工作区约定（智能体必读）

目录已于 2026-08-21 重组，2026-08-22 将 S4 Round-4 作战区整目录归档。**不重命名题目 slug**。新文件禁止散落根目录。

## 落盘地图

| 用途 | 路径 |
|---|---|
| 当前活跃题 | `work/` 仅保留 `_template` 与 `work/README.md`；S4 Round-4 作战区已迁到 `archive/challenges/{friday,ox-alpha,jarvis}-round4/` |
| 完赛 / 遗留题 | `archive/challenges/<原 slug>/` |
| jarvis / ultron 协作区 | `archive/collab/`（**只读，禁止写入**） |
| 历史桶 | `archive/historical/`（race、deepham_research 等） |
| 知识库 / 工具 | `bohrium-kb/`（`docs/` `data/` `tools/` `creds/` `round3_prep/`） |
| 提交 zip | `_artifacts/` |
| 运行日志 | `_logs/`（编号作业日志在 `_logs/job-logs/`） |
| 调研 | `_research/` |
| 一次性转储 | `scratch/` |
| 路径对照 | `archive/README.md` |

## 规则

1. **新题** 放 `work/`；完赛后整目录搬到 `archive/challenges/<原名>`，更新 `work/README.md` 与 `archive/README.md`。旧 Round-4 作战区已归档，解题写入不得越界到 `archive/collab/`。
2. **选题防撞车** 必须同时扫 `work/` **和** `archive/challenges/` + `archive/collab/`（不能只扫 `work/`）。
3. **旧路径** `work/ultrasound`、`work/ppt-4x4`、`work/jarvis-*`、`work/friday-round4` 等已失效；回看用 `archive/` 对照表。
4. 凭据只放 `bohrium-kb/creds/` 或用户主目录；禁止写入仓库可见根目录。
5. 作战手册仍是 `bohrium-kb/round3_prep/OPERATIONS_PLAYBOOK.md`。
