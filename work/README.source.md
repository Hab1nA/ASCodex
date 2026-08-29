# work/ —— 当前题目工作区

> 目录名沿用平台 slug 或历史短名，**为保持引用稳定不重命名**。
> **当前无活跃题**：`work/` 仅保留 `_template` 与本 README。S4 Round-4 三个作战区已整目录搬到 `../archive/challenges/`。
> 已完赛 / 遗留 / 只读协作区对照表见 `../archive/README.md`。
> 提交产物仍放各题自己的 `outputs/`。

## 当前活跃作战区

`work/` 现在有：

| 短名 | 目录 | 说明 |
|---|---|---|
| _template | `_template` | 新题工作区模板 |

新题放入本目录后再更新本表。完赛后：整目录 `Move-Item` 到 `../archive/challenges/<原名>`，并更新本表与 `../archive/README.md`。

## 每题标准结构（约定）

```
<题目目录>/
├── outputs/        # 提交产物（final/answer.json、evidence/、DERIVATION.md 等）
├── trace/          # trace.jsonl + 生成器
├── variants/       # 实验变体（每变体一个子目录，含 scorecard 记录）
├── src/            # 复现代码
├── REPORT.md       # 事实+结论（含 attempt id 证据）
└── EXPLORATION*.md # 探索与已证伪方向
```

## 已迁出（不要在 work/ 重建同名目录）

- S4 Round-4 三个作战区（friday / ox-alpha / jarvis）→ `../archive/challenges/`
- S4 Round-3 十题与遗留练习 → `../archive/challenges/`
- 2026-08-21 结束处理的九题（two-phase-gold 等） → `../archive/challenges/`
- jarvis / ultron 协作区 → `../archive/collab/`（只读）
- 原 `work/` 根上的 challenges JSON/扫描脚本 → `../bohrium-kb/tools/work-index/`
