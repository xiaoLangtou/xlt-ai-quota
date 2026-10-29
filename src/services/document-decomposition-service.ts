import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { isTauriDesktop } from "@/connectors/types";
import { AI_PROVIDERS } from "@/config/ai-providers";
import { dailyReportService } from "@/services/daily-report-service";
import { today } from "@/utils/productivity";
import type {
  AiProvider,
} from "@/types/daily-report";
import type {
  CommitInput,
  CommitResult,
  DecompositionRequest,
  DecompositionResult,
  DocumentContent,
} from "@/types/productivity";

export const SUPPORTED_DOCUMENT_EXTENSIONS = ["md", "markdown", "txt", "docx", "pdf"] as const;

/** AI 配置缺失：文档拆解复用 Git 报告保存的服务商 / 模型 / API Key。 */
export class AiConfigMissingError extends Error {
  constructor() {
    super("请先在「设置 › 高级」中完成 AI 配置");
    this.name = "AiConfigMissingError";
  }
}

export interface DecompositionAiConfig {
  provider: AiProvider;
  model: string;
  apiKey: string;
}

function requireDesktop(): void {
  if (!isTauriDesktop()) throw new Error("文档拆解需要在 Tauri 桌面应用中使用。");
}

/**
 * 读取 Git 报告保存的 AI 配置。仅在调用期间使用，不写入 Pinia、URL 或日志。
 * 缺少服务商、模型或密钥时返回 null。
 */
export function readAiConfig(): DecompositionAiConfig | null {
  const preferences = dailyReportService.loadPreferences();
  const provider = preferences.ai?.provider;
  if (!provider || !AI_PROVIDERS[provider]) return null;
  const apiKey = preferences.aiApiKeys?.[provider]?.trim();
  if (!apiKey) return null;
  const model = preferences.ai?.model?.trim() || AI_PROVIDERS[provider].models[0];
  if (!model) return null;
  return { provider, model, apiKey };
}

export function documentExtension(name: string): string {
  const index = name.lastIndexOf(".");
  return index >= 0 ? name.slice(index + 1).toLowerCase() : "";
}

export function isSupportedDocument(name: string): boolean {
  return (SUPPORTED_DOCUMENT_EXTENSIONS as readonly string[]).includes(documentExtension(name));
}

export const documentDecompositionService = {
  readAiConfig,

  /** 打开系统文件选择器；用户取消时返回 null。 */
  async pickDocument(): Promise<DocumentContent | null> {
    requireDesktop();
    const selected = await open({
      multiple: false,
      directory: false,
      title: "选择要拆解的文档",
      filters: [{ name: "支持文档", extensions: [...SUPPORTED_DOCUMENT_EXTENSIONS] }],
    });
    if (typeof selected !== "string" || !selected) return null;
    return documentDecompositionService.readDocument(selected);
  },

  readDocument(path: string): Promise<DocumentContent> {
    requireDesktop();
    return invoke<DocumentContent>("document_read", { path });
  },

  /** 点击「开始拆解」时读取最新 AI 配置，再调用后端生成结构化草稿。 */
  generate(input: { sourceName: string; content: string }): Promise<DecompositionResult> {
    requireDesktop();
    const config = readAiConfig();
    if (!config) throw new AiConfigMissingError();
    const request: DecompositionRequest = {
      sourceName: input.sourceName,
      content: input.content,
      provider: config.provider,
      model: config.model,
      apiKey: config.apiKey,
      currentDate: today(),
    };
    return invoke<DecompositionResult>("document_decomposition_generate", { input: request });
  },

  commit(input: CommitInput): Promise<CommitResult> {
    requireDesktop();
    return invoke<CommitResult>("document_decomposition_commit", { input });
  },
};
