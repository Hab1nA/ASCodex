#!/usr/bin/env node
// ascodex 提交门 — ZCode PreToolUse 钩子（仓库 ASCLocal-Codex 专用）。
//
// 职责（ASCodex solver-guard 六道门在单会话模式的等价硬拦截）：
// 1. 拦截针对 play.bohrium.com 的写命令（POST/PUT/PATCH/DELETE、curl -d/-F/-T、
//    requests.post、urlopen(data=)、Invoke-RestMethod -Method Post、httpie、httpx 等）
//    以及 submit_bundle.py 上传调用：仅当对应题目工作区的
//    work/<slug>/.submit-authorized 存在时放行。文件由用户手工创建，首个非空行 =
//    剩余允许提交次数；每次放行原子扣 1，扣尽即删即拦。
//    slug 从提交命令中的 work/<slug>/ 路径或会话 cwd 解析；解析不出且 work/ 下
//    存在多个授权文件时拒绝（防跨题误扣）。
// 2. --dry-run 仅在"纯 submit_bundle.py 调用、无任何写意图标记"时豁免
//    （真实 POST 命令里塞 dryrun 字样不能绕过门）。
// 3. 防自授权篡改：模型侧命令/写文件一旦触及 .submit-authorized 一律拒绝
//    （授权文件只能由用户在会话外手工创建/修改）。
//
// 退出码：0 放行（静默）；2 拒绝（stderr 给原因）；其他 = 脚本错误。
// 已知边界：本门是启发式（文档备案于 AGENTS.md），补偿控制是提交后
// submit_gate_audit.py 审计与只读核验，不是唯一安全边界。

const fs = require('fs');
const path = require('path');

function deny(msg) {
  process.stderr.write('[submit-gate] DENY: ' + msg + '\n');
  process.exit(2);
}

