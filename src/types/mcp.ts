/**
 * MCP 领域类型。与 `src-tauri/src/mcp/types.rs` 的序列化保持一致（camelCase）。
 */

export type McpTransport = "stdio" | "http" | "sse";

/** 归一化后的服务定义。 */
export interface McpServer {
  name: string;
  transport: McpTransport;
  command?: string;
  args: string[];
  env: Record<string, string>;
  url?: string;
  headers: Record<string, string>;
  /** 未识别字段，原样保留。 */
  extra: Record<string, unknown>;
}

export type InstanceStatus = "ok" | "disabled" | "config_error" | "command_missing" | "unchecked";

/** 服务在某 Agent / 范围下的落点。 */
export interface McpInstance {
  agent: string;
  agentLabel: string;
  scope: "global" | "project";
  projectPath?: string;
  configFile: string;
  enabled: boolean;
  status: InstanceStatus;
  error?: string;
}

/** 合并后的服务（列表行）。 */
export interface McpService {
  key: string;
  name: string;
  transport: McpTransport;
  command?: string;
  args: string[];
  env: Record<string, string>;
  url?: string;
  headers: Record<string, string>;
  instances: McpInstance[];
  agents: string[];
  scopes: string[];
  enabled: boolean;
  status: InstanceStatus;
  note: string;
  tags: string[];
  sourceId?: string;
}

export interface McpScanIssue {
  agent: string;
  agentLabel: string;
  configFile: string;
  message: string;
}

export interface McpScanResult {
  services: McpService[];
  issues: McpScanIssue[];
  scannedFiles: string[];
}

export interface McpAgentInfo {
  id: string;
  label: string;
  supportsDisable: boolean;
  supportsProject: boolean;
  supportsRemote: boolean;
  globalPath?: string;
  available: boolean;
}

export type PlanKind = "upsert" | "remove" | "enable" | "disable";

export interface WriteTarget {
  agent: string;
  scope: "global" | "project";
  projectPath?: string;
}

export interface PlanRequest {
  kind: PlanKind;
  server?: McpServer;
  name: string;
  targets: WriteTarget[];
}

export interface TargetPlan {
  agent: string;
  agentLabel: string;
  scope: "global" | "project";
  projectPath?: string;
  configFile: string;
  exists: boolean;
  before: string;
  after: string;
  diff: string;
  warnings: string[];
}

export interface McpPlan {
  id: string;
  summary: string;
  targets: TargetPlan[];
  warnings: string[];
}

export interface ApplyPlanRequest {
  expectedMtimes: Record<string, number>;
}

export interface WrittenFile {
  configFile: string;
  backupDir: string;
}

export interface ApplyResult {
  written: WrittenFile[];
  rolledBack: boolean;
  error?: string;
}

export interface ProbeTool {
  name: string;
  description?: string;
}

export interface ProbeResult {
  ok: boolean;
  transport: string;
  tools: ProbeTool[];
  serverInfo?: string;
  error?: string;
  durationMs: number;
}

export interface ParsedServer {
  server: McpServer;
  source: string;
}

// ---------- MCP 库 ----------

export interface Runtime {
  type: "npx" | "uvx" | "pip" | "docker" | "remote" | string;
  command?: string;
  args?: string[];
  url?: string;
  version?: string;
}

export interface EnvVar {
  name: string;
  description?: string;
  required: boolean;
  secret: boolean;
}

export type InstallInfo =
  | { level: "ready"; runtimes: Runtime[]; envSpec: EnvVar[] }
  | { level: "manual" };

export interface LanguageInfo {
  name: string;
  color?: string;
}

export interface McpPackage {
  id: string;
  name: string;
  description: string;
  source: string;
  homepage: string;
  repo?: string;
  tags: string[];
  language?: LanguageInfo;
  license?: string;
  platforms: string[];
  stars?: number;
  forks?: number;
  updatedAt?: string;
  verified: boolean;
  mirror: boolean;
  kind: "server" | "client" | "other";
  install: InstallInfo;
  installedAgents: string[];
  favorite: boolean;
}

export interface CatalogSource {
  id: string;
  label: string;
  kind: "builtin" | "registry" | "gitcode" | "custom" | string;
  url?: string;
  channelId?: string;
  subChannelId?: string;
  enabled: boolean;
  builtin: boolean;
  lastSyncAt?: string;
  lastError?: string;
}

export interface CatalogFilter {
  source?: string;
  query?: string;
  sort?: "all" | "star" | "updated" | "favorites";
  kind?: string;
  runtime?: string;
  showClients?: boolean;
  favoritesOnly?: boolean;
  limit?: number;
  offset?: number;
}

export interface ReadmeResult {
  available: boolean;
  text: string;
  snippets: ParsedServer[];
  sourceUrl?: string;
  error?: string;
}

export interface SourceSaveRequest {
  id?: string;
  label: string;
  kind?: string;
  url?: string;
  channelId?: string;
  subChannelId?: string;
  enabled: boolean;
}
