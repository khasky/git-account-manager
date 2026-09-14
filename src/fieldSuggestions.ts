/**
 * What every text field in this app spreads to stop the webview guessing at it.
 *
 * The fields here hold a profile name, a Git identity, an API token and an
 * OAuth client id. The browser has nothing useful to offer for any of them, and
 * what it does offer covers the form with a dropdown of addresses typed into
 * other applications. `autoCorrect` and `autoCapitalize` matter on the Apple
 * platforms, where they rewrite what was typed rather than merely suggest:
 * an address is not a sentence and must not be capitalized like one.
 */
export const noSuggestions = {
  autoComplete: "off",
  autoCorrect: "off",
  autoCapitalize: "off",
} as const;
