import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const target = fileURLToPath(new URL("../src-tauri/target/diagnostic/", import.meta.url));
const manifestPath = new URL("../src-tauri/Cargo.toml", import.meta.url);
const originalManifest = readFileSync(manifestPath, "utf8");
const diagnosticManifest = originalManifest.replace(
  'tauri = { version = "2", features = ["tray-icon", "macos-private-api"] }',
  'tauri = { version = "2", features = ["tray-icon"] }',
);

// 单独存放编译产物，限制并发并禁用 LTO，降低本机诊断构建的资源压力。
try {
  const result = spawnSync("pnpm", [
    "exec", "tauri", "build", "--ci", "--config", "src-tauri/tauri.diagnostic.conf.json",
  ], {
    cwd: root,
    stdio: "inherit",
    env: {
      ...process.env,
      CARGO_TARGET_DIR: target,
      CARGO_BUILD_JOBS: "2",
      CARGO_PROFILE_RELEASE_LTO: "false",
    },
  });

  if (result.error) console.error(`诊断包构建失败：${result.error.message}`);
  process.exitCode = result.status ?? 1;
} finally {
  // Tauri CLI 会按诊断配置重写依赖 feature；仅恢复已知变更，不覆盖并发编辑。
  const currentManifest = readFileSync(manifestPath, "utf8");
  if (currentManifest !== originalManifest) {
    if (currentManifest === diagnosticManifest) {
      writeFileSync(manifestPath, originalManifest);
    } else {
      console.error("Cargo.toml 出现非预期变更，未自动覆盖；请确认普通版 macos-private-api 配置。");
      process.exitCode = 1;
    }
  }
}
