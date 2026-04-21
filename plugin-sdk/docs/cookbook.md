# Cookbook

Focused recipes for common plugin tasks. Each recipe is self-contained and uses only `@amigo/plugin-sdk`.

## 1. Hoster with countdown and reCaptcha

```ts
import { plugin, types, captcha } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "my-host",
  version: "0.1.0",
  match: [/my-host\.example\//],
  async extract(context) {
    const landing = await context.browser.getPage(context.url);
    const form = landing.getForm();
    if (!form) {
      throw new Error("continue form missing");
    }
    await context.wait(5_000);
    const captchaPage = await form.submit();
    const challenge = await captcha.recaptchaV2(captchaPage);
    const submit = captchaPage.getForm();
    if (!submit) {
      throw new Error("submit form missing");
    }
    const final = await submit.submit({ "g-recaptcha-response": challenge.token });
    const url = final.regex(/href="([^"]+\.zip)"/).getMatch(1);
    if (!url) {
      throw new Error("final URL missing");
    }
    return [types.formatInfo({ url })];
  },
});
```

## 2. Paginated JSON API crawler

```ts
import { plugin } from "@amigo/plugin-sdk";

export default plugin.defineDecrypter({
  id: "api-folder",
  version: "0.1.0",
  match: [/api\.example\.test\/folder\//],
  async decrypt(context) {
    const urls: string[] = [];
    let cursor: string | null = "0";
    while (cursor !== null) {
      const page = await context.browser.getPage(
        `https://api.example.test/list?cursor=${cursor}`,
      );
      const body = page.json<{ items: string[]; next: string | null }>();
      urls.push(...body.items);
      cursor = body.next;
    }
    return urls;
  },
});
```

## 3. Folder decrypter (multi-level)

```ts
import { plugin } from "@amigo/plugin-sdk";

export default plugin.defineDecrypter({
  id: "folder-walker",
  version: "0.1.0",
  match: [/example\.test\/folder\//],
  async decrypt(context) {
    const links: string[] = [];
    const stack = [context.url];
    while (stack.length > 0) {
      const current = stack.pop()!;
      const page = await context.browser.getPage(current);
      for (const element of page.find("a[href]")) {
        const href = element.attr("href");
        if (!href) continue;
        if (/\/folder\//.test(href)) {
          stack.push(href);
        } else if (/\/file\//.test(href)) {
          links.push(href);
        }
      }
    }
    return links;
  },
});
```

## 4. Premium login with session persistence

```ts
import { plugin, types, account } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "premium-host",
  version: "0.1.0",
  match: [/premium\.example\.test\//],
  account: {
    async login(context, credentials) {
      const page = await context.browser.postPage(
        "https://premium.example.test/login",
        { username: credentials.username, password: credentials.password },
      );
      const data = page.json<{ token: string }>();
      return account.session({
        headers: { Authorization: `Bearer ${data.token}` },
        metadata: { token: data.token },
      });
    },
    async check() {
      return account.accountStatus({ validity: "valid", premium: true });
    },
  },
  async extract(context) {
    const header = context.account?.session.headers["Authorization"] ?? "";
    const page = await context.browser.getPage(context.url, {
      headers: { Authorization: header },
    });
    const data = page.json<{ url: string; size: number }>();
    return [types.formatInfo({ url: data.url, size: data.size })];
  },
});
```

## 5. HLS streaming with variant selection

```ts
import { plugin, media, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "vod",
  version: "0.1.0",
  match: [/vod\.example\.test\//],
  async extract(context) {
    const masterUrl = `${context.url}/master.m3u8`;
    const page = await context.browser.getPage(masterUrl);
    const master = media.hls.parseMaster(page.body(), masterUrl);
    const best = media.selectBestVariant(master.variants, { maxHeight: 1080 });
    if (!best) {
      throw new Error("no variants");
    }
    return [
      types.formatInfo({
        url: best.url,
        manifestUrl: masterUrl,
        mediaType: "hls",
        bandwidth: best.bandwidth,
      }),
    ];
  },
});
```

## 6. DASH extraction

```ts
import { plugin, media, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "dash-demo",
  version: "0.1.0",
  match: [/cdn\.example\.test\/.*\.mpd$/],
  async extract(context) {
    const page = await context.browser.getPage(context.url);
    const manifest = media.dash.parse(page.body(), context.url);
    const video = manifest.periods[0]?.adaptationSets.find((set) => set.contentType === "video");
    const best = media.selectBestVariant(video?.representations ?? []);
    if (!best) {
      throw new Error("no video representation");
    }
    return [
      types.formatInfo({
        url: best.baseUrl ?? context.url,
        manifestUrl: context.url,
        mediaType: "dash",
        bandwidth: best.bandwidth,
      }),
    ];
  },
});
```

## 7. RSDF container decrypter

```ts
import { plugin, container } from "@amigo/plugin-sdk";

