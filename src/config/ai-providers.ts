import type { AiProvider } from "@/types/daily-report";

export interface AiProviderDetail {
  label: string;
  models: string[];
}

/** Git 报告可用的 AI Provider 与模型（设置页与日报页共用）。 */
export const AI_PROVIDERS: Record<AiProvider, AiProviderDetail> = {
  minimax: { label: "MiniMax", models: ["MiniMax-M2.7", "abab6.5s-chat"] },
  kimi: { label: "Kimi", models: ["moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"] },
  deepseek: { label: "DeepSeek", models: ["deepseek-chat", "deepseek-reasoner"] },
};

export const AI_PROVIDER_OPTIONS = (
  Object.entries(AI_PROVIDERS) as [AiProvider, AiProviderDetail][]
).map(([value, detail]) => ({ value, label: detail.label }));

export function aiModelOptions(
  provider: AiProvider,
  customModels: readonly string[] = [],
  selectedModel?: string,
): { value: string; label: string }[] {
  const builtIn = AI_PROVIDERS[provider].models;
  const models = [...builtIn, ...customModels];
  if (selectedModel) models.push(selectedModel);
  return [...new Set(models.map((model) => model.trim()).filter(Boolean))].map((model) => ({
    value: model,
    label: builtIn.includes(model) ? model : `${model}（自定义）`,
  }));
}
