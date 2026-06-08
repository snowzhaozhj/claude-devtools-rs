import type { SessionDetail } from "../api";
import type { ExportOptions, ExportFormat } from "./types";
import { DEFAULT_EXPORT_OPTIONS } from "./types";
import { exportAsMarkdown } from "./markdownExporter";
import { exportAsJson } from "./jsonExporter";
import { exportAsHtml } from "./htmlExporter";

export type { ExportOptions, ExportFormat } from "./types";
export { DEFAULT_EXPORT_OPTIONS } from "./types";

export function exportSession(detail: SessionDetail, format: ExportFormat): string {
  const options: ExportOptions = { ...DEFAULT_EXPORT_OPTIONS, format };

  switch (format) {
    case "markdown":
      return exportAsMarkdown(detail, options);
    case "json":
      return exportAsJson(detail, options);
    case "html":
      return exportAsHtml(detail, options);
  }
}

export function getExportFileName(sessionId: string, format: ExportFormat): string {
  const ext = format === "markdown" ? "md" : format;
  return `session-${sessionId}.${ext}`;
}

export function getExportFilterExt(format: ExportFormat): string {
  return format === "markdown" ? "md" : format;
}
