# JDownloader → amigo-plugin-sdk mapping

Reference for porting JDownloader plugins to `@amigo/plugin-sdk`. The SDK keeps the same conceptual model but spells out names in full (no `br`, `ctx`, `fmt`).

## Core objects

| JDownloader | SDK |
|---|---|
| `new Browser()` | `new Browser({ hostApi })` |
| `br.getPage(url)` | `browser.getPage(url)` → `Page` |
| `br.postPage(url, data)` | `browser.postPage(url, data)` |
| `br.postPageRaw(url, body)` | `browser.postPageRaw(url, body, contentType)` |
| `br.getHost()` | `new URL(browser.getUrl()!).host` |
| `br.getRedirectLocation()` | `browser.getHeader("Location")` or `page.redirectLocation` |
| `br.setFollowRedirects(bool)` | `browser.setFollowRedirects(bool)` |
| `br.setCookie(host, name, value)` | `browser.cookieJar.set(url, "name=value")` |
| `br.cloneBrowser()` | `browser.clone()` |
| `br.containsHTML(pattern)` | `browser.containsHTML(pattern)` |
| `br.getRegex(pattern).getMatch(n)` | `browser.regex(pattern).getMatch(n)` |
| `br.getRequest().getHtmlCode()` | `browser.body()` |

## Forms

| JDownloader | SDK |
|---|---|
| `Form form = br.getFormbyProperty("name", "login")` | `page.getForm("form[name='login']")` |
| `form.put("user", value)` | `form.put("user", value)` |
| `form.remove("honeypot")` | `form.remove("honeypot")` |
| `br.submitForm(form)` | `form.submit()` or `browser.submitForm(form)` |
| `form.getInputField("name").getValue()` | `form.get("name")` |

## Regex / extraction

| JDownloader | SDK |
|---|---|
| `new Regex(text, pattern).getMatch(0)` | `regex(text, pattern).getMatch(0)` |
| `br.getRegex(pattern).getMatch(0)` | `browser.regex(pattern).getMatch(0)` |
| `br.getRegex(pattern).getColumn(0)` | `browser.regex(pattern).getColumn(0)` |
| `PluginJSonUtils.getJson(source, "token")` | `json.extract(source, "token")` |
| `JSonStorage.restoreFromString(source)` | `json.parse(source)` |
| `Encoding.htmlDecode(value)` | `encoding.htmlDecode(value)` |
| `Encoding.Base64Decode(value)` | `encoding.base64DecodeToString(value)` |

## Errors

| JDownloader | SDK |
|---|---|
| `throw new PluginException(LinkStatus.ERROR_FILE_NOT_FOUND)` | `errors.fileNotFound()` |
| `throw new PluginException(LinkStatus.ERROR_PREMIUM)` | `errors.premiumOnly()` |
| `throw new PluginException(LinkStatus.ERROR_IP_BLOCKED)` | `errors.ipBlocked()` |
| `throw new PluginException(LinkStatus.ERROR_TEMPORARILY_UNAVAILABLE, "…", retry)` | `errors.temporarilyUnavailable({ retryAfterMilliseconds: retry })` |
| `throw new PluginException(LinkStatus.ERROR_CAPTCHA)` | `errors.captchaFailed()` |
| `throw new PluginException(LinkStatus.ERROR_PLUGIN_DEFECT)` | `errors.pluginDefect()` |
| `throw new PluginException(LinkStatus.ERROR_FATAL)` | `errors.fatal()` |

## Captcha

| JDownloader | SDK |
|---|---|
| `getCaptchaCode("recaptchav2", this.getDownloadLink())` | `captcha.recaptchaV2(page)` |
| `getCaptchaCode("hcaptcha", …)` | `captcha.hcaptcha(page)` |
| `getCaptchaCode(file)` (image) | `captcha.image(url)` |

## Plugin lifecycle

| JDownloader | SDK |
|---|---|
| `public void requestFileInformation(DownloadLink link)` | `checkAvailable(context)` returning `FileInfo` |
| `public void handleFree(DownloadLink link)` / `handlePremium` | `extract(context)` returning `FormatInfo[]` |
| `public ArrayList<DownloadLink> decryptIt(CryptedLink)` | `decrypt(context)` returning `string[]` or `DownloadLink[]` |
| `public AccountInfo fetchAccountInfo(Account)` | `account.check(context, session)` returning `AccountStatus` |
| `public void login(Account)` | `account.login(context, credentials)` returning `Session` |

## Types

| JDownloader | SDK |
|---|---|
| `DownloadLink link = createDownloadlink(url)` | `context.link(url)` |
| `link.setFinalFileName(name)` | `types.formatInfo({ url, filename: name })` |
| `link.setDownloadSize(n)` | `types.formatInfo({ url, size: n })` |
| `link.setProperty("key", value)` | `types.formatInfo({ url, properties: { key: value } })` |

## Logging

| JDownloader | SDK |
|---|---|
| `logger.info("hello")` | `context.log("info", "hello")` |
| `logger.warning("warn")` | `context.log("warn", "warn")` |
| `Thread.sleep(ms)` | `await context.wait(ms)` |

## Manifest

JDownloader annotates plugins with `@DecrypterPlugin(revision = …, names = {…}, urls = {…})`. The SDK replaces this with `plugin.toml`:

```toml
id = "my-host"
name = "My Host"
version = "1.0.0"
kind = "hoster"
sdk_version = "0.0.0"
match = ["https://myhost.example/*"]
permissions = []
```

`kind` is `hoster` for `definePlugin` and `decrypter` for `defineDecrypter`. Add `permissions = ["javascript_eval"]` to opt into `javascript.run`.
