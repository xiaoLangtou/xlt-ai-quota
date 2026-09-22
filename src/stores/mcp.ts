import { computed, ref } from "vue";
import { defineStore } from "pinia";
import type { McpAgentInfo, McpScanIssue, McpService } from "@/types/mcp";
import { mcpApi, mcpErrorMessage } from "@/api/mcp";

export const useMcpStore = defineStore("mcp", () => {
  const services = ref<McpService[]>([]);
  const issues = ref<McpScanIssue[]>([]);
  const scannedFiles = ref<string[]>([]);
  const agents = ref<McpAgentInfo[]>([]);
  const loading = ref(false);
  const agentsLoaded = ref(false);
  const scannedAt = ref("");
  const error = ref("");

  /** 配置层异常数：文件级问题 + 配置错误 + 命令不存在。 */
  const errorCount = computed(
    () =>
      issues.value.length +
      services.value.filter(
        (service) => service.status === "config_error" || service.status === "command_missing",
      ).length,
  );

  async function loadAgents(): Promise<void> {
    if (agentsLoaded.value) return;
    try {
      agents.value = await mcpApi.listAgents();
      agentsLoaded.value = true;
    } catch (caught) {
      error.value = mcpErrorMessage(caught);
    }
  }

  async function refresh(projects: string[] = []): Promise<void> {
    loading.value = true;
    error.value = "";
    try {
      const result = await mcpApi.scan(projects);
      services.value = result.services;
      issues.value = result.issues;
      scannedFiles.value = result.scannedFiles;
      scannedAt.value = new Date().toISOString();
    } catch (caught) {
      error.value = mcpErrorMessage(caught);
    } finally {
      loading.value = false;
    }
  }

  return {
    services,
    issues,
    scannedFiles,
    agents,
    loading,
    error,
    scannedAt,
    errorCount,
    loadAgents,
    refresh,
  };
});

/** 侧边栏徽章：只统计配置错误 + 命令不存在。 */
export function mcpIssueCount(): number {
  return useMcpStore().errorCount;
}
