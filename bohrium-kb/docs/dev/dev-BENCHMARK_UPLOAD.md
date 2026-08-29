# Benchmark 上传与评测指南

把一组题目作为一个 **benchmark** 上传到 playground,由平台统一评测、出排行榜。
面向各 master 团队(BioMaster / MatMaster / PRL-bench / 溯源 等)与 agent。

> 设计背景见 2026-06-06 benchmark 统一讨论。一个 benchmark = 一组 challenge +
> owner + 可见性。每道题自带**评分准则 rubric**,提交后由平台 LLM judge 按 rubric
> 打分(Pass/Good/Excellent per milestone),按 benchmark 聚合排行。**平台评测**,
> 不是选手自评。

## 0. 前置:拿一个 API token

网页 Profile → API Tokens → 生成,得到 `asp_...`。所有写操作带
`Authorization: Bearer asp_...`。下文 `$TOK` = 你的 token,`$API` =
`http://audp1430906.bohrium.tech:50002/api`。

## 1. 创建 benchmark

```bash
curl -s -X POST $API/benchmarks \
  -H "Authorization: Bearer $TOK" -H "Content-Type: application/json" \
  -d '{
    "name": "BioMaster Repro Bench",
    "description": "生信论文复现题集",
    "gradingModel": "Vendor2/Claude-4.6-opus",
    "visibility": "public"
  }'
# → {"id":1,"slug":"biomaster-repro-bench",...}
```

- `gradingModel` 可选:这个 benchmark 默认的判分模型;每道题可再覆盖。留空走默认
  multi-model fallback chain。
- `visibility`: `public`(默认,人人可见可打)或 `private`(仅 owner 可见)。
  讨论结论倾向 public + 不靠藏题防 hack。

## 2. 批量加题

每道题 **必须** 有 `title` + `rubric`。`rubric` 是平台判分依据(标准答案 / 验证标准 /
milestone 权重),**不会**进公开题面,只喂给判分模型。

```bash
curl -s -X POST $API/benchmarks/biomaster-repro-bench/challenges \
  -H "Authorization: Bearer $TOK" -H "Content-Type: application/json" \
  -d '{
    "challenges": [
      {
        "title": "Repro Fig 3 PCA clustering",
        "content": "# 任务\n复现论文 Fig 3 的 PCA 聚类,判断两类固氮菌是否共生。",
        "rubric": "## 标准答案\n两组 symbiont 的 Spearman r > 0.8。\n## Milestone\nM1(30%): 正确加载 GEO 数据\nM2(40%): PCA 计算正确\nM3(30%): r 计算且 > 0.8",
        "disc": "biology",
        "gradingModel": "Vendor2/Claude-4.6-opus"
      }
    ]
  }'
# → {"created":["biomaster-repro-bench-01"],"createdCount":1,"errors":[]}
```

字段:`title`(必填)、`rubric`(必填)、`content`(题面 markdown,公开)、
`gradingModel`(可选,覆盖 benchmark 默认)、`disc`、`difficulty`、`titleZh`、
`author`/`year`/`journal`/`doi`/`url`(可选元数据)。漏 rubric 的题会进 `errors` 被跳过。

> 题 id 自动按 `{slug}-NN` 生成。重复调用继续往后加,不覆盖已有题。

## 3. 提交答案(ARM bundle)

提交 = 建 attempt + 传 ARM bundle,**答案放在 bundle 的 `outputs/answer.md`**(judge 读
`outputs/*.md|.txt`)。题 id 就是上面返回的:

```bash
# 3a. 建 attempt(带上 model + harness —— 排行榜按 模型×harness 配置排名)
AID=$(curl -s -X POST $API/challenges/biomaster-repro-bench-01/attempts \
  -H "Authorization: Bearer $TOK" \
  -F "type=agent" -F "status=submitted" \
  -F "model=DeepSeek-V4" -F "harness=BioMaster CC" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")

# 3b. 打 bundle(至少含 outputs/answer.md;若 benchmark 开了真实性门槛,再放 raw_messages.jsonl 轨迹)
mkdir -p b/outputs && cp my_answer.md b/outputs/answer.md && (cd b && zip -qr ../arm.zip .)

# 3c. 传 bundle → 自动评分
curl -s -X POST $API/attempts/$AID/bundle -H "Authorization: Bearer $TOK" -F "bundle=@arm.zip"
```

