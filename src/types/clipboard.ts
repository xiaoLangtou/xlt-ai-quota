export const CLIPBOARD_KINDS = ["text", "image", "files"] as const;

export type ClipboardKind = (typeof CLIPBOARD_KINDS)[number];
export type ClipboardScope = "all" | ClipboardKind | "pinned";

export interface ClipboardItem {
  id: string;
  kind: ClipboardKind;
  content: string;
  hash: string;
  sourceApp: string | null;
  pinned: boolean;
  copyCount: number;
  sizeBytes: number;
  imageWidth: number | null;
  imageHeight: number | null;
  createdAt: string;
  updatedAt: string;
}

export type ClipboardFilePreviewKind = "text" | "image" | "pdf" | "unsupported";

export interface ClipboardFilePreview {
  path: string;
  name: string;
  extension: string | null;
  kind: ClipboardFilePreviewKind;
  sizeBytes: number;
  content: string | null;
  dataUrl: string | null;
  message: string | null;
}

export interface ClipboardQuery {
  query?: string | null;
  kind?: ClipboardKind | null;
  pinnedOnly?: boolean;
  limit?: number;
}

export interface ClipboardSettings {
  enabled: boolean;
  launchAtLogin: boolean;
  maxItems: number;
  ttlDays: number;
  shortcut: string;
  excludedApps: string[];
}

export interface ClipboardStatus {
  settings: ClipboardSettings;
  total: number;
  pinned: number;
  textCount: number;
  imageCount: number;
  fileCount: number;
  accessibilityGranted: boolean;
  accessibilityTarget: string;
  storagePath: string;
}

export interface ClipboardSettingsDraft {
  enabled: boolean;
  launchAtLogin: boolean;
  maxItems: number;
  ttlDays: number;
  shortcut: string;
  excludedApps: string[];
}

export interface ClipboardSequenceStatus {
  active: boolean;
  total: number;
  nextIndex: number;
  remaining: number;
}

export interface ClipboardSequenceStep {
  item: ClipboardItem;
  status: ClipboardSequenceStatus;
}
