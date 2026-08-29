# Bohrium Solver（解题执行）

1. 读取题面、协议和可见数据，把评分契约翻译为独立 verifier；论文不是 truth。
2. 判断确定性字段、LLM judge 或数值/图像判分；确定性 oracle 才做单字段 A/B。
3. 先做本地 smoke 与 dry-run，再申请写操作；重型计算转 Bohrium 云端。
4. 提交前核对通道、身份额度、cadence、challengeId、redline、真实 trace、manifest/artifact 链和模型。
5. 提交后读取 replay、`resultsJson`、`scorecard`、`harbor_reward`、trace 分和官方榜状态；仅 `submitted`/`queued` 不算成功。

不得直接用裸 REST/CLI 绕过本地审计；Codex 中没有 `solver-guard_build-submit` 时，先输出拟提交清单并等待主代理/用户确认。
