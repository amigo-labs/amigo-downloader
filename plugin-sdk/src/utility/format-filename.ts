const RESERVED_WINDOWS_NAMES = new Set([
  "con",
  "prn",
  "aux",
  "nul",
  "com1",
  "com2",
  "com3",
  "com4",
  "com5",
  "com6",
  "com7",
  "com8",
  "com9",
  "lpt1",
  "lpt2",
  "lpt3",
  "lpt4",
  "lpt5",
  "lpt6",
  "lpt7",
  "lpt8",
  "lpt9",
]);

export interface FormatFilenameOptions {
  readonly maxLength?: number;
  readonly replacement?: string;
  readonly normaliseDiacritics?: boolean;
}

const DEFAULT_MAX_LENGTH = 200;
const DEFAULT_REPLACEMENT = "_";
// Disallowed on Windows: <, >, :, ", /, \, |, ?, * plus ASCII control chars.
const INVALID_CHARS = new RegExp("[<>:\"/\\\\|?*\\u0000-\\u001f]", "g");
const COMBINING_MARKS = new RegExp("[\\u0300-\\u036f]", "g");

function stripDiacritics(value: string): string {
  return value.normalize("NFD").replace(COMBINING_MARKS, "");
}

function truncatePreservingExtension(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }
  const dot = value.lastIndexOf(".");
  if (dot <= 0 || value.length - dot > 10) {
    return value.slice(0, maxLength);
  }
  const ext = value.slice(dot);
  const base = value.slice(0, dot);
  const baseLimit = Math.max(1, maxLength - ext.length);
  return base.slice(0, baseLimit) + ext;
}

export function formatFilename(unsafe: string, options: FormatFilenameOptions = {}): string {
  const replacement = options.replacement ?? DEFAULT_REPLACEMENT;
  const maxLength = options.maxLength ?? DEFAULT_MAX_LENGTH;
  let value = unsafe.trim();
  if (options.normaliseDiacritics) {
    value = stripDiacritics(value);
  }
  value = value.replace(INVALID_CHARS, replacement);
  value = value.replace(/\s+/g, " ").trim();
  value = value.replace(/\.+$/g, "");
  if (value.length === 0) {
    return replacement;
  }
  const reservedCheck = value.replace(/\..*$/, "").toLowerCase();
  if (RESERVED_WINDOWS_NAMES.has(reservedCheck)) {
    value = replacement + value;
  }
  return truncatePreservingExtension(value, maxLength);
}
