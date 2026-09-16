export type VaultRecordType = "credential" | "ai_key" | "other_key";

export interface VaultRecord {
  id: string;
  type: VaultRecordType;
  name: string;
  account: string;
  provider: string;
  secret: string;
  loginUrl: string;
  baseUrl: string;
  note: string;
  tags: string[];
  favorite: boolean;
  createdAt: string;
  updatedAt: string;
  lastUsedAt: string | null;
}

export type VaultRecordDraft = Pick<
  VaultRecord,
  | "type"
  | "name"
  | "account"
  | "provider"
  | "secret"
  | "loginUrl"
  | "baseUrl"
  | "note"
  | "tags"
  | "favorite"
> & { id?: string };

export type VaultSort = "recent" | "name" | "created";

export interface VaultPayload {
  records: VaultRecord[];
}
