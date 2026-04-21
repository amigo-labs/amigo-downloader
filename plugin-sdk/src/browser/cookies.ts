export interface Cookie {
  readonly name: string;
  readonly value: string;
  readonly domain: string;
  readonly path: string;
  readonly hostOnly: boolean;
  readonly secure: boolean;
  readonly httpOnly: boolean;
  readonly expiresAt: number | null;
}

export interface CookieJarExport {
  readonly cookies: readonly Cookie[];
}

function normaliseHost(host: string): string {
  return host.toLowerCase().replace(/^\.+/, "").replace(/\.+$/, "");
}

function domainMatches(cookieDomain: string, requestHost: string): boolean {
  const cookie = normaliseHost(cookieDomain);
  const request = normaliseHost(requestHost);
  if (cookie === request) {
    return true;
  }
  if (request.endsWith(`.${cookie}`)) {
    return true;
  }
  return false;
}

function pathMatches(cookiePath: string, requestPath: string): boolean {
  if (cookiePath === requestPath) {
    return true;
  }
  if (requestPath.startsWith(cookiePath)) {
    if (cookiePath.endsWith("/")) {
      return true;
    }
    if (requestPath.charAt(cookiePath.length) === "/") {
      return true;
    }
  }
  return false;
}

function defaultPath(requestPath: string): string {
  if (!requestPath.startsWith("/")) {
    return "/";
  }
  const lastSlash = requestPath.lastIndexOf("/");
  if (lastSlash <= 0) {
    return "/";
  }
  return requestPath.slice(0, lastSlash);
}

function parseUrl(url: string): { host: string; path: string; scheme: string } {
  const parsed = new URL(url);
  return {
    host: parsed.hostname,
    path: parsed.pathname || "/",
    scheme: parsed.protocol.replace(":", ""),
  };
}

export function parseSetCookie(header: string, requestUrl: string): Cookie | null {
  const parts = header.split(";").map((part) => part.trim()).filter((part) => part.length > 0);
  if (parts.length === 0) {
    return null;
  }
  const nameValue = parts[0]!;
  const equalsIndex = nameValue.indexOf("=");
  if (equalsIndex <= 0) {
    return null;
  }
  const name = nameValue.slice(0, equalsIndex).trim();
  const value = nameValue.slice(equalsIndex + 1).trim();
  const request = parseUrl(requestUrl);

  let domain = request.host;
  let hostOnly = true;
  let path = defaultPath(request.path);
  let secure = false;
  let httpOnly = false;
  let expiresAt: number | null = null;
  let maxAgeSet = false;

  for (const part of parts.slice(1)) {
    const separator = part.indexOf("=");
    const attributeName = (separator >= 0 ? part.slice(0, separator) : part).trim().toLowerCase();
    const attributeValue = separator >= 0 ? part.slice(separator + 1).trim() : "";
    switch (attributeName) {
      case "domain": {
        if (attributeValue.length > 0) {
          domain = normaliseHost(attributeValue);
          hostOnly = false;
        }
        break;
      }
      case "path": {
        if (attributeValue.startsWith("/")) {
          path = attributeValue;
        }
        break;
      }
      case "secure": {
        secure = true;
        break;
      }
      case "httponly": {
        httpOnly = true;
        break;
      }
      case "max-age": {
        const seconds = Number.parseInt(attributeValue, 10);
        if (!Number.isNaN(seconds)) {
          expiresAt = Date.now() + seconds * 1000;
          maxAgeSet = true;
        }
        break;
      }
      case "expires": {
        if (maxAgeSet) {
          break;
        }
        const timestamp = Date.parse(attributeValue);
        if (!Number.isNaN(timestamp)) {
          expiresAt = timestamp;
        }
        break;
      }
      default: {
        break;
      }
    }
  }

  return { name, value, domain, path, hostOnly, secure, httpOnly, expiresAt };
}

export class CookieJar {
  private cookies: Cookie[] = [];

  set(url: string, setCookieHeader: string): void {
    const cookie = parseSetCookie(setCookieHeader, url);
    if (!cookie) {
      return;
    }
    this.store(cookie);
  }

  setAll(url: string, setCookieHeaders: readonly string[]): void {
    for (const header of setCookieHeaders) {
      this.set(url, header);
    }
  }

  store(cookie: Cookie): void {
    this.cookies = this.cookies.filter(
      (existing) =>
        !(
          existing.name === cookie.name &&
          existing.domain === cookie.domain &&
          existing.path === cookie.path
        ),
    );
    if (cookie.expiresAt !== null && cookie.expiresAt <= Date.now()) {
      return;
    }
    this.cookies.push(cookie);
  }

  get(url: string): string {
    const applicable = this.applicableTo(url);
    return applicable.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
  }

  applicableTo(url: string): Cookie[] {
    const { host, path, scheme } = parseUrl(url);
    const now = Date.now();
    return this.cookies.filter((cookie) => {
      if (cookie.expiresAt !== null && cookie.expiresAt <= now) {
        return false;
      }
      if (cookie.secure && scheme !== "https") {
        return false;
      }
      const matchesHost = cookie.hostOnly
        ? normaliseHost(cookie.domain) === normaliseHost(host)
        : domainMatches(cookie.domain, host);
      if (!matchesHost) {
        return false;
      }
      return pathMatches(cookie.path, path);
    });
  }

  clear(): void {
    this.cookies = [];
  }

  clearHost(host: string): void {
    const normalised = normaliseHost(host);
    this.cookies = this.cookies.filter(
      (cookie) => normaliseHost(cookie.domain) !== normalised,
    );
  }

  export(): CookieJarExport {
    return { cookies: this.cookies.map((cookie) => ({ ...cookie })) };
  }

  import(data: CookieJarExport): void {
    this.cookies = data.cookies.map((cookie) => ({ ...cookie }));
  }

  get size(): number {
    return this.cookies.length;
  }
}
