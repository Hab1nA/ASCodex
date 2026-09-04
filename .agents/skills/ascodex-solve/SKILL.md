---
name: ascodex-solve
description: "ZCode 单会话解题总入口：Bohrium Playground 一题一会话的完整流程——开场六步、自建 verifier、四阶段解题、证据校验、提交门与回报五要素。触发词：'开始解题'、'解这道题'、'playground 解题'、challenge URL/ID。取代多代理派发模式：一个会话独立解完一道题，从开题到提交回报。"
metadata:
  version: 1.0.0
  author: friday-team
  tags: [bohrium-playground, solve, zcode, single-session]
---

# ASCodex 单会话解题（ZCode）

一个会话 = 一道题。没有总负责人、没有解题子代理、没有跨会话协调：你在本会话里完成从开题到提交回报的全程。平台知识在 `bohrium-kb/`，战术细节在各专项技能（文末索引）；本技能是流程主干与强制门。

## 0. 模式与硬约束（先读）

- **单会话**：不 spawn 解题子代理。需要红队/clean-room 视角时按 `unstuck-switch-angle` 在会话内换角度执行，或建议用户另开会话。
- **提交门（仓库钩子硬拦截，非自觉约束）**：针对 `play.bohrium.com` 的写命令与 `submit_bundle.py` 上传会被 `.zcode/hooks/submit-gate.js` 拦截，除非 `work/<slug>/.submit-authorized` 存在——该文件由**用户手工**在会话外创建，首个非空行 = 本次允许提交次数 N，每次放行扣 1，扣尽即拦；`work/` 下必须恰好一个授权文件。`--dry-run` 与 `bohrium-kb/tools/submit_gate_audit.py` 审计不受限。**禁止创建/写入/删除/读取该授权文件**（钩子会拒绝一切触及它的模型操作）；要查剩余次数就问用户。
- **凭据**：只用进程环境变量 `PLAYGROUND_TOKEN` / `BOHRIUM_TOKEN`；禁止写入文件、prompt 或打印。
- **默认 dry-run**：无授权文件时只允许只读 GET 与本地计算。

## 1. 开场六步（动手计算前全部完成）

1. 读现行评分契约 `config/playground-scoring-audit-2026-08-28.md`（2026-08-28 后旧固定公式如"trace≥70 / 8 规则 / -1000 翻账"全部作废，以该契约与现行题面为准）。
2. 读题面全文，**逐字翻译 §5/§6 评分契约为自建 verifier**（jarvis 法）；识别判分器类型（A 确定性 verifier / B LLM judge / C 内容比对，见 `platform-scorecard-analyze`）；列保留名清单、形式要求、输出 schema。
3. 只读核对：`GET /api/attempts` 查该身份该题余量与归属；查 `work/`、`bohrium-kb/round3_prep/IDENTITY_POOL.md` 与归档防撞题。429 → 换池内身份，禁新注册。
4. 复制 `work/_template/` → `work/<slug>/`，slug 用原始 challenge slug。
5. 开 `execution/run.log`：此后**每个真实执行的命令与 stdout 都落在里面**（trace 锚定的地面真值）。
6. 向用户复述：题目、判分器类型、身份与余量、授权级别（无授权文件 = 本会话仅 dry-run）、解题计划。

## 2. 解题主线（四阶段，细节见 `playground-solve-optimal`）

1. **开题侦察**：自建 verifier + 判分器类型 + 判官信号卡（档位结构、已知高分档）。
2. **首解**：科学正确性先行；判官口径以题目公式与保留名为准，论文数值只作交叉（论文可能错）。
3. **优化**：每次提交 = 一次受控实验——假设 → 单字段 A/B → 提交 → 读 harbor → diff。距最高档 ≥2 档且无进展 → 触发 `unstuck-switch-angle`。
4. **满分配置**：A 类对齐 hidden reference（裸值/保留名/符号保留/因子归属/正 q）；B 类 canonical 推导 + 完备性，判词驱动；C 类内容对齐 truth model + 参数化悬崖扫描。

高频坑速查（论文数值错、角平均 vs 裸值、因子双计、符号保留、负值被拒、答案前置等）见 `playground-solve-optimal` 末表。

## 3. 证据与提交（每一步都是门）

1. **trace**：`work/<slug>/trace/trace.jsonl` 从真实执行记录转录（ZCode 会话记录 + run.log 为取材与锚定来源；schema 与铁律见 `real-trace-capture`）。**禁止脚本合成**——`make_traces.py` 只是 schema 参考模板，不是生成器。
2. **本地校验（必须全绿才能进入提交流程）**：
   ```
   python work/_template/trace_check.py work/<slug>/trace/trace.jsonl --run-log work/<slug>/execution/run.log --root work/<slug>
   python work/_template/redline_scan.py work/<slug>
   ```
   红线命中时改写提交物后重扫，不得放宽词表。
3. **审计**：`python bohrium-kb/tools/submit_gate_audit.py`（提交六门对照，只读）。
4. **请求授权**：向用户说明本次提交内容与预期，请用户创建 `.submit-authorized`（次数 N）。
5. **执行提交**：`python work/_template/submit_bundle.py --challenge <id> --outcome <success|partial|failed|stuck> ...`（钩子扣减 1 次授权）。提交前 dry-run 看包内清单：契约点名文件逐字在包内（错名上传成功但 0 分）。
6. **提交后只读核实（`submitted`/`queued` 不是成功）**：replay、`resultsJson`、scorecard、credited owner 与授权身份一致、fresh rescore、榜单 scope（清单见 `submit-attempt` Step 5）。记录提交账本：attempt id、bundle revision、sha256、时间。

## 4. 卡死与收板

- 卡死检测与强制换角度：`unstuck-switch-angle`（clean-room 重读题、判官信号挖掘、换建模角度）。
- 收板前过 `closure-evidence-standard` 封板三问（场上有人在你上面？多独立证伪？本地曾达更高值？）——本地曾达更高值 = 禁止以预算止损封板。

## 5. 回报格式（五要素）

`attempt id + 身份 + harbor + trace 位置 + 判词/resultsJson`，并给出 `work/<slug>/` 证据清单：run.log、trace.jsonl、trace_check/redline_scan 输出、提交账本。

## 6. 专项技能索引

战术总纲 `playground-solve-optimal`｜判分器 `platform-scorecard-analyze`｜高分未满定位 `differential-scoring` + `judge-field-audit` + `oracle-probe`｜云算力 `bohrium-bohr`（重型计算走 Bohrium job，本地只做 ≤120s smoke）｜mp-r 同家族题 `mp-r-family-solve`｜长任务断点 `checkpoint`/`resume`。多代理协作 `competition-coordinate` 在单会话模式下不适用。

## 7. 本迁移明确不包含的原 ASCodex 运行时组件

账本 SQLite 与额度预留、Ed25519 policy 签名链、RoundPlan 原子派发与按题 binding、StageBrief 签发、spawn lineage 约束、AutoPush、网络 egress 白名单。单会话模式的等价物 = 提交门钩子 + 授权文件次数门 + 开场只读额度核对 + 本技能与各专项技能纪律。
