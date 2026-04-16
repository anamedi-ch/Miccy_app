/**
 * Where users download full app installers (marketing site).
 * Set `VITE_DOWNLOAD_PAGE_URL` at build time (e.g. staging URL); production default
 * is https://miccy.app
 *
 * Note: In-app “check for updates” still uses the Tauri updater `endpoints` URL
 * (typically GitHub Releases `latest.json`) — that is separate from this link.
 */
export function getDownloadPageUrl(): string {
  const fromEnv = import.meta.env.VITE_DOWNLOAD_PAGE_URL;
  if (typeof fromEnv === "string" && fromEnv.trim() !== "") {
    return fromEnv.trim();
  }
  return "https://github.com/anamedi-ch/anamedi_lokal/releases/latest";
}
