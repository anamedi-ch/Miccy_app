/**
 * Expected GitHub Release filenames for Miccy installers (see `.github/workflows/release.yml`).
 * Use `buildReleaseAssetFilename` + `buildReleaseAssetDownloadUrl` to link to a tagged release.
 *
 * If Tauri renames bundles in a future version, update `filenameTemplate` here only.
 */

const VERSION_TOKEN = "{version}" as const;

export type ReleaseDownloadKind =
  | "dmg"
  | "exe"
  | "msi"
  | "appimage"
  | "deb"
  | "rpm";

/** Human-facing row for download pages (marketing site, docs). */
export interface ReleaseDownloadRow {
  readonly id: string;
  readonly osLine: string;
  readonly archOrFlavorLine: string;
  readonly distributionLine: string;
  readonly packageLine: string;
  readonly kind: ReleaseDownloadKind;
  readonly filenameTemplate: string;
}

/**
 * All variants produced by the Release workflow, in a stable display order.
 * `filenameTemplate` uses `{version}` (e.g. `0.7.1`, no `v` prefix).
 */
export const RELEASE_DOWNLOAD_MATRIX: readonly ReleaseDownloadRow[] = [
  {
    id: "mac-apple-silicon-dmg",
    osLine: "Mac",
    archOrFlavorLine: "Apple Silicon",
    distributionLine: "",
    packageLine: ".dmg",
    kind: "dmg",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_aarch64.dmg`,
  },
  {
    id: "mac-intel-dmg",
    osLine: "Mac",
    archOrFlavorLine: "Intel (x86)",
    distributionLine: "",
    packageLine: ".dmg",
    kind: "dmg",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_x64.dmg`,
  },
  {
    id: "windows-x64-nsis",
    osLine: "Windows (x64)",
    archOrFlavorLine: "Installer",
    distributionLine: "",
    packageLine: ".exe",
    kind: "exe",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_x64-setup.exe`,
  },
  {
    id: "windows-x64-msi",
    osLine: "Windows (x64)",
    archOrFlavorLine: "MSI",
    distributionLine: "",
    packageLine: ".msi",
    kind: "msi",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_x64_en-US.msi`,
  },
  {
    id: "windows-arm-nsis",
    osLine: "Windows (ARM)",
    archOrFlavorLine: "Installer",
    distributionLine: "",
    packageLine: ".exe",
    kind: "exe",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_arm64-setup.exe`,
  },
  {
    id: "windows-arm-msi",
    osLine: "Windows (ARM)",
    archOrFlavorLine: "MSI",
    distributionLine: "",
    packageLine: ".msi",
    kind: "msi",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_arm64_en-US.msi`,
  },
  {
    id: "linux-x64-appimage",
    osLine: "Linux (x64)",
    archOrFlavorLine: "Most distros",
    distributionLine: "",
    packageLine: ".AppImage",
    kind: "appimage",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_amd64.AppImage`,
  },
  {
    id: "linux-x64-deb",
    osLine: "Linux (x64)",
    archOrFlavorLine: "Ubuntu / Debian",
    distributionLine: "",
    packageLine: ".deb",
    kind: "deb",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_amd64.deb`,
  },
  {
    id: "linux-x64-rpm",
    osLine: "Linux (x64)",
    archOrFlavorLine: "RHEL / Fedora",
    distributionLine: "",
    packageLine: ".rpm",
    kind: "rpm",
    filenameTemplate: `Miccy_${VERSION_TOKEN}-1.x86_64.rpm`,
  },
  {
    id: "linux-arm-appimage",
    osLine: "Linux (ARM)",
    archOrFlavorLine: "Most distros",
    distributionLine: "",
    packageLine: ".AppImage",
    kind: "appimage",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_aarch64.AppImage`,
  },
  {
    id: "linux-arm-deb",
    osLine: "Linux (ARM)",
    archOrFlavorLine: "Ubuntu / Debian",
    distributionLine: "",
    packageLine: ".deb",
    kind: "deb",
    filenameTemplate: `Miccy_${VERSION_TOKEN}_arm64.deb`,
  },
  {
    id: "linux-arm-rpm",
    osLine: "Linux (ARM)",
    archOrFlavorLine: "RHEL / Fedora",
    distributionLine: "",
    packageLine: ".rpm",
    kind: "rpm",
    filenameTemplate: `Miccy_${VERSION_TOKEN}-1.aarch64.rpm`,
  },
] as const;

/** Default `owner/repo` for GitHub Releases (override with `VITE_GITHUB_RELEASE_REPOSITORY`). */
export const GITHUB_RELEASE_REPOSITORY_FALLBACK = "anamedi-ch/anamedi_lokal" as const;

/** Shown on download/marketing pages next to “Made with …”. */
export const DOWNLOAD_PAGE_MADE_WITH = {
  name: "Tauri",
  url: "https://tauri.app",
} as const;

/**
 * Resolves `owner/repo` for release asset URLs (same default as `getDownloadPageUrl`’s GitHub target).
 */
export function getGithubReleaseRepository(): string {
  const fromEnv = import.meta.env.VITE_GITHUB_RELEASE_REPOSITORY;
  if (typeof fromEnv === "string" && fromEnv.trim() !== "") {
    return fromEnv.trim();
  }
  return GITHUB_RELEASE_REPOSITORY_FALLBACK;
}

/**
 * @param appVersion - Semver from `tauri.conf.json` (e.g. `0.7.1`), no `v` prefix.
 */
export function buildReleaseAssetFilename(
  appVersion: string,
  filenameTemplate: string,
): string {
  return filenameTemplate.split(VERSION_TOKEN).join(appVersion);
}

/**
 * Direct download URL for a file attached to a GitHub Release.
 *
 * @param appVersion - Semver (e.g. `0.7.1`); tag becomes `v${appVersion}`.
 */
export function buildReleaseAssetDownloadUrl(
  appVersion: string,
  filenameTemplate: string,
  repository: string = getGithubReleaseRepository(),
): string {
  const filename = buildReleaseAssetFilename(appVersion, filenameTemplate);
  const tag = `v${appVersion}`;
  return `https://github.com/${repository}/releases/download/${tag}/${filename}`;
}

/**
 * Convenience: URL for a row in `RELEASE_DOWNLOAD_MATRIX`.
 */
export function buildMatrixRowDownloadUrl(
  row: Pick<ReleaseDownloadRow, "filenameTemplate">,
  appVersion: string,
  repository?: string,
): string {
  return buildReleaseAssetDownloadUrl(appVersion, row.filenameTemplate, repository);
}
