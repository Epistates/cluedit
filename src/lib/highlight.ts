import { createHighlighter, type Highlighter } from "shiki";

let highlighterPromise: Promise<Highlighter> | null = null;

const PRELOADED_LANGUAGES = [
  "javascript",
  "typescript",
  "python",
  "rust",
  "go",
  "bash",
  "json",
  "html",
  "css",
  "svelte",
  "toml",
  "yaml",
  "markdown",
  "sql",
  "diff",
] as const;

export function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ["vitesse-dark"],
      langs: [...PRELOADED_LANGUAGES],
    });
  }
  return highlighterPromise;
}

export async function highlightCode(
  code: string,
  lang: string
): Promise<string> {
  try {
    const highlighter = await getHighlighter();
    const loadedLangs = highlighter.getLoadedLanguages();
    const normalizedLang = lang.toLowerCase();

    if (!loadedLangs.includes(normalizedLang as typeof loadedLangs[number])) {
      try {
        await highlighter.loadLanguage(normalizedLang as Parameters<typeof highlighter.loadLanguage>[0]);
      } catch {
        // Language not available — fall back to plaintext
        return highlighter.codeToHtml(code, {
          lang: "text",
          theme: "vitesse-dark",
        });
      }
    }

    return highlighter.codeToHtml(code, {
      lang: normalizedLang,
      theme: "vitesse-dark",
    });
  } catch {
    // Highlighter failed entirely — return escaped plaintext
    const escaped = code
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    return `<pre class="shiki"><code>${escaped}</code></pre>`;
  }
}
