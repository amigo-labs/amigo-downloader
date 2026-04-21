import type { HostApi } from "./api.js";

let currentHostApi: HostApi | null = null;

export function setHostApi(api: HostApi): void {
  currentHostApi = api;
}

export function getHostApi(): HostApi {
  if (currentHostApi === null) {
    throw new Error(
      "HostApi not initialized. Call setHostApi() before using SDK features, or rely on the default runtime to install it.",
    );
  }
  return currentHostApi;
}

export function clearHostApi(): void {
  currentHostApi = null;
}

export function hasHostApi(): boolean {
  return currentHostApi !== null;
}
