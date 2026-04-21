export type Availability = "online" | "offline" | "unknown";

export interface FileHash {
  readonly algorithm: "md5" | "sha1" | "sha256" | "crc32" | string;
  readonly value: string;
}

export interface FileInfo {
  readonly filename: string | null;
  readonly size: number | null;
  readonly hash: FileHash | null;
  readonly availability: Availability;
  readonly mimeType: string | null;
}

export function fileInfo(partial: Partial<FileInfo> & { availability?: Availability }): FileInfo {
  return {
    filename: partial.filename ?? null,
    size: partial.size ?? null,
    hash: partial.hash ?? null,
    availability: partial.availability ?? "unknown",
    mimeType: partial.mimeType ?? null,
  };
}
