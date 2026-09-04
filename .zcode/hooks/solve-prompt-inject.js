#!/usr/bin/env node
// ascodex 开题注入 — ZCode UserPromptSubmit 钩子。
// 提示词含触发词"开始解题"时注入解题纪律前言；否则静默放行，永不阻塞。
// 协议与 ponytail 同款：stdin 读事件 JSON，stdout 输出 hookSpecificOutput。

let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (c) => { raw += c; });
process.stdin.on('end', () => {
  let prompt = '';
  try {
    prompt = String(JSON.parse(raw.replace(/^\uFEFF/, '')).prompt || '');
  } catch (e) { process.exit(0); }
  // 触发集与 ascodex-solve 技能 description 对齐
  if (!/(开始解题|解这道题|playground\s*解题|play\.bohrium\.com)/i.test(prompt)) process.exit(0);

  const ctx = [
    '[ascodex-solve 解题会话已识别]',
    '模式：单会话独立解题。不 spawn 解题子代理，不跨会话协调；红队视角用 unstuck-switch-angle 在会话内换角度实现。',
    '',
    '动手前必做（开场六步，详见 .agents/skills/ascodex-solve/SKILL.md）：',
    '1. 读现行评分契约 config/playground-scoring-audit-2026-08-28.md（2026-08-28 后旧固定公式全部作废）。',
    '2. 题面逐字翻译评分契约为自建 verifier；识别判分器类型（platform-scorecard-analyze）。',
    '3. 只读核对（已按平台实况校准）：归属 GET /api/attempts?author=<identity>；余量 GET /api/challenges/<slug>/attempts 按 authorId 计数（每身份每题 10 次）；查 work/、bohrium-kb/round3_prep/IDENTITY_POOL.md 与归档防撞题。多会话并行时各题锁定各自身份，选中后在 IDENTITY_POOL.md 对应行标注 (占用中: <slug>)。429 换池内身份，禁新注册。',
    '4. 复制 work/_template/ → work/<slug>/。',
    '5. 开 execution/run.log，此后每个真实执行的命令与 stdout 都落在里面（trace 锚定的地面真值）；平台 API 响应（attempt id/分数/判罚）不进 run.log 与提交物，平台情报只写 diagnostics/。',
    '6. 向用户复述题目、判分器类型、身份余量、授权级别与计划。',
    '',
    '提交纪律（仓库钩子硬拦截，非自觉约束）：',
    '- 提交命令只有在对应题目的 work/<slug>/.submit-authorized 存在时放行（用户手工创建，内容=允许次数，每次原子扣 1，扣尽即拦；slug 从命令中 work/<slug>/ 路径或会话 cwd 解析，多会话并行各用各题的授权文件）。授权文件本身禁止模型创建/写入/删除。',
    '- 提交前必过且全绿：python work/_template/trace_check.py work/<slug>/trace/trace.jsonl --run-log work/<slug>/execution/run.log --root work/<slug>',
    '  python work/_template/redline_scan.py work/<slug>',
    '- 审计先走 bohrium-kb/tools/submit_gate_audit.py；真实提交用 python work/<slug>/submit_bundle.py --challenge <id>（先 --dry-run 看包内清单；draft 不附 script 字段）。',
    '- 凭据只用进程环境变量 PLAYGROUND_TOKEN / BOHRIUM_TOKEN，禁止写文件/prompt/打印。',
    '- submitted/queued 不是成功：提交后只读核实 replay、resultsJson、scorecard、credited owner、fresh rescore、榜单 scope。',
    '',
    '红线：提交物零分数/attempt id/判官结论/他人做法/榜单情报；trace 禁止脚本合成，只从真实执行记录转录；论文数值只作交叉，以题面 verifier 为准。',
    '卡死：距最高档 ≥2 档无进展 → unstuck-switch-angle 换角度。收板：closure-evidence-standard 封板三问。',
    '回报五要素：attempt id + 身份 + harbor + trace 位置 + 判词。',
  ].join('\n');

  process.stdout.write(JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'UserPromptSubmit',
      additionalContext: ctx,
    },
  }));
});
