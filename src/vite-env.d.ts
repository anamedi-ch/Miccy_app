/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_DOWNLOAD_PAGE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

