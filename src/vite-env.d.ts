/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_STORE_SAFE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
