import { fetchQoderUsage } from "./qoder-usage-lib.mjs";
try {
  console.log(JSON.stringify(await fetchQoderUsage()));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
