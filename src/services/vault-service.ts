import { z } from "zod";
import { invoke } from "@tauri-apps/api/core";
import type { VaultPayload, VaultRecord, VaultRecordDraft } from "@/types/vault";

const ENVELOPE_ID = "primary";
const BACKUP_KIND = "xlt-workbench-vault-backup";
const BACKUP_VERSION = 1;
const PBKDF2_ITERATIONS = 310_000;
const VERIFIER = "xlt-workbench-vault:verified:v1";
const AAD = new TextEncoder().encode("xlt-workbench-vault:v1");
const AUTO_LOCK_KEY = "xlt-workbench:vault:auto-lock-minutes";
const DEFAULT_AUTO_LOCK_MINUTES = 15;

const vaultRecordSchema = z.object({
  id: z.string().min(1),
  type: z.enum(["credential", "ai_key", "other_key"]),
  name: z.string().min(1),
  account: z.string(),
  provider: z.string(),
  secret: z.string().min(1),
  loginUrl: z.string(),
  baseUrl: z.string(),
  note: z.string(),
  tags: z.array(z.string()),
  favorite: z.boolean(),
  createdAt: z.string(),
  updatedAt: z.string(),
  lastUsedAt: z.string().nullable(),
});

const payloadSchema = z.object({ records: z.array(vaultRecordSchema) });
const envelopeSchema = z.object({
  id: z.literal(ENVELOPE_ID),
  kind: z.literal(BACKUP_KIND),
  version: z.literal(BACKUP_VERSION),
  iterations: z.number().int().positive(),
  salt: z.string().min(1),
  verifierIv: z.string().min(1),
  verifierCiphertext: z.string().min(1),
  payloadIv: z.string().min(1),
  payloadCiphertext: z.string().min(1),
  updatedAt: z.string(),
});

type VaultEnvelope = z.infer<typeof envelopeSchema>;
type Listener = (unlocked: boolean) => void;

let sessionKey: CryptoKey | null = null;
let sessionPayload: VaultPayload | null = null;
let lockTimer: number | null = null;
let activityBound = false;
const listeners = new Set<Listener>();

async function readEnvelope(): Promise<VaultEnvelope | null> {
  const raw = await invoke<string | null>("vault_read");
  if (raw === null) return null;
  let json: unknown;
  try {
    json = JSON.parse(raw);
  } catch {
    throw new Error("密钥库文件格式已损坏");
  }
  const parsed = envelopeSchema.safeParse(json);
  if (!parsed.success) throw new Error("密钥库文件格式已损坏");
  return parsed.data;
}

async function writeEnvelope(envelope: VaultEnvelope): Promise<void> {
  await invoke("vault_write", { content: JSON.stringify(envelope, null, 2) });
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 32_768) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 32_768));
  }
  return btoa(binary);
}

function base64ToBytes(value: string): Uint8Array<ArrayBuffer> {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

async function deriveKey(password: string, salt: Uint8Array<ArrayBuffer>, iterations: number): Promise<CryptoKey> {
  const material = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(password),
    "PBKDF2",
    false,
    ["deriveKey"],
  );
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", hash: "SHA-256", salt, iterations },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

async function encryptText(key: CryptoKey, value: string): Promise<{ iv: string; ciphertext: string }> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encrypted = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv, additionalData: AAD },
    key,
    new TextEncoder().encode(value),
  );
  return { iv: bytesToBase64(iv), ciphertext: bytesToBase64(new Uint8Array(encrypted)) };
}

async function decryptText(key: CryptoKey, iv: string, ciphertext: string): Promise<string> {
  const decrypted = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: base64ToBytes(iv), additionalData: AAD },
    key,
    base64ToBytes(ciphertext),
  );
  return new TextDecoder().decode(decrypted);
}

async function createEnvelope(password: string, payload: VaultPayload): Promise<{ envelope: VaultEnvelope; key: CryptoKey }> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const key = await deriveKey(password, salt, PBKDF2_ITERATIONS);
  const verifier = await encryptText(key, VERIFIER);
  const encryptedPayload = await encryptText(key, JSON.stringify(payload));
  return {
    key,
    envelope: {
      id: ENVELOPE_ID,
      kind: BACKUP_KIND,
      version: BACKUP_VERSION,
      iterations: PBKDF2_ITERATIONS,
      salt: bytesToBase64(salt),
      verifierIv: verifier.iv,
      verifierCiphertext: verifier.ciphertext,
      payloadIv: encryptedPayload.iv,
      payloadCiphertext: encryptedPayload.ciphertext,
      updatedAt: new Date().toISOString(),
    },
  };
}

