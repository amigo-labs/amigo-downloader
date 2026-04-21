# Tutorial — your first plugin in 10 minutes

This walkthrough builds a small hoster plugin that extracts a download URL from a landing page. It covers the core lifecycle: scaffold → code → test.

## 1. Scaffold

```
npx amigo-plugin new my-hoster --kind hoster
cd my-hoster
npm install
```

You now have:

```
my-hoster/
├── package.json
├── tsconfig.json
├── plugin.toml
├── README.md
└── src/
    └── index.ts
```

## 2. Code the extractor

Replace `src/index.ts` with the plugin below. It fetches the landing page, pulls the download URL out of the HTML, and returns a `FormatInfo`.

```ts
import { plugin, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "my-hoster",
  version: "0.1.0",
  match: [/my-hoster\.example\/file\//],
  async extract(context) {
    const page = await context.browser.getPage(context.url);
    const downloadUrl = page.regex(/"downloadUrl":"([^"]+)"/).getMatch(1);
    if (!downloadUrl) {
      throw new Error("download URL missing");
    }
    const filename = page.regex(/"filename":"([^"]+)"/).getMatch(1);
    return [
      types.formatInfo({
        url: downloadUrl,
        filename: filename ?? null,
      }),
    ];
  },
});
```

## 3. Build

```
npm run build
```

This runs `tsc` and emits `dist/index.js` plus a declaration file.

## 4. Test against a mock

The CLI can run your plugin against any URL using a canned fixture file. Create `fixtures.json`:

```json
{
  "https://my-hoster.example/file/xyz": "{\"downloadUrl\":\"https://cdn.my-hoster.example/xyz.zip\",\"filename\":\"xyz.zip\"}"
}
```

Run:

```
npx amigo-plugin test https://my-hoster.example/file/xyz \
  --plugin ./dist/index.js --fixtures ./fixtures.json
```

You should see the extracted `FormatInfo` printed as JSON.

## 5. Validate the manifest

```
npx amigo-plugin validate --plugin ./dist/index.js
```

This prints the plugin manifest and a capability summary (whether `extract`, `decrypt`, `checkAvailable`, or an account config are defined).

## Next steps

- Read the [cookbook](./cookbook.md) for patterns (captcha, pagination, HLS, containers, …).
- Read the [JDownloader mapping](./jdownloader-mapping.md) if you are porting an existing plugin.
- When the plugin is ready, package and install it into a running amigo-downloader instance.