function findProjectDir(ev) {
  const candidates = [
    process.env.ZCODE_PROJECT_DIR,
    process.env.CLAUDE_PROJECT_DIR,
  ].filter(Boolean);
  for (const dir of candidates) {
    if (fs.existsSync(path.join(dir, 'work'))) return dir;
  }
  // 兜底：从事件 cwd 向上走到含 work/ 与 .git 的仓库根
  let dir = ev.cwd || process.cwd();
  for (let i = 0; i < 12; i++) {
    if (fs.existsSync(path.join(dir, 'work')) && fs.existsSync(path.join(dir, '.git'))) {
      return dir;
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return candidates[0] || ev.cwd || process.cwd();
}

// 写意图：只认明确的写动词/数据标志。短旗标 -F/-T/-d 按原始大小写匹配，
// 避免误伤 `curl -fsSL`（--fail 只读 GET）与 pwsh 原生 GET 侦察。
function postIntent(cmd) {
  const lower = cmd.toLowerCase();
  return (
    /(-x|--request)\s*"?(post|put|patch|delete)\b/i.test(cmd) ||
    /--request=(post|put|patch|delete)\b/i.test(cmd) ||
    /--data(-raw|-binary|-urlencode)?\b|--form(-string)?\b|--upload-file\b|--post-data\b|--post-file\b/i.test(lower) ||
    /(^|\s)-[FT]\s/.test(cmd) ||
    /(^|\s)-d[\s="'`]/.test(cmd) ||
    /\brequests\.(post|put|patch|request|delete)\s*\(/.test(lower) ||
    /urlopen\s*\([^)]*(data|method)\s*=/i.test(cmd) ||
    /\bhttpx\.(post|put|patch|request|stream)\s*\(|axios\.(post|put|patch|request)\s*\(|\bfetch\s*\([^)]*method[^)]*(post|put|patch|delete)/i.test(lower) ||
    /invoke-(rest|web)method[^|;&]*-method\s*[:=]?\s*["']?(post|put|patch|delete)\b/i.test(lower) ||
    /(^|[&;|(]\s*)https?\s+(post|put|patch|delete)\b/i.test(lower)
  );
}

// submit_bundle.py 调用：解释器 token（py/python/python3.12…）必须是独立词，
// 避免把 "xx.py work/.../submit_bundle.py"（如 git diff 参数列表）误判为调用
function uploaderInvocation(lower) {
  return /(?:^|[&;|(]|\s)(?:py|python[0-9.]*)(?:\.exe)?\s+(?:-\S+\s+)*\S*submit_bundle\.py\b/.test(lower) ||
         /(^|[&;|(]\s*)submit_bundle\.py\b/.test(lower);
}

function submitIntent(cmd, lower) {
  const host = /play\.bohrium\.com/.test(lower);
  const attemptsApi = /\/api\/(attempts|challenges\/[^'"\s]*\/attempts)/.test(lower);
  return (host && postIntent(cmd)) || (attemptsApi && postIntent(cmd)) || uploaderInvocation(lower);
}

// slug 解析：优先命令文本中的 work/<slug>/（单段路径），其次会话 cwd 位于 work/<slug> 内
function resolveSlug(cmd, ev) {
  const m = cmd.match(/work[\/\\]+([^'"\s&|;)\/\\]+)/);
  if (m) return m[1];
  const cm = String(ev.cwd || '').match(/[\/\\]work[\/\\]([^\/\\]+)/);
  if (cm) return cm[1];
  return null;
}

function readRemaining(authPath) {
  const text = fs.readFileSync(authPath, 'utf8');
  const firstLine = text.split(/\r?\n/).map((s) => s.trim()).find((s) => s.length > 0) || '0';
  const n = parseInt(firstLine, 10);
  return Number.isInteger(n) ? n : NaN;
}

// 原子扣次：临时文件 + rename 覆盖，避免并发读改写双花
function decrementOrDelete(authPath, remaining) {
  if (remaining - 1 >= 1) {
    const tmp = authPath + '.tmp';
    fs.writeFileSync(tmp, String(remaining - 1) + '\n', 'utf8');
    fs.renameSync(tmp, authPath);
    return remaining - 1;
  }
  fs.unlinkSync(authPath);
  return 0;
}

let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (c) => { raw += c; });
process.stdin.on('end', () => {
  let ev = {};
  let cmd = '';
  let filePath = '';
  try {
    ev = JSON.parse(raw.replace(/^\uFEFF/, ''));
    const ti = ev.tool_input || ev.toolInput || {};
    cmd = String(ti.command || ti.cmd || '');
    filePath = String(ti.file_path || ti.filePath || ti.path || '');
  } catch (e) { process.exit(0); }

  const lower = (cmd + ' ' + filePath).toLowerCase();
  const tool = String(ev.tool_name || ev.toolName || '');

  // 防自授权：任何模型命令/写文件触及授权文件 → 拒绝（读写删一概不允许）
  if (lower.includes('.submit-authorized')) {
    deny('授权文件 .submit-authorized 只能由用户在会话外手工创建/修改，模型不得创建、写入、删除或读取它。请让用户手工操作该文件。');
  }

  if (tool !== 'Bash' && tool !== 'bash') process.exit(0);
  if (!cmd) process.exit(0);

  const writeIntent = submitIntent(cmd, lower);
  if (!writeIntent) process.exit(0);

  // dry-run 豁免：仅当意图只来自 submit_bundle.py 调用本身、命令不含任何真实写标记
  if (/--dry-run\b/.test(lower) && !postIntent(cmd)) process.exit(0);

  const projectDir = findProjectDir(ev);
  const workDir = path.join(projectDir, 'work');
  const slug = resolveSlug(cmd, ev);
  let authPath = slug ? path.join(workDir, slug, '.submit-authorized') : null;

  if (authPath) {
    let stat = null;
    try { stat = fs.statSync(authPath); } catch (e) { stat = null; }
    if (!stat || !stat.isFile()) {
      deny(`未找到题目 ${slug} 的提交授权：请由用户手工创建 work/${slug}/.submit-authorized（首个非空行 = 本次允许提交次数）。当前仅允许 dry-run 与只读审计。`);
    }
  } else {
    let authFiles = [];
    try {
      authFiles = fs.readdirSync(workDir)
        .map((name) => path.join(workDir, name, '.submit-authorized'))
        .filter((p) => { try { return fs.statSync(p).isFile(); } catch (e) { return false; } });
    } catch (e) {
      deny('work/ 目录不可读（' + e.message + '）。');
    }
    if (authFiles.length === 0) {
      deny('未找到任何提交授权。真实提交前由用户手工创建 work/<slug>/.submit-authorized（内容 = 允许次数）。当前仅允许 dry-run 与只读审计。');
    }
    if (authFiles.length > 1) {
      deny('提交命令未指明题目工作区，且 work/ 下存在多个授权文件（' + authFiles.join(', ') + '）。请在命令中带上 work/<slug>/ 路径，或让会话 cwd 位于 work/<slug> 内。');
    }
    authPath = authFiles[0];
  }

  let remaining = readRemaining(authPath);
  if (!Number.isInteger(remaining) || remaining < 1) {
    try { fs.unlinkSync(authPath); } catch (e) { /* 已不存在则忽略 */ }
    deny('授权次数已用尽，授权文件已移除。需要继续提交请让用户重新创建 .submit-authorized。');
  }

  try {
    const left = decrementOrDelete(authPath, remaining);
    process.stderr.write('[submit-gate] ALLOW: 消耗 1 次提交授权，剩余 ' + left + '（' + authPath + '）\n');
  } catch (e) {
    deny('扣减授权次数失败（' + e.message + '），按 fail-closed 拒绝。');
  }
  process.exit(0);
});
