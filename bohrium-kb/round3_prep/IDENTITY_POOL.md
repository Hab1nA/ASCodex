# 身份池（Ultron / Jarvis / Friday 三系列 24 身份）

> 2026-09-04 重建：本文件只记录 24 个授权身份的 `显示名 → 真实 id → 凭据文件` 绑定与当前有效性，
> **不记录任何历史解题使用备注**（历史战况看 `archive/` 与 DECISION_LOG，不在这里）。
> 全部绑定经平台 `/agent/register` roster 的 token-prefix 实测核对，存活为当日只读探测结果。

## 24 身份总表（2026-09-04 全量实测：绑定一致、token 全部存活 HTTP 200）

| 显示名 | 真实 id（authorId） | 凭据文件（token 所在） |
|---|---|---|
| Friday-01 | `friday-n55379-n1` | `~/.dsh/friday-n55379-n1_credentials.txt` |
| Friday-02 | `friday-n55379-n2` | `~/.dsh/friday-n55379-n2_credentials.txt` |
| Friday-03 | `friday-n55379-n3` | `~/.dsh/friday-n55379-n3_credentials.txt` |
| Friday-04 | `friday-p51288` | `~/.dsh/cred_friday-p51288.txt` |
| Friday-05 | `friday-r1` | `~/.dsh/friday-r1_credentials.txt` |
| Friday-06 | `friday-r2` | `~/.dsh/friday-r2_credentials.txt` |
| Friday-07 | `friday-r3` | `~/.dsh/friday-r3_credentials.txt` |
| Friday-08 | `friday-s2-24714` | `~/.dsh/agent2_credentials.txt` |
| Jarvis-01 | `friday-s3-67618` | `~/.dsh/agent3_credentials.txt` |
| Jarvis-02 | `friday-t51795` | `~/.dsh/agent_t1_credentials.txt` |
| Jarvis-03 | `friday-u1` | `~/.dsh/agent_u1_credentials.txt` |
| Jarvis-04 | `friday-u2` | `~/.dsh/friday-u2_credentials.txt` |
| Jarvis-05 | `friday-u2-51065` | `~/.dsh/friday-u2-51065_credentials.txt` |
| Jarvis-06 | `friday-u3` | `~/.dsh/friday-u3_credentials.txt` |
| Jarvis-07 | `friday-u3-51065` | `~/.dsh/friday-u3-51065_credentials.txt` |
| Jarvis-08 | `friday-u4-52367` | `~/.dsh/friday-u4-52367_credentials.txt` |
| Ultron-01 | `friday-u5-52903` | `~/.dsh/friday-u5-52903_credentials.txt` |
| Ultron-02 | `friday-u6-53704` | `~/.dsh/friday-u6-53704_credentials.txt` |
| Ultron-03 | `friday-u7-54212` | `~/.dsh/friday-u7-54212_credentials.txt` |
| Ultron-04 | `jarvis` | `~/.dsh/jarvis_credentials.txt` |
| Ultron-05 | `jarvis-2` | `~/.config/playground/agents/jarvis-2.env`（PLAYGROUND_TOKEN 字段） |
| Ultron-06 | `jarvis-3` | `~/.config/playground/agents/jarvis-3.env`（PLAYGROUND_TOKEN 字段） |
| Ultron-07 | `jarvis-4` | `~/.config/playground/agents/jarvis-4.env`（PLAYGROUND_TOKEN 字段） |
| Ultron-08 | `ultron` | `~/.config/playground/credentials.env`（PLAYGROUND_TOKEN 字段） |

- 显示名只用于排行榜/署名；**authorId 与额度一律按真实 id 记**。
- human 主账户令牌（用户级环境变量 `PLAYGROUND_TOKEN`）不属身份池，仅作管理操作与默认回退。

## 使用规则（现行，无历史条款）

1. **池冻结**：2026-08-15 起禁止注册任何新身份；只用上表 24 个。
2. **额度**：每身份每题 10 次提交。用前自查：
   `GET /api/challenges/<slug>/attempts` 按 `authorId` 过滤计数，余量 = 10 − 已用。
3. **429**：换同题余量充足的身份，禁止新注册。
4. **token 注入**：只进当前进程环境，禁止写入文件/提交物/打印：
   ```bash
   export PLAYGROUND_TOKEN=$(grep -oP 'api_token\s*=\s*\K\S+' ~/.dsh/<凭据文件>)
   ```
5. **提交轨**：当前唯一有效提分通道 = ARM bundle 轨（`work/<slug>/submit_bundle.py`，
   draft 不附 script 字段）。Worker 轨（playground CLI）因平台侧账户授权缺口
   `worker_unauthorized(401)` 暂不可用（状态见下）。
6. **授权纪律**：真实提交前必须由用户创建 `work/<slug>/.submit-authorized`（提交门钩子校验），
   会话内只 dry-run；`submitted`/`queued` 不是成功，提交后按 submit-attempt Step 5 只读核验。

## Worker 轨状态备忘（非身份条款）

CLI 账号（上表 Ultron-04~08，id=jarvis/jarvis-2/3/4/ultron）提交后停在
`pending_review + execStatus=failed + scorecard 全 0`（38872/38875/38876/38932，
2026-09-04 复测仍未恢复）。恢复条件：账户在 Worker 系统完成授权 / CLI 升级后重试探针。
