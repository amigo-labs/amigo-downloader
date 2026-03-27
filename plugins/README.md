# amigo-downloader Plugins

JavaScript/TypeScript plugin system for hoster support, powered by QuickJS-NG.

## Creating a Plugin

1. Copy `plugin-template/plugin.js` to `hosters/your_hoster.js`
2. Implement the required functions: `pluginId`, `pluginName`, `pluginVersion`, `urlPattern`, `resolve`
3. Place the file in the `plugins/hosters/` directory — it will be auto-detected

See `plugin-template/plugin.js` for the full template and `types/amigo.d.ts` for TypeScript type stubs.

## Host API

All network/filesystem access goes through `amigo.*` — plugins have no direct access.

- `amigo.httpGet(url)` / `amigo.httpPost(url, body, contentType)` / `amigo.httpHead(url)` — returns JSON string with `{status, body, headers}`
- `amigo.regexMatch(pattern, text)` / `amigo.regexMatchAll(pattern, text)` — regex helpers
- `amigo.base64Encode(input)` / `amigo.base64Decode(input)` — Base64 helpers
- `amigo.setCookie(domain, name, value)` / `amigo.getCookie(domain, name)` / `amigo.clearCookies(domain)` — cookie management
- `amigo.storageGet(key)` / `amigo.storageSet(key, value)` / `amigo.storageDelete(key)` — persistent storage
- `amigo.logInfo(msg)` / `amigo.logWarn(msg)` / `amigo.logError(msg)` / `amigo.logDebug(msg)` — logging
- `console.log/warn/error` — also available as logging bridge
