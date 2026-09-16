import { invoke } from "@tauri-apps/api/core";
import { isTauriDesktop } from "@/connectors/types";
import type {
  AiProvider,
  DailyReportPreferences,
  GitProject,
  GitProjectCommits,
  GitReportImport,
  ReportKind,
} from "@/types/daily-report";

const PREFERENCES_KEY = "xlt.daily-report.preferences";

function readPreferences(): DailyReportPreferences {
  const raw = localStorage.getItem(PREFERENCES_KEY);
  if (!raw) return { projects: [], authors: [] };
  const parsed = JSON.parse(raw) as Partial<DailyReportPreferences>;
  return {
    projects: Array.isArray(parsed.projects) ? parsed.projects : [],
    authors: Array.isArray(parsed.authors) ? parsed.authors : [],
    ai: parsed.ai,
    aiApiKeys: parsed.aiApiKeys,
  };
}

function requireDesktop(): void {
  if (!isTauriDesktop()) throw new Error("Git 日报功能需要在 Tauri 桌面应用中使用。");
}

export const dailyReportService = {
  loadPreferences(): DailyReportPreferences {
    return readPreferences();
  },

  savePreferences(preferences: DailyReportPreferences): void {
    localStorage.setItem(PREFERENCES_KEY, JSON.stringify(preferences));
  },

  async validateProject(path: string): Promise<GitProject> {
    requireDesktop();
    return invoke<GitProject>("daily_report_validate_project", { path });
  },

  async collectCommits(input: {
    projects: GitProject[];
    authors: string[];
    startDate: string;
    endDate: string;
  }): Promise<GitProjectCommits[]> {
    requireDesktop();
    return invoke<GitProjectCommits[]>("daily_report_collect_commits", { input });
  },

  async generateReport(input: {
    provider: AiProvider;
    model: string;
    apiKey: string;
    reportType: ReportKind;
    startDate: string;
    endDate: string;
    projects: GitProjectCommits[];
  }): Promise<string> {
    requireDesktop();
    return invoke<string>("daily_report_generate", { input });
  },

  async importGitReportConfig(): Promise<GitReportImport> {
    requireDesktop();
    return invoke<GitReportImport>("daily_report_import_gitreports");
  },
};
