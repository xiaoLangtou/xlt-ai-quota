export const SNIPPET_KINDS = ["code", "command", "prompt", "text", "link"] as const;

export type SnippetKind = (typeof SNIPPET_KINDS)[number];
export type SnippetSort = "recent" | "updated" | "created" | "title" | "usage";

export interface Snippet {
  id: string;
  title: string;
  kind: SnippetKind;
  content: string;
  language: string | null;
  description: string;
  tags: string[];
  pinned: boolean;
  useCount: number;
  lastUsedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SnippetDraft {
  id?: string;
  title: string;
  kind: SnippetKind;
  content: string;
  language: string | null;
  description: string;
  tags: string[];
  pinned: boolean;
}

export interface SnippetFilter {
  query?: string | null;
  kind?: SnippetKind | null;
  tag?: string | null;
  sort: SnippetSort;
}

export interface SnippetTag {
  name: string;
  count: number;
}
