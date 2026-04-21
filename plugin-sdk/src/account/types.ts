export interface SessionCookie {
  readonly name: string;
  readonly value: string;
  readonly domain: string;
  readonly path: string;
  readonly expiresAt: number | null;
  readonly secure: boolean;
  readonly httpOnly: boolean;
  readonly hostOnly: boolean;
}

export interface Session {
  readonly cookies: readonly SessionCookie[];
  readonly headers: Readonly<Record<string, string>>;
  readonly metadata: Readonly<Record<string, unknown>>;
  readonly createdAt: number;
}

export interface AccountCredentials {
  readonly username: string;
  readonly password: string;
  readonly extra: Readonly<Record<string, string>>;
}

export type AccountValidity = "valid" | "invalid" | "expired" | "unknown";

export interface AccountStatus {
  readonly validity: AccountValidity;
  readonly premium: boolean;
  readonly expiresAt: number | null;
  readonly trafficLeftBytes: number | null;
  readonly message: string | null;
}

export function session(init: Partial<Session> = {}): Session {
  return {
    cookies: init.cookies ?? [],
    headers: init.headers ?? {},
    metadata: init.metadata ?? {},
    createdAt: init.createdAt ?? Date.now(),
  };
}

export function accountStatus(init: Partial<AccountStatus> = {}): AccountStatus {
  return {
    validity: init.validity ?? "unknown",
    premium: init.premium ?? false,
    expiresAt: init.expiresAt ?? null,
    trafficLeftBytes: init.trafficLeftBytes ?? null,
    message: init.message ?? null,
  };
}
