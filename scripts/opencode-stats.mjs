import { DatabaseSync } from "node:sqlite";
import { join } from "node:path";
import { homedir } from "node:os";

export function readOpenCodeStats({ start, end, tz, databasePath, hourly = false } = {}) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(start) || !/^\d{4}-\d{2}-\d{2}$/.test(end) || start > end) {
    throw new Error("OpenCode 起止日期必须是有效的 YYYY-MM-DD");
  }
  const path = databasePath ?? join(process.env.XDG_DATA_HOME || join(homedir(), ".local", "share"), "opencode", "opencode.db");
  const db = new DatabaseSync(path, { readOnly: true });
  try {
    db.exec("PRAGMA busy_timeout = 5000");
    const rows = db.prepare(`SELECT time_created,
      COALESCE(json_extract(data,'$.modelID'),json_extract(data,'$.model'),json_extract(data,'$.modelId'),'opencode') model,
      json_extract(data,'$.tokens.input') inp, json_extract(data,'$.tokens.output') outp,
      COALESCE(json_extract(data,'$.tokens.cache.read'),0) cache
      FROM message WHERE time_created >= ? AND time_created < ?
      AND json_extract(data,'$.role')='assistant' AND json_extract(data,'$.tokens.input') IS NOT NULL`)
      .all(Date.parse(start) - 86400000, Date.parse(end) + 172800000);
    const formatter = new Intl.DateTimeFormat("en-CA", { timeZone: tz, year: "numeric", month: "2-digit", day: "2-digit" });
    const totals = new Map();
    for (const row of rows) {
      const instant = new Date(row.time_created);
      const date = formatter.format(instant);
      if (date < start || date > end) continue;
      const hour = hourly ? new Intl.DateTimeFormat("en-GB", { timeZone: tz, hour: "2-digit", hourCycle: "h23" }).format(instant) : "";
      const bucket = hourly ? `${date} ${hour}` : date;
      const key = `${bucket}:${row.model}`;
      if (!totals.has(key)) totals.set(key, { d: bucket, model: row.model, inp: 0, outp: 0, cache: 0 });
      const total = totals.get(key);
      total.inp += row.inp;
      total.outp += row.outp ?? 0;
      total.cache += row.cache;
    }
    return [...totals.values()].sort((a, b) => a.d.localeCompare(b.d) || a.model.localeCompare(b.model));
  } finally { db.close(); }
}