async function decryptEnvelope(envelope: VaultEnvelope, password: string): Promise<{ key: CryptoKey; payload: VaultPayload }> {
  try {
    const key = await deriveKey(password, base64ToBytes(envelope.salt), envelope.iterations);
    const verifier = await decryptText(key, envelope.verifierIv, envelope.verifierCiphertext);
    if (verifier !== VERIFIER) throw new Error("主密码不正确");
    const rawPayload = await decryptText(key, envelope.payloadIv, envelope.payloadCiphertext);
    return { key, payload: payloadSchema.parse(JSON.parse(rawPayload)) };
  } catch (reason) {
    if (reason instanceof z.ZodError || reason instanceof SyntaxError) throw new Error("密钥库数据格式已损坏");
    if (reason instanceof Error && reason.message === "主密码不正确") throw reason;
    throw new Error("主密码不正确");
  }
}

export async function encryptVaultPayload(password: string, payload: VaultPayload): Promise<string> {
  if (password.length < 8) throw new Error("主密码至少需要 8 个字符");
  const validatedPayload = payloadSchema.parse(payload);
  const { envelope } = await createEnvelope(password, validatedPayload);
  return JSON.stringify(envelope, null, 2);
}

export async function decryptVaultBackup(raw: string, password: string): Promise<VaultPayload> {
  let json: unknown;
  try {
    json = JSON.parse(raw);
  } catch {
    throw new Error("备份文件格式不正确");
  }
  const parsedEnvelope = envelopeSchema.safeParse(json);
  if (!parsedEnvelope.success) throw new Error("备份文件格式不正确");
  return (await decryptEnvelope(parsedEnvelope.data, password)).payload;
}

function requireSession(): { key: CryptoKey; payload: VaultPayload } {
  if (!sessionKey || !sessionPayload) throw new Error("密钥库已锁定");
  return { key: sessionKey, payload: sessionPayload };
}

async function persistSession(): Promise<void> {
  const { key, payload } = requireSession();
  const envelope = await readEnvelope();
  if (!envelope) throw new Error("密钥库尚未初始化");
  const encryptedPayload = await encryptText(key, JSON.stringify(payload));
  await writeEnvelope({
    ...envelope,
    payloadIv: encryptedPayload.iv,
    payloadCiphertext: encryptedPayload.ciphertext,
    updatedAt: new Date().toISOString(),
  });
}

function notify(): void {
  for (const listener of listeners) listener(Boolean(sessionKey));
}

function getAutoLockMinutes(): number {
  const stored = Number(localStorage.getItem(AUTO_LOCK_KEY));
  return [1, 5, 15, 30, 60].includes(stored) ? stored : DEFAULT_AUTO_LOCK_MINUTES;
}

function resetAutoLock(): void {
  if (!sessionKey) return;
  if (lockTimer !== null) window.clearTimeout(lockTimer);
  lockTimer = window.setTimeout(() => vaultService.lock(), getAutoLockMinutes() * 60_000);
}

function bindActivity(): void {
  if (activityBound) return;
  activityBound = true;
  for (const eventName of ["pointerdown", "keydown", "scroll"] as const) {
    window.addEventListener(eventName, resetAutoLock, { passive: true });
  }
}

function activateSession(key: CryptoKey, payload: VaultPayload): void {
  sessionKey = key;
  sessionPayload = payload;
  bindActivity();
  resetAutoLock();
  notify();
}

function cleanDraft(draft: VaultRecordDraft): VaultRecordDraft {
  const tags = [...new Set(draft.tags.map((tag) => tag.trim()).filter(Boolean))];
  return {
    ...draft,
    name: draft.name.trim(),
    account: draft.account.trim(),
    provider: draft.provider.trim(),
    secret: draft.secret.trim(),
    loginUrl: draft.loginUrl.trim(),
    baseUrl: draft.baseUrl.trim(),
    note: draft.note.trim(),
    tags,
  };
}

function validateDraft(draft: VaultRecordDraft): void {
  if (!draft.name) throw new Error("请输入名称");
  if (!draft.secret) throw new Error("请输入密码或 Key 值");
  if (draft.type !== "credential" && !draft.provider) throw new Error("请输入服务商");
}

