export type ExportFormat = "markdown" | "json" | "html";

export type ToolOutputMode = "full" | "truncated" | "name-only";

export interface ExportOptions {
  format: ExportFormat;
  includeThinking: boolean;
  toolOutputMode: ToolOutputMode;
  toolOutputMaxLength: number;
  includeSubagents: boolean;
}

export const DEFAULT_EXPORT_OPTIONS: Omit<ExportOptions, "format"> = {
  includeThinking: true,
  toolOutputMode: "full",
  toolOutputMaxLength: 2000,
  includeSubagents: true,
};
