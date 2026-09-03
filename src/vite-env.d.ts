/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<Record<string, unknown>, Record<string, unknown>, unknown>;
  export default component;
}

interface ImportMetaEnv {
  readonly VITE_ARK_API_KEY?: string;
  readonly VITE_ARK_BASE_URL?: string;
  readonly VITE_OPENAI_ADMIN_KEY?: string;
  readonly VITE_OPENAI_API_KEY?: string;
  readonly VITE_OPENAI_ORG_ID?: string;
  readonly VITE_OPENAI_BASE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
