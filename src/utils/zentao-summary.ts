import type { TodoTask, ZentaoDetail } from "@/types/productivity";

export interface SummarySource { id: string; text: string; label: string }
export interface SummaryStatement { text: string; sources: string[] }
export interface ZentaoSummaryResult {
  overview: SummaryStatement[]; purpose: SummaryStatement[]; frontend: SummaryStatement[]; backend: SummaryStatement[];
}

export function summaryImages(detail: ZentaoDetail): string[] {
  return [...new Set([
    ...detail.images,
    ...detail.actions.flatMap(action => [...action.images, ...action.changes.flatMap(change => change.newImages)]),
  ])];
}

/** 文字与材料清单绑定输入版本，图片数据也进入指纹。 */
export function summarySources(task: TodoTask, detail: ZentaoDetail): SummarySource[] {
  const sources: SummarySource[] = [
    { id: "current", label: "当前事项", text: `编号：${detail.externalType}:${detail.externalId}\n标题：${task.title}\n状态：${detail.status}\n指派：${detail.assignedTo}\n优先级：${task.priority}\n截止：${task.externalDue ?? "未提供"}` },
    { id: "description", label: "描述", text: detail.description.slice(0, 12000) },
  ];
  const start = Math.max(0, detail.actions.length - 30);
  detail.actions.slice(start).forEach((action, index) => {
    const changes = action.changes.map(change => `${change.field}: ${change.old} → ${change.new}`).join("\n");
    sources.push({ id: `action-${start + index}`, label: `${action.date} ${action.actor}`, text: `${action.date}\n${action.actor}\n${action.desc}\n${action.comment}\n${changes}`.slice(0, 850) });
  });
  summaryImages(detail).forEach((image, index) => sources.push({ id: `image-${index}`, label: `图片 ${index + 1}`, text: image }));
  detail.files.forEach(file => sources.push({ id: `file-${file.id}`, label: `附件：${file.title}`, text: JSON.stringify({ id: file.id, title: file.title, extension: file.extension, size: file.size }) }));
  return sources;
}

export async function summaryFingerprint(sources: SummarySource[]): Promise<string> {
  const data = new TextEncoder().encode(JSON.stringify(sources));
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, "0")).join("");
}

export function validSummary(value: unknown, sources: SummarySource[]): value is ZentaoSummaryResult {
  if (!value || typeof value !== "object") return false;
  const result = value as Record<string, unknown>;
  const ids = new Set(sources.map(source => source.id));
  return ["overview", "purpose", "frontend", "backend"].every(key => {
    const section = result[key];
    return Array.isArray(section) && section.length <= 3 && (key !== "overview" || section.length > 0) && section.every(item =>
      item && typeof item.text === "string" && item.text.trim().length > 0 && Array.from(item.text).length <= 240
      && Array.isArray(item.sources) && item.sources.length > 0 && item.sources.length <= 3
      && item.sources.every((id: unknown) => typeof id === "string" && ids.has(id)));
  });
}
