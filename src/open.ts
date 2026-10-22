import { openUrl as open } from "@tauri-apps/plugin-opener";

/** Open a localhost URL in the default browser (scoped in capabilities/default.json). */
export function openUrl(url: string): Promise<void> {
  return open(url);
}