export default plugin.defineDecrypter({
  id: "rsdf",
  version: "0.1.0",
  match: [/\.rsdf$/i],
  async decrypt(context) {
    const page = await context.browser.getPage(context.url);
    return container.rsdf.parse(page.body());
  },
});
```

## 8. DLC container decrypter (requires service endpoint)

```ts
import { plugin, container } from "@amigo/plugin-sdk";

export default plugin.defineDecrypter({
  id: "dlc",
  version: "0.1.0",
  match: [/\.dlc$/i],
  async decrypt(context) {
    const page = await context.browser.getPage(context.url);
    const result = await container.dlc.parse(page.body(), {
      keyExchangeEndpoint: context.config.getString("keyExchangeEndpoint") ?? "",
    });
    return result.links.map((link) => link.url);
  },
});
```

## 9. Obfuscated JS link derivation

```ts
import { plugin, javascript, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "obfuscated",
  version: "0.1.0",
  match: [/obf\.example\.test\//],
  async extract(context) {
    const page = await context.browser.getPage(context.url);
    const packed = page.regex(/eval\(function\(p,a,c,k,e,[dr]\)[\s\S]+?\)\)/).getMatch(0);
    if (!packed) {
      throw new Error("packed snippet not found");
    }
    const unpacked = await javascript.unpackDeanEdwards(packed);
    const url = /https:\/\/cdn[^"']+/.exec(unpacked)?.[0];
    if (!url) {
      throw new Error("no URL after unpacking");
    }
    return [types.formatInfo({ url })];
  },
});
```

## 10. Redirect shortener resolver

```ts
import { plugin } from "@amigo/plugin-sdk";

export default plugin.defineDecrypter({
  id: "shortener",
  version: "0.1.0",
  match: [/short\.example\.test\//],
  async decrypt(context) {
    const page = await context.browser.getPage(context.url);
    return [page.url];
  },
});
```

## 11. Multiple URL patterns

```ts
import { plugin, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "multi",
  version: "0.1.0",
  match: [
    /provider\.example\.test\/watch\//,
    /provider\.example\.test\/v\//,
    "https://provider.example.test/embed/*",
  ],
  async extract(context) {
    const page = await context.browser.getPage(context.url);
    const url = page.regex(/src="([^"]+\.mp4)"/).getMatch(1);
    if (!url) {
      throw new Error("no URL");
    }
    return [types.formatInfo({ url })];
  },
});
```

## 12. Multipart form submit (file upload style)

The SDK ships with `postPageRaw` for custom content types. Build the multipart body yourself.

```ts
import { plugin, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "upload-host",
  version: "0.1.0",
  match: [/upload\.example\.test\//],
  async extract(context) {
    const boundary = `----amigo${Date.now()}`;
    const body = [
      `--${boundary}`,
      `Content-Disposition: form-data; name="id"`,
      "",
      "42",
      `--${boundary}--`,
      "",
    ].join("\r\n");
    const page = await context.browser.postPageRaw(
      "https://upload.example.test/api",
      body,
      `multipart/form-data; boundary=${boundary}`,
    );
    const url = page.regex(/"url":"([^"]+)"/).getMatch(1);
    if (!url) {
      throw new Error("no URL");
    }
    return [types.formatInfo({ url })];
  },
});
```

## 13. Rate-limited API with retry

```ts
import { plugin, errors, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "rate-limited",
  version: "0.1.0",
  match: [/rate\.example\.test\//],
  async extract(context) {
    for (let attempt = 0; attempt < 3; attempt += 1) {
      const page = await context.browser.getPage(context.url);
      if (page.status === 429) {
        await context.wait(5_000 * (attempt + 1));
        continue;
      }
      const url = page.regex(/"url":"([^"]+)"/).getMatch(1);
      if (!url) {
        errors.parseError({ message: "no URL in JSON" });
      }
      return [types.formatInfo({ url })];
    }
    errors.retry({ retryAfterMilliseconds: 60_000 });
  },
});
```

## 14. Sanitised filename

```ts
import { plugin, types, utility } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "safe-filename",
  version: "0.1.0",
  match: [/files\.example\.test\//],
  async extract(context) {
    const page = await context.browser.getPage(context.url);
    const raw = page.regex(/"name":"([^"]+)"/).getMatch(1) ?? "file";
    return [
      types.formatInfo({
        url: context.url,
        filename: utility.formatFilename(raw, { normaliseDiacritics: true }),
      }),
    ];
  },
});
```

## 15. Debug logging and progress reporting

```ts
import { plugin, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "debug",
  version: "0.1.0",
  match: [/./],
  async extract(context) {
    context.log("info", "extraction start", { url: context.url });
    const page = await context.browser.getPage(context.url);
    context.progress(1, 2, "landing fetched");
    const url = page.regex(/"url":"([^"]+)"/).getMatch(1);
    context.progress(2, 2, "done");
    context.log("debug", "extraction done", { found: url !== null });
    if (!url) {
      throw new Error("no URL");
    }
    return [types.formatInfo({ url })];
  },
});
```
