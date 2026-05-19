export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function getServerBaseUrl(): string {
  if (typeof window === "undefined") return "";
  return window.location.origin;
}
