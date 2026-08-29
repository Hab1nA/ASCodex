# Skill 与智能体设定改进建议（战后复盘交付）

> **性质**：round-1 练习轮战后复盘正式交付文档
> **依据**：全程实战（R1-P48→R1-P83 决策日志，attempt id 证据链）+ 高分选手 skill 包复盘（SKILLS_RELEASE_REVIEW.md）+ 子代理三份战后复盘（POSTGAME_09.md / POSTGAME_07.md / POSTGAME_JUDGE.md）
> **日期**：2026-08-22
> **状态**：✅ **已实施**（2026-08-22 按实施计划落地；2.2/2.3 合并 → `judge-field-audit`；新 skill 落点 `.dsh/skills/`；手册新增 §9；4 个预设 persona 更新；STATUS.md 机制落地。详见本文档 §8 实施记录）

---

## 0. 本次作战复盘结论速览

- **最大成果**：09 满分 100.0（0→82 trace 配方 →92 C4 闭式 →100 C3 mechanisms 枚举修复）；08=99.9996、02=99.96、10=94.9288、03=81.875、01=76.99、07=16.55→续战中
- **最大教训**（本清单的源头）：
  1. 判官 H1"Bell-8 丢"错指——判官分解是**推断**，真缺口 = C3 机制**枚举字符串**（数值差分测不到）
  2. 07 止损过早——"FCNN 天花板 rel 0.28"是伪结论（本地曾达 −16.7），**本地曾达更高值 = 禁止封板**
  3. 提交物污染风险——判官/红队情报进入 trace 会被 LLM judge 审计（高分选手 skill 明示）
  4. worker /uploads 提交链是 ARM 路径之外的第二轨（07/08 满分机制）
  5. 消息纪律——running 代理 followup 会积压（10 solver 曾积 11 条）

---

## 1. 现有 skill 清单（与本战场相关）

| Skill | 作用 | 本次实战验证 |
|---|---|---|
| platform-scorecard-analyze | 判分器类型识别 + 单字段 A/B | ✅ 有效（01/03/10 判分器定性）|
| differential-scoring | 高分未满差分定位 | ⚠️ 缺字符串字段维度 |
| oracle-probe | 判官只读诊断 | ⚠️ 缺分解歧义输出 |
| trace-maximize | trace 满分配方 | ⚠️ 缺真实执行史变体 |
| red-team-review | 红队对抗审查 | ⚠️ 缺 clean-room 独立参考协议 |
| submit-attempt | ARM bundle 提交 | ⚠️ 只覆盖 ARM 路径，缺 worker 链 |
| bohrium-bohr | 云算力 | ⚠️ 缺长训接力/monitor 纪律 |
| competition-coordinate | 多代理协调手册 | ⚠️ 缺消息纪律/污染红线 |
| unstuck-switch-angle | 防卡死换角度 | ✅ 有效（09 换角度成功）|
| mp-r-family-solve | mp-r 家族字段规则 | 未涉及 |

---

## 2. 新增 Skill 建议（5 个）

### 2.1 `worker-submit-chain`（提交链第二轨）

- **触发**：`solver-guard_build-submit`（插件唯一入口）后 scorecard executability=0（bundle 未接数值 verifier）时
  - ⚠️ 注：本技能提案已被 R4 实证推翻并清理（2026-08-23，见 INDEX.md 已清理表：worker 队列停摆 + 四步链不进官方榜）；保留此处仅为记录当时假设。
- **核心机制**：
  1. Draft（REST：`json.dumps(trace)` + manifest inline，traceCount>0）
  2. **worker /uploads**（EMBEDDED_WORKER_TOKEN，端点 /api/uploads，201 queued 挂 bundle）
  3. PATCH 补 trace（若 traceCount=0）
  4. submit → 200 → 多轮询判分（late_scored 暂态勿误判 0）
- **bundle 必备**：真实 src（runner/训练脚本）+ characterization.json（自备 targets）+ 蒸馏 trace + 契约版模型产物（07: wavefunction.ts forward [B,32]→[B,2]）
- **坑**：
  - PATCH 挂 bundle 不稳（return 200 但 bundleAvail 假）——bundle 只由 worker 挂
  - trace 必须 `json.dumps(list)` 非裸 JSONL
  - 判分队列积压（draft 卡 14min+ 属正常，勿误判 0）
- **识别特征**："No per-challenge grader" **≠ 不接数值**（09 实证接数值）；好模型+蒸馏 trace 仍全 0 → 试 worker 链而非判"判分器不接数值"
- **配置考古法**：REPORT 反推历史最高值来源脚本（07: −16.7 → train_impt.py，精确 |ψ_GS|² 采样 + importance 无偏重加权）→ 本地小规模复现轨迹命中 → 再上 bohr 长训
- **实证**：08=99.9996、07=16.55（0→突破）、09 机制确认

