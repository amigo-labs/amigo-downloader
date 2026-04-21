# @amigo/plugin-sdk

TypeScript SDK for building [amigo-downloader](https://github.com/amigo-labs/amigo-downloader) plugins.

## Install

```
npm install @amigo/plugin-sdk
```

## Quick start

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
