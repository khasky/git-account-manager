import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { en, type Messages } from "./en";
import { zhHans } from "./zh-Hans";
import { zhHant } from "./zh-Hant";
import { ja } from "./ja";
import { ko } from "./ko";
import { es } from "./es";
import { ptBR } from "./pt-BR";
import { ru } from "./ru";
import { fr } from "./fr";
import { de } from "./de";
import { uk } from "./uk";

export type LangCode =
  | "en"
  | "zh-Hans"
  | "zh-Hant"
  | "ja"
  | "ko"
  | "es"
  | "pt-BR"
  | "ru"
  | "fr"
  | "de"
  | "uk";

const registry: Record<LangCode, Messages> = {
  en,
  "zh-Hans": zhHans,
  "zh-Hant": zhHant,
  ja,
  ko,
  es,
  "pt-BR": ptBR,
  ru,
  fr,
  de,
  uk,
};

// Native names shown in the language picker (always in their own script).
export const LANGUAGES: { code: LangCode; name: string }[] = [
  { code: "en", name: "English" },
  { code: "zh-Hans", name: "简体中文" },
  { code: "zh-Hant", name: "繁體中文" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "es", name: "Español" },
  { code: "pt-BR", name: "Português (BR)" },
  { code: "ru", name: "Русский" },
  { code: "fr", name: "Français" },
  { code: "de", name: "Deutsch" },
  { code: "uk", name: "Українська" },
];

const KEY = "language-preference";

function isLangCode(v: string | null): v is LangCode {
  return v !== null && Object.prototype.hasOwnProperty.call(registry, v);
}

// Best-effort match of the OS/browser locale to one of our supported languages.
function detectLang(): LangCode {
  const stored = localStorage.getItem(KEY);
  if (isLangCode(stored)) return stored;

  const nav = (navigator.language || "en").toLowerCase();
  if (nav.startsWith("zh")) {
    return nav.includes("tw") || nav.includes("hk") || nav.includes("hant")
      ? "zh-Hant"
      : "zh-Hans";
  }
  if (nav.startsWith("pt")) return "pt-BR";
  const base = nav.split("-")[0];
  const simple: LangCode[] = ["en", "ja", "ko", "es", "ru", "fr", "de", "uk"];
  const hit = simple.find((c) => c === base);
  return hit ?? "en";
}

// Replace {placeholder} tokens with provided values.
export function fmt(
  template: string,
  vars?: Record<string, string | number>,
): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (_, k) =>
    k in vars ? String(vars[k]) : `{${k}}`,
  );
}

interface RichOptions {
  onLink?: () => void;
  href?: string;
  codeClass?: string;
}

const TAG_RE = /(<\/?(?:code|b|a)>)/g;

// Render a translated string that contains a small, fixed set of inline tags
// (<code>, <b>, <a>) into React nodes. Only one <a> per string is supported;
// its behaviour comes from opts.onLink (button) or opts.href (external link).
export function rich(
  text: string,
  opts: RichOptions = {},
): React.ReactNode {
  const codeClass = opts.codeClass ?? "text-fg-3";
  const tokens = text.split(TAG_RE).filter((t) => t !== "");
  let i = 0;
  let key = 0;

  const parse = (stop?: string): React.ReactNode[] => {
    const nodes: React.ReactNode[] = [];
    while (i < tokens.length) {
      const tok = tokens[i];
      if (stop && tok === stop) {
        i++;
        break;
      }
      if (tok === "<code>") {
        i++;
        nodes.push(
          <code key={key++} className={codeClass}>
            {parse("</code>")}
          </code>,
        );
      } else if (tok === "<b>") {
        i++;
        nodes.push(<b key={key++}>{parse("</b>")}</b>);
      } else if (tok === "<a>") {
        i++;
        const inner = parse("</a>");
        nodes.push(
          opts.href ? (
            <a
              key={key++}
              href={opts.href}
              target="_blank"
              rel="noreferrer"
              className="text-link hover:text-link-hover"
            >
              {inner}
            </a>
          ) : (
            <button
              key={key++}
              type="button"
              onClick={opts.onLink}
              className="font-medium text-link underline hover:text-link-hover"
            >
              {inner}
            </button>
          ),
        );
      } else {
        nodes.push(tok);
        i++;
      }
    }
    return nodes;
  };

  return parse();
}

interface I18nCtx {
  lang: LangCode;
  setLang: (l: LangCode) => void;
  m: Messages;
}

const I18nContext = createContext<I18nCtx>({
  lang: "en",
  setLang: () => {},
  m: en,
});

export function LanguageProvider({ children }: { children: React.ReactNode }) {
  const [lang, setLangState] = useState<LangCode>(detectLang);

  const setLang = useCallback((l: LangCode) => {
    localStorage.setItem(KEY, l);
    setLangState(l);
    document.documentElement.setAttribute("lang", l);
  }, []);

  const value = useMemo<I18nCtx>(
    () => ({ lang, setLang, m: registry[lang] }),
    [lang, setLang],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  return useContext(I18nContext);
}
