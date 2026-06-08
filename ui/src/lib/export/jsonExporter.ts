import type { SessionDetail } from "../api";
import type { ExportOptions } from "./types";
import { projectSessionDetail } from "./projection";

export function exportAsJson(detail: SessionDetail, options: ExportOptions): string {
  const projected = projectSessionDetail(detail, options);
  return JSON.stringify(projected, null, 2);
}