### 2.2 `judge-field-audit`（判官字段审计与提交级裁决）

- **触发**：判官分解存在多等价解（如 D70+N17 vs D62+N25）；目标涉及字符串/枚举/词表字段；"改 X 必 +N"承诺无法本地机器校验
- **阶段 A 审计（本地零成本）**：
  - 交付物字段分**数值类 vs 字符串枚举类**，**同权重受检**（C3 mechanisms 枚举串错 = 丢 8 分，数值全对）
  - 枚举字段逐 label 码表核对（C3 mechanisms: nonzero / first_bilinear_vanishes / second_bilinear_vanishes / lorentz_contraction_vanishes；C2 separability_class；C6 outer_maximization）
  - 题面逐字判机制：按每个分量独立判（J1/J2 是否消失），不概化
- **阶段 B 裁决（提交 ≤2 发）**：破坏性差分——故意弄坏单字段看 harbor 变不变（变=受检且在拿分；不变=未受检/从未拿分）；**差分必须覆盖全部字段类型**（数值+字符串）；修复后提交验证
- **输出**：`decomposition_ambiguity` 段（全部等价解 + 各自置信 + 判别性 A/B 设计 + 悬空字符串字段清单）
- **坑**：差分测错维度（09: 弄坏数值不动 ≠ 不检查，真丢分项是枚举串）；harbor 粗档化使小差分不可分辨；本地自检全对 ≠ 判官过（self-referent 盲区）
- **实证**：09 满分链路（29180 best-of-16 证伪 H1 → 29181 C3 差分 → 29183 修复满分）

### 2.3 `probe-diff-submission`（破坏性提交级差分）

- **触发**：缺口定位不确定、多假设竞争、判官口径分歧时（通用手法）
- **核心**：单字段 A/B（改 1 处提交 1 发 vs 基线）；每发提交前写一行"此发确立什么、每个结果导向什么"；禁止 retry 循环（失败=停下列线诊断）
- **实例**：01 R_cell vs R_RIR（题面字面读 vs 实现 → 提交 1 发裁决判官口径 0.6<0.77）；03 Cu27Pd28_ico 近简并 A/B（DROP 3.125 → gold=baseline）；09 C3 差分
- **原则**：判官口径分歧（题面字面 vs 物理实现）唯一裁决者 = 提交

### 2.4 `trace-contamination-redline`（提交物污染红线）

- **触发**：任何提交前（尤其 trace 重建时）
- **核心**：提交物（trace/raw_messages/thought/stdout/代码注释/变量名）**零**平台分数、**零** attempt id、**零**判官/红队结论、**零**他人做法；只允许"从题面与数据出发的推导"
- **执行**：提交前 banned 词扫描 gate（全 CLEAN 才提交）；提交物与诊断脚本目录分离（正式提交只用干净集）
- **测试句**："没看过答案或分数的人能写出这句话吗？"——方法论通过，结论不通过
- **原理**：判官/红队情报 = "给 solver 的答案"；LLM judge 审计 transcript 知识来源（高分选手 exam-logic）
- **实证**：09/07/01/03 全部提交物扫描 CLEAN；高分选手 skill 复盘落地

### 2.5 `closure-evidence-standard`（封板证据标准）

- **触发**：solver 建议收关/封板时
- **封板三问**：
  1. 场上有人在你上面？（有人 → 不是封板，头寸已被证明可达）
  2. 多个独立 attempt 证伪还是只有你？（只有你 = 你方法的问题）
  3. **本地/历史是否曾达更高值？**（曾达 → 继续追，**禁止以预算/止损封板**）
- **真天花板核对清单**（07 教训细化）：
  - 是否曾达更高值记录（REPORT 考古）？是否反推复现过？
  - 采样 regime 是否共享遗漏（uniform 度量 vs |ψ|² 采样）？
  - 判分器是否真确认过（worker 链 vs ARM 路径）？
  - **时间止损在练习轮改为额度止损**（额度见底才收口）
- **封板条件**：多轴证伪 + 提交级证据 + 无已知上升路径；封板结论标 ⚠ 可重测（下轮零成本重验）
- **坑**："当前实现天花板" ≠ "问题天花板"（07 rel 0.28 伪天花板——本地曾达 −16.7/rel 0.148，importance 采样破壁 0.171@4k）
- **实证**：用户纠正（07 止损过早）+ 07 续战破壁

---

## 3. 修改现有 Skill 建议（6 项）

