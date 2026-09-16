import { decryptVaultBackup, encryptVaultPayload } from "@/services/vault-service";
import type { VaultPayload } from "@/types/vault";

let ok = true;
function assert(name: string, condition: boolean): void {
  console.log(`${condition ? "PASS" : "FAIL"}  ${name}`);
  if (!condition) ok = false;
}

const payload: VaultPayload = {
  records: [{
    id: "record-1",
    type: "ai_key",
    name: "生产环境 Key",
    account: "",
    provider: "OpenAI",
    secret: "sk-test-should-never-appear-in-backup",
    loginUrl: "",
    baseUrl: "https://api.openai.com",
    note: "加密回归测试",
    tags: ["工作", "生产环境"],
    favorite: true,
    createdAt: "2026-09-13T00:00:00.000Z",
    updatedAt: "2026-09-13T00:00:00.000Z",
    lastUsedAt: null,
  }],
};

const password = "correct horse battery staple";
const encrypted = await encryptVaultPayload(password, payload);
assert("备份不包含明文密钥", !encrypted.includes(payload.records[0].secret));
assert("备份不包含明文名称", !encrypted.includes(payload.records[0].name));

const decrypted = await decryptVaultBackup(encrypted, password);
assert("AES-GCM 加解密往返", decrypted.records[0].secret === payload.records[0].secret);
assert("标签与收藏字段完整", decrypted.records[0].tags.length === 2 && decrypted.records[0].favorite);

let rejectedWrongPassword = false;
try {
  await decryptVaultBackup(encrypted, "wrong password");
} catch (reason) {
  rejectedWrongPassword = reason instanceof Error && reason.message === "主密码不正确";
}
assert("错误主密码无法解密", rejectedWrongPassword);

console.log("结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
