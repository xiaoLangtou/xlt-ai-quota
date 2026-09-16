export interface GitProject {
  path: string;
  name: string;
  branch: string;
}

export interface GitCommit {
  hash: string;
  message: string;
  date: string;
}

export interface GitProjectCommits {
  project: GitProject;
  commits: GitCommit[];
}

export type AiProvider = "minimax" | "kimi" | "deepseek";
export type ReportKind = "daily" | "weekly";

export interface AiProviderPreference {
  provider: AiProvider;
  model: string;
}

export interface DailyReportPreferences {
  projects: GitProject[];
  authors: string[];
  ai?: AiProviderPreference;
  aiApiKeys?: Partial<Record<AiProvider, string>>;
}

export interface GitReportImport {
  projects: GitProject[];
  authors: string[];
}
