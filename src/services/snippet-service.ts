import { invoke } from "@tauri-apps/api/core";
import type { Snippet, SnippetDraft, SnippetFilter, SnippetTag } from "@/types/snippet";

export const snippetService = {
  list(filter: SnippetFilter): Promise<Snippet[]> {
    return invoke<Snippet[]>("snippet_list", { filter });
  },

  listTags(): Promise<SnippetTag[]> {
    return invoke<SnippetTag[]>("snippet_list_tags");
  },

  save(draft: SnippetDraft): Promise<Snippet> {
    return invoke<Snippet>("snippet_save", { draft });
  },

  delete(id: string): Promise<void> {
    return invoke<void>("snippet_delete", { id });
  },

  touch(id: string): Promise<Snippet> {
    return invoke<Snippet>("snippet_touch", { id });
  },

  copy(id: string): Promise<Snippet> {
    return invoke<Snippet>("snippet_copy", { id });
  },
};
