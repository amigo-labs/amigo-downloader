import { parseArgs } from "./args.js";
import { runNew, runTest, runValidate } from "./commands.js";

const USAGE = `amigo-plugin <command> [options]

Commands:
  new <id> [--kind hoster|decrypter] [--dir .]
      Scaffold a new plugin project.
  validate --plugin <path>
      Load a compiled plugin and print its manifest.
  test <url> --plugin <path> [--fixtures <json-file>]
      Execute the plugin's extract/decrypt against <url> with a mock host.
      Optional fixtures JSON maps request URLs to response bodies.
`;

function fail(message: string): never {
  process.stderr.write(`${message}\n\n${USAGE}`);
  process.exit(2);
}

export async function main(argv: readonly string[]): Promise<void> {
  const [command, ...rest] = argv;
  if (!command || command === "--help" || command === "-h") {
    process.stdout.write(USAGE);
    return;
  }
  const parsed = parseArgs(rest);

  switch (command) {
    case "new": {
      const [id] = parsed.positional;
      if (!id) {
        fail("new: <id> is required");
      }
      const kind = (parsed.flags["kind"] ?? "hoster") as "hoster" | "decrypter";
      if (kind !== "hoster" && kind !== "decrypter") {
        fail(`new: --kind must be "hoster" or "decrypter"`);
      }
      const dir = typeof parsed.flags["dir"] === "string" ? parsed.flags["dir"] : ".";
      await runNew({ id, kind, dir });
      return;
    }
    case "validate": {
      const pluginPath = parsed.flags["plugin"];
      if (typeof pluginPath !== "string") {
        fail("validate: --plugin <path> is required");
      }
      await runValidate({ pluginPath });
      return;
    }
    case "test": {
      const [url] = parsed.positional;
      if (!url) {
        fail("test: <url> is required");
      }
      const pluginPath = parsed.flags["plugin"];
      if (typeof pluginPath !== "string") {
        fail("test: --plugin <path> is required");
      }
      const fixtures =
        typeof parsed.flags["fixtures"] === "string" ? parsed.flags["fixtures"] : undefined;
      await runTest(fixtures !== undefined ? { url, pluginPath, fixtures } : { url, pluginPath });
      return;
    }
    default: {
      fail(`unknown command: ${command}`);
    }
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(1);
  });
}
