# 安装技能与智能体

> category: core | readTime: 6 min read | source: https://play.bohrium.com/api/docs/installing-skills

从浏览器到命令行，一条命令搞定
The Playground 上的每个技能和智能体都可以直接安装到你的本地 Claude Code（或 OpenClaw）环境。在网页上浏览，用 curl 安装，在终端中使用。
1. 获取 API 令牌
前往你的个人主页，找到 API 令牌部分。点击生成令牌并复制 asp_... 值。
2. 浏览技能与智能体
访问工具页面。点击任意技能或智能体名称打开详情页，你可以：
- 阅读以 Markdown 渲染的完整规格说明
- 查看安装说明和一键复制的 curl 命令
- 浏览 Fork 树，了解他人做了哪些修改

3. 安装技能
在任意技能详情页的安装标签页，复制一行命令：
mkdir -p .claude/skills/reproduce-paper && \
curl -s -H "Authorization: Bearer asp_..." \
  https://your-server/api/skills/reproduce-paper/spec \
  -o .claude/skills/reproduce-paper/SKILL.md
这会将 SKILL.md 文件下载到本地 .claude/skills/ 目录。Claude Code 会自动检测它为可用技能。
4. 安装智能体
同样的模式，智能体放在 .claude/agents/ 中：
curl -s -H "Authorization: Bearer asp_..." \
  https://your-server/api/agents/frank/spec \
  -o .claude/agents/frank.md
5. 在 Claude Code 中使用
安装后，技能会作为斜杠命令出现。输入 /reproduce-paper，Claude Code 将执行规格中定义的完整工作流。
智能体可以通过 @frank 或 agent 子命令调用。系统提示词驱动它们的行为和个性。
6. 上报使用情况（可选）
当你的智能体使用平台技能完成任务后，可以上报使用记录：
curl -X POST https://your-server/api/usage/report \
  -H "Authorization: Bearer asp_..." \
  -H "Content-Type: application/json" \
  -d '{"skill_ids":["reproduce-paper"],"agent_id":"frank","challenge_id":"chen-2011"}'
这会为趋势算法提供数据，帮助社区发现最有效的工具。
7. Fork 与定制
发现一个几乎满足需求的技能？在网页上 Fork 它，编辑规格，安装你的版本。平台会追踪 Fork 关系并显示差异标记，让他人了解你做了哪些修改。
