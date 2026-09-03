// 一键端到端：启动 Vite dev server -> 等就绪 -> 跑 e2e + 单元冒烟 -> 关停
// 依赖本机已安装并登录 arkcli / opencode / kiro-cli / codex
import { spawn } from "node:child_process";

const ORIGIN = "http://localhost:1420";
const E2E = ["smoke-codex-e2e", "smoke-kiro-e2e", "smoke-opencode-e2e", "smoke-sync-e2e"];
const UNIT = ["smoke-data", "smoke-ark-mapping", "smoke-settings"];

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function waitUntilUp(timeoutMs = 40000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const r = await fetch(ORIGIN + "/");
      if (r.ok) return true;
    } catch {
      // 还没起来
    }
    await sleep(400);
  }
  return false;
}

function runTest(name) {
  return new Promise((resolve) => {
    const p = spawn("pnpm", ["exec", "tsx", `scripts/${name}.mts`], {
      stdio: "inherit",
      env: process.env,
    });
    p.on("close", (code) => resolve({ name, ok: code === 0 }));
  });
}

const dev = spawn("pnpm", ["dev"], { stdio: "ignore", detached: true, env: process.env });
dev.unref();

const results = [];
let failed = false;

try {
  process.stdout.write("等待 dev server 起来…");
  const up = await waitUntilUp();
  if (!up) {
    console.error("\n❌ dev server 未在 40s 内就绪");
    process.exit(1);
  }
  console.log(" 就绪\n");

  process.stdout.write("--- 单元冒烟（无需 dev server）---\n");
  for (const t of UNIT) {
    const r = await runTest(t);
    results.push(r);
    if (!r.ok) failed = true;
  }

  process.stdout.write("\n--- e2e（经 dev 中间件调用本机 CLI）---\n");
  for (const t of E2E) {
    const r = await runTest(t);
    results.push(r);
    if (!r.ok) failed = true;
  }
} finally {
  try {
    process.kill(-dev.pid, "SIGTERM");
  } catch {
    try {
      dev.kill("SIGTERM");
    } catch {
      /* ignore */
    }
  }
}

console.log("\n=== 汇总 ===");
for (const r of results) console.log(`${r.ok ? "PASS" : "FAIL"}  ${r.name}`);
console.log(failed ? "\n存在失败 ❌" : "\n全部通过 ✅");
process.exit(failed ? 1 : 0);
