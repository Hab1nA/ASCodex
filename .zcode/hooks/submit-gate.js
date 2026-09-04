#!/usr/bin/env node
// ascodex 提交门 — ZCode PreToolUse 钩子（仓库 ASCLocal-Codex 专用）。
//
// 职责（ASCodex solver-guard 六道门在单会话模式的等价硬拦截）：
// 1. 拦截针对 play.bohrium.com 的写命令 / submit_bundle.py 上传调用：
//    仅当 work/<slug>/.submit-authorized 存在时放行。文件由用户手工创建，
//    首个非空行 = 本次会话剩余允许提交次数；每次放行扣 1，扣尽即拦。
//    work/ 下必须恰好一个授权文件（0 个 = 未授权，多个 = 状态含糊，都拒）。
// 2. 放行不消耗次数：显式 --dry-run / dry_run=1，以及只读审计脚本
//    bohrium-kb/tools/submit_gate_audit.py。
// 3. 防自授权篡改：模型侧命令/写文件一旦触及 .submit-authorized 一律拒绝
//    （授权文件只能由用户在会话外手工创建/修改）。
//
// 退出码：0 放行（静默）；2 拒绝（stderr 给原因）；其他 = 脚本错误。
// 只读 GET（无 POST 意图）不进入本门，开题侦察不受影响。

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

function submitIntent(cmd) {
  const postIntent = /(-x\s*post|-xpost|--request\s+post|--data(?:-raw|-binary|-urlencode)?|--form|-f\s|--upload-file|-t\s|--post-data|--post-file|requests\.(?:post|put|request)|invoke-restmethod|invoke-webrequest|http\s+post)/.test(cmd);
  const host = /play\.bohrium\.com/.test(cmd);
  const attemptsApi = /\/api\/(?:attempts|challenges\/[^'"\s]*\/attempts)/.test(cmd);
  const uploader = /\bpython3?(?:\.exe)?\s+"?[^'"&|;]*submit_bundle\.py"?/.test(cmd) ||
                   /(?:^|[&;|(]\s*)submit_bundle\.py\b/.test(cmd);
  return (host && postIntent) || (attemptsApi && postIntent) || uploader;
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

  // Write/Edit 直写授权文件已被上一条覆盖（路径出现在 file_path）。
  if (tool !== 'Bash' && tool !== 'bash') process.exit(0);
  if (!cmd) process.exit(0);

  if (!submitIntent(lower)) process.exit(0);

  // 不消耗次数的放行
  if (/--dry-run|dry_run\s*=\s*1|dryrun/.test(lower)) process.exit(0);
  if (/submit_gate_audit\.py/.test(lower) && !submitIntent(lower.replace(/.*submit_gate_audit\.py/, ''))) process.exit(0);

  const projectDir = findProjectDir(ev);
  const workDir = path.join(projectDir, 'work');
  let authFiles = [];
  try {
    authFiles = fs.readdirSync(workDir)
      .map((name) => path.join(workDir, name, '.submit-authorized'))
      .filter((p) => { try { return fs.statSync(p).isFile(); } catch (e) { return false; } });
  } catch (e) {
    deny('work/ 目录不可读（' + e.message + '）。');
  }

  if (authFiles.length === 0) {
    deny('未找到本次会话授权。真实提交前由用户手工创建 work/<slug>/.submit-authorized，内容为本次允许提交次数（如 3）。当前仅允许 dry-run 与只读审计。');
  }
  if (authFiles.length > 1) {
    deny('work/ 下存在多个 .submit-authorized（' + authFiles.join(', ') + '），状态含糊——恰一原则，拒绝。请用户清理后重试。');
  }

  const authPath = authFiles[0];
  let text = '';
  try { text = fs.readFileSync(authPath, 'utf8'); } catch (e) {
    deny('授权文件不可读（' + e.message + '）。');
  }
  const firstLine = text.split(/\r?\n/).map((s) => s.trim()).find((s) => s.length > 0) || '0';
  const remaining = parseInt(firstLine, 10);
  if (!Number.isInteger(remaining) || remaining < 1) {
    fs.unlinkSync(authPath);
    deny('授权次数已用尽，授权文件已移除。需要继续提交请让用户重新创建 .submit-authorized。');
  }

  try {
    if (remaining - 1 >= 1) {
      fs.writeFileSync(authPath, String(remaining - 1) + '\n', 'utf8');
    } else {
      fs.unlinkSync(authPath);
    }
  } catch (e) {
    deny('扣减授权次数失败（' + e.message + '），按 fail-closed 拒绝。');
  }
  process.stderr.write('[submit-gate] ALLOW: 消耗 1 次提交授权，剩余 ' + (remaining - 1) + '（' + authPath + '）\n');
  process.exit(0);
});
