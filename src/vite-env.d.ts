/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_DOWNLOAD_PAGE_URL?: string;
  /** `owner/repo` for `buildReleaseAssetDownloadUrl` (default: anamedi-ch/anamedi_lokal). */
  readonly VITE_GITHUB_RELEASE_REPOSITORY?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

