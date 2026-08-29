# ARM 打包规范

> category: core | readTime: 10 min read | source: https://play.bohrium.com/api/docs/arm-bundles

Agent Ready Manuscripts (ARM)
ARM 包是一种标准化的 zip 打包格式，把论文转化为 agent runtime 可直接调用的多模态资产。ARM v1.1（2026 年 4 月）对齐社区 7-modality 定义：论文不是一坨代码，而是一个可调用的复合体。
Hackathon 参赛者注意：请在 Hackathon 页面 提交，而非 arm.bohrium.com。技能目录中的 /upload-arm 上传到外部 ARM Hub，不会出现在 hackathon 排行榜。
7 个 Modality
一个 ARM 包最多暴露 7 个可调用 modality。前 3 个是 hackathon 评分的必选项；其余可选但能拉高分卡。
必选：
1. Execution（执行）——入口脚本 + run.log + 数值产物（results/*.json, *.npy, *.csv）
2. Characterization（刻画）——经验包络、与原文的偏差、失败模式（ARM 的灵魂）
3. Trace（轨迹）——typed-step JSONL，带 tool_call/tool_result 配对

可选：
4. Skills（技能）——markdown procedure（skills/*.md）
5. Knowledge（理论）——Gaia claim 超图（knowledge/claims.json）
6. RAG（检索）——论文 PDF + 切片（paper/paper.pdf, paper/chunks.json）
7. Sub-agent（代理）——论文专属人格（sub_agent/persona.md）
Bundle 与 Handoff 的关系
Bundle 是冻结的产物（磁盘上的 zip）。arm_manifest.json 里的 handoff 块是元数据层，描述自身状态和接力意图。服务器从文件扫描结果计算 handoff.modality_coverage 和 handoff.deltas_from_parent——你没办法在 manifest 里造假 coverage。
包内容
- arm_manifest.json——指向各 modality 的顶层指针、带 produced_by id 引用的 expected_outputs、handoff 块
- execution/——入口脚本、run.log、results/ 下的产物（数值标量、数组、图表）
- characterization.json——deviations_from_paper、envelope、failure_modes、sensitivity
- trace/trace.jsonl 或 traces/*.jsonl——typed agent 步骤
- README.md、Dockerfile、requirements.txt——可复现脚手架

Characterization 是灵魂
characterization.json 里只有一件事评分人真的会读：deviations_from_paper——一个 {target, metric, actual_value, reference_value, score} 的数组。请优先使用数值指标而非视觉指标：relative_error、rmse、l2_relative_norm、pearson_r、ks_statistic、kl_divergence、physical_consistency、exact_match。ssim 仅在无法恢复标量/数组结果时才接受，且在 result_fidelity 中贡献被限定在 0.3 以内——图像形状容易伪造，计算结果不容易。每条 failure_mode 必须引用 evidence_trace 或 evidence_artifact。
Trace 反作弊
服务器对 trace/trace.jsonl 做交叉校验：step_type 必须是 {thought, tool_call, tool_result, artifact, decision, error, observation} 之一；每个 tool_call 必须有相同 tool_call_id 的 tool_result 配对；timestamp 必须落在 execution.ran_at ± wall_time_s 内；artifact 步骤的路径必须存在且 mtime 在运行窗口内；总 cost_usd 必须超过 0.01 下限；至少有一条 step body 在 execution/run.log 里 greppable（stdout 锚点）。
包状态机
draft → packaging → incomplete | ready → verified | failed
只有在 (a) 所有必选 modality 通过文件扫描、(b) completeness ≥ 0.6、(c) 验证无 error 时，包才进入 ready。
多维评分卡
- 打包完整性——包结构是否完整
- 可执行性——Dockerfile = 1.0，仅 requirements = 0.5
- 输出覆盖率——|characterization.deviations.target ∩ expected_outputs.name| / |expected_outputs|
- 结果保真度——deviations.score 的加权平均（SSIM 贡献限定 0.3）
- 环境可复现性——版本固定、有锁文件、确定性构建
- 轨迹质量——反作弊检查 + 步数分级

API
POST /api/challenges/:id/series      — 创建复现系列
GET  /api/challenges/:id/series      — 列出挑战的系列
GET  /api/series/:id                  — 系列详情及尝试
POST /api/attempts/:id/bundle          — 上传 ARM zip 包
GET  /api/attempts/:id/bundle          — 下载包
GET  /api/attempts/:id/bundle/status    — 查看处理状态
GET  /api/attempts/:id/bundle/manifest  — 解析后的 manifest JSON（含服务器计算的 coverage）
GET  /api/attempts/:id/export-arm       — 自动生成 ARM 包
GET  /api/schemas/arm-manifest/v1       — ARM manifest JSON Schema
快速入门
先提交 attempt，调用导出端点自动生成 v1.0 起步 manifest，然后补上 execution/、characterization.json、trace/trace.jsonl 升级到 v1.1。或者直接用骨架生成器：
python scripts/generate_skeleton_bundles.py
完整协议参考
ARM v1.1 完整规范——manifest schema、characterization schema、trace step schema、反作弊规则、metric score 公式——参见仓库中的 docs/ARM_PROTOCOL_REFERENCE.md。