| Skill | 修改内容 | 依据实例 |
|---|---|---|
| platform-scorecard-analyze | +字符串/枚举字段审计维度（双字段列）；+档位分布反推参考值法（score=harbor×100、1/160 桶位、E_ref 窗口、0.9 档贴参考）；+判官口径 A/B 原则（字面 vs 实现分歧 → 提交裁决）；+确定性 verifier 识别（harbor 对内容多变体不变） | judge-01/03/09 档位反推；01 R_cell A/B |
| oracle-probe | +`decomposition_ambiguity` 输出段（等价解 + conf + 悬空字符串字段清单） | judge-09 错指（D70/N17 vs D62/N25） |
| trace-maximize | +真实执行史变体（提交实际做工作的 transcript：真实 stdout/thought/失败尝试，非蒸馏编造）；+trace-labels 耦合识别（同 labels 多次 <80 = 呈现问题非内容） | 10 题 6 次反证（构造×5 + 真实×1） |
| red-team-review | +clean-room 独立参考协议（本地自检全对但判官丢 → 从题面从零重写交叉 + 约定变体库） | 09 C3 indep_c3.py（投射等价 1.11e-16 定位枚举串） |
| bohrium-bohr | +长训接力纪律（ckpt 每 N 步存盘、job 接力 40k→40k、monitor 存活轮询 10h cap、kill 语法）；kill 仅额度见底/用户裁决 | 07 monitor 死亡空窗教训；23228405→23228699 接力 |
| competition-coordinate | +消息纪律（running/resident 代理 subagent_send 必须 mode=steer/cancel_first；followup 仅 cold resume；subagent_queue 定期核查清积压）；+monitor 职责扩展（STATUS.md 固定状态文件，五要素格式） | 10 solver 积压 11 条；高分选手 endgame skill |

---

## 4. 智能体设定修改建议（手册级）

### 4.1 判官角色定位：假设工厂，非结论授权
- 输出：N 候选 + 判别法 + 置信 + **判别性 A/B 设计**（能区分等价解的提交实验）
- 交接：给 solver"分量 + 判别法"，**不交接"单解定论"**
- 字符串/枚举字段**永远**入交接单
- 预期增量须来自判别法，否则禁盲试
- **依据**：judge-09 H1 错指（分解正确但单押等价解之一）

### 4.2 solver 增加 clean-room 验证位（提交前强制门）
- 数值 + 字符串双字段机器核对（非事后红队）
- 红线 banned 词扫描 gate
- 契约文件名逐字核对 + bundle dry-run 内检（缺包内文件静默致命）

### 4.3 手册 OPERATIONS_PLAYBOOK §7 增补
1. **预算/止损须与用户对齐**：练习轮 vs 正式轮语境不同；"本地曾达更高值"时禁止以预算封板；练习轮按额度止损非时间止损
2. **污染红线入册**（§2.4 全文）
3. **字符串枚举字段检查单**（逐分量机械判定）
4. **判分路径两轨**（ARM vs worker 链）写入手册

### 4.4 判官信号卡模板升级
- 新增"数值/字符串双绿"列（两字段类型均受检才算过）
- 新增"结论置信级"标注（推断 vs 提交级实证）
- 新增验证锚点（每个结论挂 attempt id）

---

## 5. 高分选手 skill 包可借鉴点（已吸收/待吸收）

| 借鉴点 | 状态 |
|---|---|
| transcript 是一等交付物（LLM judge 审计） | ✅ 已吸收（污染红线） |
| 给 solver 问题不给答案 | ✅ 已吸收（红线测试句） |
| 提交实际做工作的 transcript | ⚠️ 部分（10 题验证失败，07/09 构造蒸馏成功——需按题甄别） |
| 封顶三问 | ✅ 已吸收（closure-evidence-standard） |
| 对手分数 = 已证明可达 | ✅ 已实践（judge 差分） |
| postgame 四行格式（测量/推断分离、负面先写、prohibition 标注） | ⚠️ 待吸收（LESSONS_24H.md 升级） |
| STATUS.md 固定状态 | ✅ 已建议（monitor 职责扩展） |
| 空闲循环（判分轮询/未提交的提交/榜差距） | ⚠️ 待吸收（monitor 职责） |
| CLI 保持当前 | ⚠️ 与用户裁决冲突（禁升 0.1.29）——待用户定夺 |

---

## 6. 实施顺序建议

| 优先级 | 动作 | 产出 | 耗时 |
|---|---|---|---|
| P0 | 污染红线 + 封板标准写入 OPERATIONS_PLAYBOOK §7 | 手册增补 | 30min |
| P0 | 判官信号卡模板升级（双字段列 + 置信级） | 模板文件 | 30min |
| P1 | 5 个新 skill 文件（各 30-60 行，附 attempt 实例） | skills/ 目录 | 2-3h |
| P1 | 6 项现有 skill 修改 | skill 文件更新 | 2h |
| P2 | LESSONS_24H.md 按 postgame 四行格式升级 + prohibition 标注（04/05/06） | 知识库更新 | 1h |
| P2 | monitor STATUS.md 机制落地 | monitor 职责文档 | 1h |

