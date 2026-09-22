import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyPlanRequest,
  ApplyResult,
  CatalogFilter,
  CatalogSource,
  McpAgentInfo,
  McpPackage,
  McpPlan,
  McpScanResult,
  McpServer,
  ParsedServer,
  PlanRequest,
  ProbeResult,
  ReadmeResult,
  SourceSaveRequest,
} from "@/types/mcp";

/** 通用「已移除」响应。 */
export interface Removed {
  removed: boolean;
}

/** 把 invoke 抛出的错误（字符串或 McpError 对象）归一为可读消息。 */
export function mcpErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(mcpErrorMessage(error));
  }
}

/**
 * MCP 命令客户端。桌面端通过 Tauri invoke；浏览器模式无此能力（入口隐藏）。
 */
export const mcpApi = {
  // ---------- 服务 ----------
  listAgents: () => call<McpAgentInfo[]>("mcp_agents"),
  scan: (projects: string[] = []) => call<McpScanResult>("mcp_scan", { projects }),
  plan: (request: PlanRequest) => call<McpPlan>("mcp_plan", { request }),
  apply: (id: string, request: ApplyPlanRequest) =>
    call<ApplyResult>("mcp_apply", { id, request }),
  probe: (server: McpServer) => call<ProbeResult>("mcp_probe", { server }),
  restoreBackup: (backupDir: string) =>
    call<Removed>("mcp_restore_backup", { backupDir }),
  setMeta: (key: string, note: string, tags: string[]) =>
    call<Removed>("mcp_set_meta", { key, note, tags }),
  parsePaste: (text: string) => call<ParsedServer[]>("mcp_parse_paste", { text }),

  // ---------- 库 ----------
  listSources: () => call<CatalogSource[]>("mcp_catalog_sources"),
  sync: (sourceId?: string) =>
    call<CatalogSource[]>("mcp_catalog_sync", { sourceId: sourceId ?? null }),
  listPackages: (filter: CatalogFilter, projects: string[] = []) =>
    call<McpPackage[]>("mcp_catalog_list", { filter, projects }),
  getPackage: (id: string) => call<McpPackage | null>("mcp_catalog_detail", { id }),
  getReadme: (id: string) => call<ReadmeResult>("mcp_catalog_readme", { id }),
  saveSource: (request: SourceSaveRequest) =>
    call<CatalogSource>("mcp_catalog_source_save", { request }),
  removeSource: (id: string) => call<Removed>("mcp_catalog_source_remove", { id }),
  favorite: (id: string, value: boolean) =>
    call<Removed>("mcp_catalog_favorite", { id, value }),
};
