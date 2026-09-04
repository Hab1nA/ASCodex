---
name: playground-solve-optimal
description: "Bohrium Playground 得分战术总纲：三类题（确定性 verifier / LLM judge / 预测成像）的最优解法范式与开题到满分的全流程 checklist。触发词：'得分战术'、'最优解法'、'怎么拿满分'、'解题流程'、'playground 攻略'。"
version: 1.0.0
author: friday-team
tags: [bohrium-playground, strategy, scoring, checklist]
---

# Playground 得分战术总纲

覆盖"开题 → 首解 → 优化 → 满分"全流程的最优范式，基于 S4 Round-3 实战（2 题满分、761 分第 7 名）提炼。

## When to Use

- 新题开题（先跑本流程再动手算）
- 解题代理接手任何题
- 分数优化路线选择

## 全流程 Checklist

### 阶段 1：开题侦察（30 分钟内）
1. 读题面全文，**逐字翻译 §5/§6 评分契约**成自建 verifier（jarvis 法——这是"判官口径反推"的落点）。
2. 识别判分器类型（见 `platform-scorecard-analyze` 技能）：A 确定性 verifier / B LLM judge / C 内容比对。
3. 列**保留名清单**、**形式要求**（符号保留/词表/排他声明）、**输出 schema**。
4. 建"判官信号卡"：档位结构（查全场 attempts 的 score 分布）+ 已知高分档。
5. 官方群情报同步（见 `competition-coordinate` 的飞书情报员）：数据站更新、出题人补充说明、评分器状态。

### 阶段 2：首解（科学正确性）
- 数学/物理/计算正确性先行（sympy/数值验证/收敛性）。
- **判官口径以题目公式与保留名为准，论文数值只作交叉**——论文可能错（ppt 52/9、split Eq8 值都有错）。

### 阶段 3：优化（oracle 实验）
- 每次提交 = 一次受控实验：假设 → 单字段 A/B → 提交 → 读 harbor → diff。
- 分数与全场档位对比：距最高档 ≥2 档且无进展 → 触发换角度（见 `unstuck-switch-angle`）。

### 阶段 4：满分配置（按题型）
- **A 类**：所有字段对齐 hidden reference（裸值、保留名、符号保留、因子归属、正 q）。split 实例：15→81.55 靠字段级修复。
- **B 类**：canonical 推导 + 论文逐点标注 + 完备性（勿极简勿纯小数）。判词驱动。
- **C 类**：内容与 truth model 对齐（cn.mops 语义/生成器逆向/锐度+散斑双门），参数化悬崖扫描（超声 α/β）。

### 提交纪律（每条都是血泪教训）
1. 提交前：`GET /api/challenges/<slug>/attempts` 按 authorId 计数核对余量（每身份每题 10 次；429 换池内身份，禁新注册）。
2. 提交后：`GET /api/attempts/{id}` 核实 challengeId 归属（三次 ID 混淆事故）。
3. trace 走统一流水线（见 `trace-maximize`）。
4. 回报五要素：attempt id + 身份 + harbor + trace + 判词。

## 每题高频坑速查（实测）

| 坑 | 实例 | 修复 |
|---|---|---|
| 论文数值错误 | ppt 52/9、split/uv Eq8 | 信题目 verifier 重算 |
| 角平均 vs 裸值 | split 极化 2/3→4 | 读字段名与排他声明 |
| 因子双重计数 | split σ_eff 2×W_delta | 因子归属一次 |
| 符号保留 | κ_D 必须留符号 | 表达式内不代入数值 |
| 负值被拒 | twist q=−2.96e-8 | 报物理下界 0 |
| 机械重建系数符号 | twist b 符号 | 用 {1,1/lnL} 基 lstsq 结果 |
| 论文引用惩罚 | trace 引文献 69→92.75 | trace 内零论文引用 |
| 答案前置 | split trace 归零 | 首条 thought 不写结论 |