export const vaultService = {
  async isInitialized(): Promise<boolean> {
    return Boolean(await readEnvelope());
  },

  isUnlocked(): boolean {
    return Boolean(sessionKey);
  },

  subscribe(listener: Listener): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },

  async setup(password: string): Promise<void> {
    if ((await readEnvelope()) !== null) throw new Error("密钥库已初始化");
    if (password.length < 8) throw new Error("主密码至少需要 8 个字符");
    const payload: VaultPayload = { records: [] };
    const { envelope, key } = await createEnvelope(password, payload);
    await writeEnvelope(envelope);
    activateSession(key, payload);
  },

  async unlock(password: string): Promise<void> {
    const envelope = await readEnvelope();
    if (!envelope) throw new Error("密钥库尚未初始化");
    const { key, payload } = await decryptEnvelope(envelope, password);
    activateSession(key, payload);
  },

  lock(): void {
    sessionKey = null;
    sessionPayload = null;
    if (lockTimer !== null) window.clearTimeout(lockTimer);
    lockTimer = null;
    notify();
  },

  list(): VaultRecord[] {
    return structuredClone(requireSession().payload.records);
  },

  get(id: string): VaultRecord {
    const record = requireSession().payload.records.find((item) => item.id === id);
    if (!record) throw new Error("未找到该密钥记录");
    return structuredClone(record);
  },

  async save(input: VaultRecordDraft): Promise<VaultRecord> {
    const draft = cleanDraft(input);
    validateDraft(draft);
    const { payload } = requireSession();
    const now = new Date().toISOString();
    const current = draft.id ? payload.records.find((item) => item.id === draft.id) : undefined;
    const record: VaultRecord = {
      id: current?.id ?? crypto.randomUUID(),
      type: draft.type,
      name: draft.name,
      account: draft.account,
      provider: draft.provider,
      secret: draft.secret,
      loginUrl: draft.loginUrl,
      baseUrl: draft.baseUrl,
      note: draft.note,
      tags: draft.tags,
      favorite: draft.favorite,
      createdAt: current?.createdAt ?? now,
      updatedAt: now,
      lastUsedAt: current?.lastUsedAt ?? null,
    };
    const index = payload.records.findIndex((item) => item.id === record.id);
    if (index >= 0) payload.records.splice(index, 1, record);
    else payload.records.push(record);
    await persistSession();
    resetAutoLock();
    return structuredClone(record);
  },

  async remove(id: string): Promise<void> {
    const { payload } = requireSession();
    const index = payload.records.findIndex((item) => item.id === id);
    if (index < 0) throw new Error("未找到该密钥记录");
    payload.records.splice(index, 1);
    await persistSession();
    resetAutoLock();
  },

  async setFavorite(id: string, favorite: boolean): Promise<void> {
    const record = requireSession().payload.records.find((item) => item.id === id);
    if (!record) throw new Error("未找到该密钥记录");
    record.favorite = favorite;
    record.updatedAt = new Date().toISOString();
    await persistSession();
    resetAutoLock();
  },

  async touch(id: string): Promise<void> {
    const record = requireSession().payload.records.find((item) => item.id === id);
    if (!record) throw new Error("未找到该密钥记录");
    record.lastUsedAt = new Date().toISOString();
    await persistSession();
    resetAutoLock();
  },

  getAutoLockMinutes,

  setAutoLockMinutes(minutes: number): void {
    if (![1, 5, 15, 30, 60].includes(minutes)) throw new Error("无效的自动锁定时长");
    localStorage.setItem(AUTO_LOCK_KEY, String(minutes));
    resetAutoLock();
  },

  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    if (newPassword.length < 8) throw new Error("新主密码至少需要 8 个字符");
    const envelope = await readEnvelope();
    if (!envelope) throw new Error("密钥库尚未初始化");
    await decryptEnvelope(envelope, currentPassword);
    const payload = requireSession().payload;
    const next = await createEnvelope(newPassword, payload);
    await writeEnvelope(next.envelope);
    activateSession(next.key, payload);
  },

  async exportBackup(): Promise<string> {
    requireSession();
    const envelope = await readEnvelope();
    if (!envelope) throw new Error("密钥库尚未初始化");
    return JSON.stringify(envelope, null, 2);
  },

  async importBackup(raw: string, backupPassword: string): Promise<number> {
    const importedPayload = await decryptVaultBackup(raw, backupPassword);
    const current = requireSession();
    const encryptedPayload = await encryptText(current.key, JSON.stringify(importedPayload));
    const envelope = await readEnvelope();
    if (!envelope) throw new Error("密钥库尚未初始化");
    await writeEnvelope({
      ...envelope,
      payloadIv: encryptedPayload.iv,
      payloadCiphertext: encryptedPayload.ciphertext,
      updatedAt: new Date().toISOString(),
    });
    sessionPayload = importedPayload;
    resetAutoLock();
    return importedPayload.records.length;
  },
};
