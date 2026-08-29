# 工具源码快照

这里保留源 ASCLocal 的 Playground/Bohrium 工具，便于审计和逐步适配。它们不是已经接入 Codex 的安全插件：部分脚本会 POST、删除、注册身份、读取旧 DSH 凭据路径或轮询外部服务。

执行前必须：

1. 先读脚本的 argparse/help 和调用链；
2. 确认 token 仅来自当前进程的 `BOHRIUM_TOKEN`/`PLAYGROUND_TOKEN`，不从文件回退；
3. 对写操作先使用 dry-run，向用户报告目标、身份、配额和回滚方案；
4. 提交后用 live attempt 核对 replay、`resultsJson`、`scorecard` 和 `harbor_reward`。

优先从 `bohrium_client.py` 的只读 GET 方法开始；`agent_admin.py`、`delete_*`、`cleanup_*`、`submit_*` 等脚本需要单独审查，不得批量运行。

提交后的状态核验使用 `verify_attempt.py`：

```powershell
python bohrium-kb/tools/verify_attempt.py --fixture attempt.json --challenge-id <challenge-id>
```

`--live` 仅允许 `https://play.bohrium.com` 的 GET，请求凭据来自当前进程 `PLAYGROUND_TOKEN` 或 `BOHRIUM_TOKEN`（前者优先）；`submitted`/`queued`、缺 replay、空 `resultsJson` 或空 `scorecard` 都会返回未核验。

fixture 可以携带榜单数据，工具不会访问网络：

```json
{
  "attempt": {
    "id": "attempt-1",
    "challengeId": "challenge-a",
    "status": "scored",
    "scorecard": {"harbor_replay_executed": 1, "harbor_reward": 0.91},
    "resultsJson": {"value": 1}
  },
  "leaderboard": {"entries": [{"attemptId": "attempt-1"}]}
}
```

`--live --check-leaderboard` 会从 `/api/challenges/{challengeId}/attempts` 读取最多 20 页，只跟随 `play.bohrium.com` 的 HTTPS `next` 链接；不会 POST、PATCH、DELETE 或提交新 attempt。分页响应可能是 `data`、`attempts`、`items`、`entries` 或 `leaderboard` 包装，平台若只返回最新 20 条仍按严格证据规则处理。