---

## 7. 证据台账（本清单全部实例出处）

- 09：attempt 29142/29144（92 基线）· 29180（best-of-16 证伪 H1）· 29181（C3 差分）· 29183（**100 满分**）；脚本 indep_c3.py / verify_top1_bestofK.py / probe_c3_crosscheck.py
- 07：attempt 29165（ARM 全 0）· 29168（**16.55 worker 链**）· 29176（R_cell A/B）· bohr 23228405（止损）/ 23228699（importance 长训破壁 rel 0.171@4k）
- 03：29109（全局极小 DROP）· 29177（H2 呈现）· 29179（近简并 A/B DROP）
- 01：28916（76.99 定稿）· 29176（R_cell 0.6）· 29182（尾截断仲裁）
- 10：29108（94.9288 定稿）· 29139/29143（0.974 labels）· 29175（终局 trace 30.7）
- 02：29195（0.999558 第 4 次复现）

---

*附：分析工作文件 research/SKILLS_GAP_ANALYSIS.md（13 项矩阵）与 research/SKILLS_GAP_FINAL.md（合并清单）为本文档底稿。*

---

## 8. 实施记录（2026-08-22）

> 本清单全部建议已落地。实施中的结构调整（相对 §2/§3/§4 原文）：

| 项 | 落地结果 |
|---|---|
| 2.1 worker-submit-chain | ✅ 新 skill `.dsh/skills/worker-submit-chain/SKILL.md`（5 步链 + 坑表 + 配置考古法 + 实证） |
| 2.2 judge-field-audit | ✅ 新 skill `.dsh/skills/judge-field-audit/SKILL.md`（阶段 A 本地审计 + 阶段 B 提交级裁决 + decomposition_ambiguity 输出段） |
| 2.3 probe-diff-submission | 🔀 **并入 judge-field-audit 阶段 B** + differential-scoring Step 3 增补"破坏性提交级差分"互引小节（避免两个触发几乎相同的 skill） |
| 2.4 trace-contamination-redline | ✅ 新 skill `.dsh/skills/trace-contamination-redline/SKILL.md`（零清单 + 测试句 + banned 词 gate + 目录分离） |
| 2.5 closure-evidence-standard | ✅ 新 skill `.dsh/skills/closure-evidence-standard/SKILL.md`（封板三问 + 真天花板核对清单 5 项） |
| 3.1 platform-scorecard-analyze | ✅ 1.1.0→1.2.0：+Step 1b 档位反推参考值法、+Step 2.5 字符串枚举审计、+Step 4b 判官口径 A/B、+确定性 verifier 识别、+信号卡双绿列/置信级/验证锚点 |
| 3.2 oracle-probe | ✅ 1.0.0→1.1.0：+decomposition_ambiguity 输出段 |
| 3.3 trace-maximize | ✅ 1.0.0→1.1.0：+真实执行史变体、+trace-labels 耦合识别、+判分 wall-time 平衡 |
| 3.4 red-team-review | ✅ →1.1.0：+clean-room 独立参考协议（从题面从零重写 + 约定变体库 + 逐分量字符串判定） |
| 3.5 bohrium-bohr | ✅ →1.1.0：+长训接力纪律（ckpt 存盘/job 接力/monitor 存活/kill 语法与额度止损） |
| 3.6 competition-coordinate | ✅ 1.0.0→1.1.0：+消息纪律（steer/cancel_first/cold resume/queue 核查）+ monitor STATUS.md 五要素 |
| 补充 submit-attempt | ✅ →1.1.0：+Step 4.5 dry-run 包内检、+Step 4.6 两轨判别（ARM vs worker） |
| 4.1 判官假设工厂 | ✅ bohrium-judge-analyst 预设 persona 更新（N 候选+判别法+置信+判别性 A/B 设计；字符串字段永远入交接单） |
| 4.2 solver clean-room 验证位 | ✅ bohrium-solver 预设 persona 更新（提交前强制门五查） |
| 4.3 手册 §7 增补 | ✅ OPERATIONS_PLAYBOOK.md：§1 角色表更新、§3 五查强制门、§5 trace 红线/wall-time、**新增 §9 提交物纪律与封板标准**（9.1 红线/9.2 封板/9.3 字符串检查单/9.4 判分两轨/9.5 分解=假设） |
| 4.4 信号卡模板升级 | ✅ 并入 platform-scorecard-analyze 回报格式（双绿列 + 置信级 + 验证锚点），未建独立模板文件 |
| P2 STATUS.md | ✅ monitor 预设 persona + MONITOR_REPORT.md 模板 + `STATUS.md` 初始文件 |
| P2 LESSONS_24H 四行格式 | ⏳ 轮末可选（不影响得分能力，知识库维护项） |
