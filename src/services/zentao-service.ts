import { invoke } from "@tauri-apps/api/core";
import { isTauriDesktop } from "@/connectors/types";
import type {
  ZentaoActionContext,
  ZentaoActionInput,
  ZentaoActionResult,
  ZentaoArchiveEntry,
  ZentaoBatchResolveInput,
  ZentaoBatchResolveResult,
  ZentaoDetail,
  ZentaoFileContent,
  ZentaoStatus,
  ZentaoSyncResult,
} from "@/types/productivity";

function requireDesktop(): void {
  if (!isTauriDesktop()) throw new Error("禅道同步需要在 Tauri 桌面应用中使用。");
}

/** 缓存的详情：detail + 拉取时间。图片以 data URL 存在，故限制条数避免内存膨胀。 */
export interface CachedZentaoDetail {
  detail: ZentaoDetail;
  fetchedAt: number;
}

const detailCache = new Map<string, CachedZentaoDetail>();
const DETAIL_CACHE_LIMIT = 20;

function cacheKey(externalType: string, externalId: number): string {
  return `${externalType}:${externalId}`;
}

/** 清空详情缓存（同步后数据可能变化时调用）。 */
export function clearZentaoDetailCache(): void {
  detailCache.clear();
}

export const zentaoService = {
  async comment(taskId: string, comment: string, password: string): Promise<void> {
    requireDesktop();
    try { await invoke("zentao_comment", { input: { taskId, comment, password } }); }
    finally { clearZentaoDetailCache(); }
  },
  actionContext(taskId: string, operation: string): Promise<ZentaoActionContext> {
    requireDesktop();
    return invoke("zentao_action_context", { taskId, operation });
  },
  async action(input: ZentaoActionInput): Promise<ZentaoActionResult> {
    requireDesktop();
    try {
      return await invoke("zentao_action", { input });
    } finally {
      clearZentaoDetailCache();
    }
  },
  /** 批量解决 Bug（逐条提交、回读核实；单条失败不影响其他条目）。 */
  async batchResolve(input: ZentaoBatchResolveInput): Promise<ZentaoBatchResolveResult> {
    requireDesktop();
    try {
      return await invoke("zentao_batch_resolve", { input });
    } finally {
      clearZentaoDetailCache();
    }
  },
  status(): Promise<ZentaoStatus> {
    requireDesktop();
    return invoke<ZentaoStatus>("zentao_status");
  },

  /** 登录并写入本机 token 缓存；密码只在本次调用中使用，不落应用存储。 */
  login(url: string, account: string, password: string): Promise<ZentaoStatus> {
    requireDesktop();
    return invoke<ZentaoStatus>("zentao_login", { url, account, password });
  },

  logout(): Promise<void> {
    requireDesktop();
    return invoke<void>("zentao_logout");
  },

  sync(): Promise<ZentaoSyncResult> {
    requireDesktop();
    return invoke<ZentaoSyncResult>("zentao_sync");
  },

  /**
   * 读取任务/Bug 详情。默认走内存缓存（同一会话内重复打开秒开）；
   * force=true 时重新从禅道拉取并更新缓存。
   */
  detail(externalType: string, externalId: number, options?: { force?: boolean }): Promise<CachedZentaoDetail> {
    requireDesktop();
    const key = cacheKey(externalType, externalId);
    if (!options?.force) {
      const hit = detailCache.get(key);
      if (hit) {
        // 命中后移到队尾，保持 LRU 顺序。
        detailCache.delete(key);
        detailCache.set(key, hit);
        return Promise.resolve(hit);
      }
    }
    return invoke<ZentaoDetail>("zentao_task_detail", { externalType, externalId }).then((detail) => {
      const entry = { detail, fetchedAt: Date.now() };
      detailCache.delete(key);
      detailCache.set(key, entry);
      while (detailCache.size > DETAIL_CACHE_LIMIT) {
        const oldest = detailCache.keys().next().value;
        if (oldest === undefined) break;
        detailCache.delete(oldest);
      }
      return entry;
    });
  },

  /** 下载附件到本机下载目录，返回保存路径。 */
  downloadFile(fileId: number, extension: string, title: string): Promise<string> {
    requireDesktop();
    return invoke<string>("zentao_download_file", { fileId, extension, title });
  },

  /** 读取附件内容用于预览（文本类 / 图片类）。 */
  readFile(fileId: number, extension: string): Promise<ZentaoFileContent> {
    requireDesktop();
    return invoke<ZentaoFileContent>("zentao_read_file", { fileId, extension });
  },

  /** 读取 zip 附件的文件条目列表（目录树由前端构建）。 */
  readArchive(fileId: number): Promise<ZentaoArchiveEntry[]> {
    requireDesktop();
    return invoke<ZentaoArchiveEntry[]>("zentao_read_archive", { fileId });
  },

  /** 读取 zip 附件内单个条目内容（按条目索引，文本类 / 图片类）。 */
  readArchiveEntry(fileId: number, index: number): Promise<ZentaoFileContent> {
    requireDesktop();
    return invoke<ZentaoFileContent>("zentao_read_archive_entry", { fileId, index });
  },
};

/** 经系统默认程序打开外部链接（Tauri 内 target=_blank 不可用）。 */
export function openExternal(url: string): Promise<void> {
  if (!url) return Promise.resolve();
  if (isTauriDesktop()) {
    return invoke("open_external", { url }).then(() => undefined);
  }
  window.open(url, "_blank", "noreferrer");
  return Promise.resolve();
}
