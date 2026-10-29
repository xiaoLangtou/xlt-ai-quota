import { qodercliAuth, query } from "@qoder-ai/qoder-agent-sdk";

export async function fetchQoderUsage(createQuery = query) {
  let releaseInput;
  let authExpired = false;
  async function* idlePrompt() {
    await new Promise((resolve) => { releaseInput = resolve; });
  }
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 30_000);
  const session = createQuery({
    prompt: idlePrompt(),
    options: {
      auth: qodercliAuth(), cwd: process.cwd(), abortController: controller,
      onAuthExpired: () => { authExpired = true; },
    },
  });
  try {
    await session.initializationResult();
    const usage = await session.getUsageInfo();
    if (authExpired) throw new Error("Qoder 登录已失效，请在设置页点击「登录 Qoder」重新授权");
    const quota = usage?.userQuota;
    if (!quota || !Number.isFinite(quota.used) || !Number.isFinite(quota.total)) {
      throw new Error("Qoder 未返回套餐 Credits，请在设置页重新登录 Qoder 后同步");
    }
    const plans = { personal_free: "Free", personal_professional: "Pro", personal_teams: "Teams" };
    return {
      used: quota.used, total: quota.total,
      remaining: quota.remaining,
      planTag: plans[usage.userType] ?? usage.userType,
      expiresAt: typeof usage.expiresAt === "number" ? new Date(usage.expiresAt).toISOString() : undefined,
      addOnUsed: usage.addOnQuota?.used, addOnTotal: usage.addOnQuota?.total,
    };
  } catch (error) {
    if (authExpired) throw new Error("Qoder 登录已失效，请在设置页点击「登录 Qoder」重新授权");
    if (controller.signal.aborted) throw new Error("Qoder 额度查询超过 30 秒，已终止");
    throw error;
  } finally {
    clearTimeout(timer);
    releaseInput?.();
    await session.close();
  }
}
