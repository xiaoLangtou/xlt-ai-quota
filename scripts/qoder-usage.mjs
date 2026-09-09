import { qodercliAuth, query } from "@qoder-ai/qoder-agent-sdk";

let releaseInput;
async function* idlePrompt() {
  await new Promise((resolve) => {
    releaseInput = resolve;
  });
}

const session = query({
  prompt: idlePrompt(),
  options: {
    auth: qodercliAuth(),
    cwd: process.cwd(),
  },
});

try {
  await session.initializationResult();
  const usage = await session.getUsageInfo();
  const quota = usage?.userQuota;
  if (!quota || !Number.isFinite(quota.used) || !Number.isFinite(quota.total)) {
    throw new Error("Qoder 未返回套餐 Credits，请确认 qodercli 已登录且为最新版本");
  }
  const plans = {
    personal_free: "Free",
    personal_professional: "Pro",
    personal_teams: "Teams",
  };
  console.log(JSON.stringify({
    used: quota.used,
    total: quota.total,
    remaining: Number.isFinite(quota.remaining) ? quota.remaining : quota.total - quota.used,
    planTag: plans[usage.userType] ?? usage.userType,
    expiresAt: typeof usage.expiresAt === "number" ? new Date(usage.expiresAt).toISOString() : undefined,
    addOnUsed: Number.isFinite(usage.addOnQuota?.used) ? usage.addOnQuota.used : undefined,
    addOnTotal: Number.isFinite(usage.addOnQuota?.total) ? usage.addOnQuota.total : undefined,
  }));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
} finally {
  releaseInput?.();
  await session.close().catch(() => undefined);
}