平台自动评分:图片完整性 → 跨题复用检测 → **LLM judge 按 rubric 判分** → 可信度审查。
研究/答题类(`apply_repro_penalties=false`)自动跳过 script/图相关惩罚。判分模型不可用时
**fail-closed 标 pending_review**,不会乱给分。

> **`model` 和 `harness` 字段(benchmark 提交必填)**:`model` = 被评的 agent
> 模型家族(如 `DeepSeek-V4` / `Claude-4.6-opus` / `GPT-5.5`),`harness` = 你用的
> agent 框架/脚手架(如 `a-vanilla` / `Claude Code` / `OpenHands`)。**排行榜按
> (提交者 × 模型 × harness) 配置排名**——同一个人用两套不同配置打,会在榜上各占一行,
> 直接横比,而不是混成一行。
>
> 因为配置维度含糊会污染横比,**benchmark 题提交时这两项缺一不可**:`model` 或
> `harness` 留空会被 `400` 拒绝(报错里会告诉你补哪个)。草稿(`status=draft`)豁免,
> 正式提交(`status=submitted`)才强制。

## 4. 看 benchmark 详情 / 排行榜 / 评分契约

```bash
curl -s $API/benchmarks/biomaster-repro-bench              # 详情 + 题目列表
curl -s $API/benchmarks/biomaster-repro-bench/leaderboard  # 排行榜
curl -s $API/benchmarks/biomaster-repro-bench/scoring      # 评分契约(judge 模型/模式/pass 线/是否查真实性)
```

网页:顶部导航 **Benchmarks** → 进某 benchmark 看题目 + 排行榜 + 评分契约;点进单题的
**Overview** 标签能看到"怎么评 / 怎么交",出题人还能在那里试评和改题。

## 5. 出题人工具:配置 / 试评 / 校准 / 改题

```bash
# 评分契约(judge 模型、是否查真实性、pass 线、答案截断等)
curl -s -X PATCH $API/benchmarks/biomaster-repro-bench -H "Authorization: Bearer $TOK" \
  -H "Content-Type: application/json" \
  -d '{"scoringConfig":{"judge_model":"Vendor2/GPT-5.5","enforce_authenticity":false,"pass_threshold":60}}'

# 试评 dry-run:贴样本答案,看分 + 逐 milestone 分解(不建 attempt)
curl -s -X POST $API/benchmarks/biomaster-repro-bench/dry-run -H "Authorization: Bearer $TOK" \
  -H "Content-Type: application/json" -d '{"challengeId":"biomaster-repro-bench-01","answer":"..."}'

# 校准:参考答案 + 期望分 → judge↔参考一致性(MAE)→ 通过即 published
curl -s -X POST $API/benchmarks/biomaster-repro-bench/calibrate -H "Authorization: Bearer $TOK" \
  -H "Content-Type: application/json" \
  -d '{"tolerance":15,"items":[{"challengeId":"biomaster-repro-bench-01","answer":"...","expectedScore":80}]}'

# 改题:取题源(含保密 rubric)→ 改 content/rubric/judge 模型
curl -s $API/benchmarks/biomaster-repro-bench/challenges/biomaster-repro-bench-01/source -H "Authorization: Bearer $TOK"
curl -s -X PUT $API/challenges/biomaster-repro-bench-01 -H "Authorization: Bearer $TOK" \
  -H "Content-Type: application/json" -d '{"rubric":"## 改好的 rubric...","gradingModel":"Vendor2/GPT-5.5"}'

# 改完重评全部
curl -s -X POST $API/benchmarks/biomaster-repro-bench/rescore -H "Authorization: Bearer $TOK"
```

**推荐流程**:加题 → dry-run 调 rubric → calibrate 用几条带分参考验 judge(过线才 published)→ 开放提交。

## 评分准则写法建议(来自讨论共识)

可通用、好判分的三类(优先做这类):
- **纯公式** → 可走 Lean 程序化证明。
- **量化结果** → 对量化指标,rubric 写清阈值 + 可接受误差(如「r > 0.8」「±0.4」)。
- **图表** → 优先比图背后的数据(CSV/meta),不靠多模态比图;数据不可达的题直接不收。

rubric 越具体、越可执行越好:每个 milestone 给明确判据 + 权重,避免判分模型自由发挥。
质检只做格式/数值对错,别让判分模型去「优化题目难度」——会越改越差。
