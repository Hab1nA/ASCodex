# DeepSeek Harness 工具用法手册（防污染固化版）

> 固化时间：2026-08-22 09:00 CST｜维护人：Ox Alpha 总负责人
> 触发场景：执行器出现「意图→动作」错配（想发消息却发出 spawn/config）时，**先读本文核对签名，再动作**。
> 本文全部条目来自 tool_describe 权威输出 + 2026-08-21/22 夜间实战验证，非记忆推断。

## 一、子代理生命周期类

### 1.1 创建（派单）

| 工具 | 参数签名 | 路由行为 | 实战纪律 |
|---|---|---|---|
| `subagent` / `subagent_solver` / `subagent_judge` / `subagent_redteam` / `subagent_monitor` | `{description, prompt, run_in_background?}` | 继承父会话路由 | ✅ 零路由失误记录；⚠ 仍需派后 probe 核验 |
| `subagent_run` | `{description, prompt, provider?, model?, maxTokens?, reasoningEffort?, temperature?, agent_preset?}` | 显式指定 | ⚠⚠ 两颗雷：①`reasoningEffort` 在 opencode-go-zen/x-preview-f-free 上**任何值都被拒**（max/high 均报 does not support）→ **永远省略**；②曾观察到显式 model 未按预期生效而继承 packycode/grok-4.6 → **派单后必 probe 核验** |
| `subagent_fork` | `{description, prompt, run_in_background?}` | 继承本会话上下文（seeded with completed turns） | 子任务依赖主对话上下文时用 |

**角色工具 vs subagent_run 的选择**：优先角色工具（参数面窄、无法误传 effort/model）；仅当需要 agent_preset 等特殊参数才用 subagent_run，且派后必核路由。

### 1.2 校准（改运行时配置）

```
subagent_config {subagent_id!, provider?, model?, maxTokens?, reasoningEffort?, temperature?}
```
- 只改非空字段；变更持久化（进程重启后冷恢复仍生效）；从子代理下一次模型请求起生效。
- **本环境唯一合法组合**：`provider=opencode-go-zen, model=x-preview-f-free`，**绝不带 reasoningEffort**（适配器对 max/high/任意值均报 does not support——已三连实证）。
- 用途：路由纠偏（继承污染时）。校准后用 subagent_probe 复验 effective_config。

### 1.3 消息（唤醒 / 续作 / 插话）

| 工具 | 参数签名 | 行为 | 纪律 |
|---|---|---|---|
| `send_message` | `{subagent_id, message}` | 后台子代理续聊：排队为其下一 turn；运行中则等当前 turn 结束；**冷子代理自动恢复** | ✅ 最可靠；harness 重启后的唤醒一律首选此工具 |
| `subagent_send` | `{subagent_id, message, mode?: steer/followup/inject, cancel_first?}` | steer=插话进 running 代理当前 turn 的步骤边界；followup=排队新 turn（cold 自动回落）；inject=不唤醒仅注入；cancel_first=先取消当前活动 | running 代理→steer；禁止无 mode 的积压式 followup 泛滥 |
| ~~`send_agent_message`~~ | —— | **会话通信工具，禁止用于子代理**（报「目标是子代理，不能通过会话通信插件直接发送」） | 目标只能是兄弟会话 id |

### 1.4 巡检 / 干预 / 收敛

| 工具 | 参数签名 | 用途 |
|---|---|---|
| `subagent_probe` | `{subagent_id, limit?, max_chars?, since_turn?, include_reasoning?}` | 实时状态（running/resident/ready）+ 转写窗口 + **effective_config 核验**（派单后必做一次） |
| `subagent_queue` | `{subagent_id, action: list/remove/clear, message_id?}` | 待处理消息队列：list 看 next-turn/next-step 积压及 id；remove 删单条（仅驻留）；clear 清空（仅驻留）；已被消费进 turn 的消息不可删 |
| `interrupt_agent` | `{agent_id}` | 停止目标当前活动（队列保留、代理存活）；对已结束者 no-op。多实例冲突熔断的标配 |
| `subagent_wait` | `{subagent_ids[], timeout_seconds≤600, require: all/any, max_chars?}` | 当前 turn 内等子代理收敛（settlement notice 可能丢失时的替代） |
| `list_agents` | `{scope: children/descendants}` | 全量子代理清单与状态（running=idle/ready）；ready=仅存于存储可恢复 |

## 二、会话通信类（跨会话，非子代理！）

| 工具 | 参数签名 | 用途 | 红线 |
|---|---|---|---|
| `list_peer_agents` | `{}` | 列出可通信兄弟会话（id/title/cwd/status；status: online/其它） | **子代理不在列** |
| `send_agent_message` | `{to!, content!, mode?: followup/inject/steer}` | 向兄弟会话投递；目标离线自动恢复后投递 | ⚠⚠ to 填子代理 id 必报「目标是子代理」——子代理通信走 §1.3 |
| `check_delivery` | `{to!, messageId?}` | 投递状态：pending/claimed/discarded/unknown；claimed ≠ 已读 ≠ 已回复 | 监督场景主动调用，默认安静 |

**跨队内容交换红线**：只交换通道/机制/schema 级信息；他队解法数值、坐标、推理叙事不得进入我方提交物（污染红线）。

## 三、平台判分机制（2026-08-22 实证，影响提交轨选择）

1. **双轨互斥**：同一内容，CLI/手动链 → harbor 轨（延迟回填 18min~2h，属批处理机制非故障）；REST 四步链带 script → ARM 轨（即时判分）。两轨 scorecard 形态不同。
2. **清零波**：直挂 REST bundle（未走 worker /uploads）的历史分会被重评分批次按 `missing_worker_submission` 批量归零；worker 通道历史分免疫。-1000 罚分波有组委会撤销先例——异常分数先取证再行动。
3. **ARM 轨公式**（泛型题）：score = 70×result_fidelity + 30×modality_coverage + reasoning_bonus(5)；fidelity/coverage 由 deviations.target ∩ expected_outputs.name 交集驱动——**命名空间必须对齐官方 schema**（/api/schemas/characterization/v1 可查）。
4. **reasoning_bonus 配方**：trace 保留 thinking 链（~14000 字符级）= +5 封顶（tetra/matchgate 双实证）。
5. **qc 精度容差**：数值产物精度不足（如 6dp 舍入 vs 金标全精度）会在容差检查丢分——cnvkit 89→100 唯一缺口。
6. **CLI 全自动轨**：playground submit 后 worker 自主完成挂载→replay→评分，**绝不手动 POST /submit**（会与 worker 赛跑导致提前定稿）。
7. **bohr ACCESS_KEY**：失效时报 unsupported protocol scheme/401——影响云训车道，需用户侧更新凭据。

## 四、执行器防污染 SOP（给未来轮值的自己）

1. **单一动作原则**：从失败/掉线通知恢复时，一个 turn 只做一个动作；做完即收口汇报，不串联。
2. **循环识别**：若连续两次发射了同类冗余调用（config 轰炸/spawn 连发），立即 text-only 收口，等外部消息重置。
3. **spawn 三问**：准备 subagent_run 前自问——a) 角色工具能否替代？b) reasoningEffort 是否已确保省略？c) 派后 probe 核验排上了吗？
4. **消息三查**：发往子代理 → send_message/subagent_send；发往兄弟会话 → send_agent_message(to=session-id)；查投递 → check_delivery。
5. **队列卫生**：定期 subagent_queue list 普查；堆积即 clear + steer 合并重发（mode=steer 插话式）。
6. **凭据安全**：token 只从凭据文件读入 env 变量使用，永不回显、永不写入产物。
