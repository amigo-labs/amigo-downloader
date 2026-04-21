import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { parseArgs } from "../../src/cli/args.js";
import { runNew } from "../../src/cli/commands.js";

describe("parseArgs", () => {
  it("collects positional and --flag pairs", () => {
    const args = parseArgs(["test", "https://a.test/", "--plugin", "./p.js"]);
    expect(args.positional).toEqual(["test", "https://a.test/"]);
    expect(args.flags["plugin"]).toBe("./p.js");
  });

  it("supports --flag=value and boolean flags", () => {
    const args = parseArgs(["--name=abc", "--verbose"]);
    expect(args.flags["name"]).toBe("abc");
    expect(args.flags["verbose"]).toBe(true);
  });
});

describe("runNew", () => {
  let workdir: string;
  beforeEach(() => {
    workdir = mkdtempSync(join(tmpdir(), "amigo-sdk-"));
  });
  afterEach(() => {
    rmSync(workdir, { recursive: true, force: true });
  });

  it("scaffolds a hoster plugin project", async () => {
    await runNew({ id: "my-host", kind: "hoster", dir: workdir });
    const root = join(workdir, "my-host");
    expect(existsSync(join(root, "package.json"))).toBe(true);
    expect(existsSync(join(root, "tsconfig.json"))).toBe(true);
    expect(existsSync(join(root, "plugin.toml"))).toBe(true);
    expect(existsSync(join(root, "src", "index.ts"))).toBe(true);
    const source = readFileSync(join(root, "src", "index.ts"), "utf8");
    expect(source).toContain("definePlugin");
    expect(source).toContain('id: "my-host"');
  });

  it("scaffolds a decrypter plugin project", async () => {
    await runNew({ id: "my-decrypt", kind: "decrypter", dir: workdir });
    const source = readFileSync(
      join(workdir, "my-decrypt", "src", "index.ts"),
      "utf8",
    );
    expect(source).toContain("defineDecrypter");
  });

  it("refuses to overwrite existing directory", async () => {
    await runNew({ id: "dup", kind: "hoster", dir: workdir });
    await expect(runNew({ id: "dup", kind: "hoster", dir: workdir })).rejects.toThrow(
      /already exists/,
    );
  });
});
