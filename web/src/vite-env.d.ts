/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_DZI_URL?: string;
  readonly VITE_REFERENCE_URL?: string;
  readonly VITE_RELEASE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
