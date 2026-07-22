# @amigo/plugin-sdk

TypeScript SDK for building [amigo-downloader](https://github.com/amigo-labs/amigo-downloader) plugins.

> **Status:** The SDK targets its own plugin model and mock host; the
> amigo-downloader runtime does not load SDK-style plugins yet. For plugins
> that should run inside the app today, use the CommonJS interface from
> `docs/plugin-api.md` (`plugins/template/plugin.ts`). The runtime bridge is
> tracked in `docs/plan-plugin-sdk.md`.

## Install

```
npm install @amigo/plugin-sdk
```

## Quick start

Plugins that run in amigo-downloader today use the **synchronous CommonJS**
contract — assign to `module.exports`, implement `resolve(url)`, and call the
injected global `amigo.*` host API directly. There is no `import`, no
`async`/`await`, and URLs are matched with a single `urlPattern` string.

```ts
/// <reference path="../types/amigo.d.ts" />

module.exports = {
  id: "example-hoster",
  name: "Example Hoster",
  version: "1.0.0",
  urlPattern: "https?://files\\.example\\.test/.+",

  resolve(url: string): DownloadPackage {
    const page = amigo.httpGet(url);
    if (page.status !== 200) {
      throw new Error("HTTP " + page.status);
    }
    const href = amigo.htmlQueryAttr(page.body, "a.download-btn", "href");
    if (!href) {
      throw new Error("download URL not found");
    }
    return {
      name: amigo.htmlExtractTitle(page.body) || "Download",
      downloads: [{
        url: amigo.urlResolve(url, href),
        filename: null,
        filesize: null,
        chunks_supported: true,
        max_chunks: null,
        headers: null,
        cookies: null,
        wait_seconds: null,
        mirrors: [],
      }],
    };
  },
} satisfies AmigoPlugin;
```

See `plugins/template/plugin.ts` for a fully commented starting point and
`docs/plugin-api.md` for the authoritative API reference.

> **Async is not supported.** Plugins run in a sandboxed QuickJS VM that
> executes synchronously; `resolve()` and every other plugin callback must be
> synchronous. The host API blocks internally (the runtime wraps each call in
> `spawn_blocking`), so you write straight-line code — `const page =
> amigo.httpGet(url)` — with no `await`.

## Experimental SDK model (not yet loaded by the runtime)

The `@amigo/plugin-sdk` package also ships a higher-level, promise-based
authoring model (`definePlugin`, `Browser`/`Page`, captcha helpers). It runs
only against the SDK's own mock host for now — the runtime bridge is tracked in
`docs/plan-plugin-sdk.md`. Do **not** ship a plugin written this way expecting
it to load in the app yet.

```ts
import { plugin, types, captcha } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "example-hoster",
  version: "1.0.0",
  match: [/files\.example\.test\//],
  async extract(context) {
    const landing = await context.browser.getPage(context.url);
    const form = landing.getForm();
    if (!form) {
      throw new Error("continue form missing");
    }
    const captchaPage = await form.submit();
    const challenge = await captcha.recaptchaV2(captchaPage);
    const submit = captchaPage.getForm();
    const finalPage = await submit!.submit({
      "g-recaptcha-response": challenge.token,
    });
    const href = finalPage.regex(/href="([^"]+\.zip)"/).getMatch(1);
    if (!href) {
      throw new Error("download URL not found");
    }
    return [types.formatInfo({ url: href })];
  },
});
```

## Scaffold a new plugin

```
npx amigo-plugin new my-hoster --kind hoster
cd my-hoster
npm install
npm run build
npx amigo-plugin test https://my-hoster.example/sample --plugin ./dist/index.js
```

## Modules

| Namespace | Purpose |
|---|---|
| `host` | Inject a `HostApi`, or use `createMockHostApi` in tests |
| `browser` | `Browser`, `Page`, `CookieJar`, `Headers`, `Element` |
| `extraction` | `regex`, `json.*`, `encoding.*`, `html.*` helpers |
| `form` | `Form` with `.submit()` |
| `errors` | `PluginError`, `ErrorCode`, factories |
| `captcha` | `recaptchaV2`, `hcaptcha`, `turnstile`, `image`, `auto`, … |
| `plugin` | `definePlugin`, `defineDecrypter` |
| `context` | `createPluginContext` |
| `account` | `Session`, `AccountConfig`, `AccountStatus` |
| `media` | `hls.parseMaster`, `dash.parse`, `selectBestVariant`, … |
| `container` | `rsdf.parse`, `ccf.parse`, `dlc.parse`, `detect` |
| `javascript` | `run`, `unpackDeanEdwards`, `unpackEval` |
| `utility` | `parseSize`, `parseDuration`, `parseDate`, `formatFilename` |
| `cli` | `amigo-plugin` commands |

## Docs

- [Tutorial — your first plugin in 10 minutes](./docs/tutorial.md)
- [Cookbook](./docs/cookbook.md)
- [JDownloader → SDK mapping](./docs/jdownloader-mapping.md)

## Development

```
npm install
npm test
npm run typecheck
```

## License

See repository root.
