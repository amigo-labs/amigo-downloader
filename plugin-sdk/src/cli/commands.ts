import { createRequire } from "node:module";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import type { Plugin } from "../plugin/plugin.js";
import { createPluginContext } from "../context/context.js";
import { createMockHostApi, setHostApi } from "../host/index.js";
import { SDK_VERSION } from "../index.js";
import {
  manifestToml,
  packageJson,
  pluginTs,
  readme,
  tsconfigJson,
} from "./templates.js";

export async function runNew(args: { id: string; kind: "hoster" | "decrypter"; dir: string }): Promise<void> {
  const target = resolve(args.dir, args.id);
  if (existsSync(target)) {
    throw new Error(`target directory already exists: ${target}`);
  }
  mkdirSync(join(target, "src"), { recursive: true });
  const options = { id: args.id, kind: args.kind, sdkVersion: SDK_VERSION };
  writeFileSync(join(target, "package.json"), packageJson(options));
  writeFileSync(join(target, "tsconfig.json"), tsconfigJson());
  writeFileSync(join(target, "plugin.toml"), manifestToml(options));
  writeFileSync(join(target, "README.md"), readme(options));
  writeFileSync(join(target, "src", "index.ts"), pluginTs(options));
  process.stdout.write(`Created plugin scaffold at ${target}\n`);
}

function isPlugin(candidate: unknown): candidate is Plugin {
  if (!candidate || typeof candidate !== "object") {
    return false;
  }
  const plugin = candidate as Partial<Plugin>;
  return (
    typeof plugin.id === "string" &&
    typeof plugin.version === "string" &&
    (plugin.kind === "hoster" || plugin.kind === "decrypter") &&
    Array.isArray(plugin.match) &&
    typeof plugin.matches === "function"
  );
}

async function loadPlugin(pluginPath: string): Promise<Plugin> {
  const absolute = resolve(pluginPath);
  const url = pathToFileURL(absolute).href;
  const module = (await import(url)) as { default?: unknown };
  const candidate = module.default ?? (module as unknown);
  if (!isPlugin(candidate)) {
    throw new Error(
      `module at ${pluginPath} does not export a valid Plugin as default`,
    );
  }
  return candidate;
}

export async function runValidate(args: { pluginPath: string }): Promise<void> {
  const plugin = await loadPlugin(args.pluginPath);
  process.stdout.write(
    JSON.stringify(
      {
        ok: true,
        manifest: plugin.manifest(),
        capabilities: {
          extract: typeof plugin.extract === "function",
          decrypt: typeof plugin.decrypt === "function",
          checkAvailable: typeof plugin.checkAvailable === "function",
          hasAccount: plugin.account !== null,
        },
      },
      null,
      2,
    ) + "\n",
  );
}

export async function runTest(args: {
  url: string;
  pluginPath: string;
  fixtures?: string;
}): Promise<void> {
  const plugin = await loadPlugin(args.pluginPath);
  if (!plugin.matches(args.url)) {
    throw new Error(`plugin ${plugin.id} does not match URL ${args.url}`);
  }

  const fixtureMap: Record<string, string> = args.fixtures
    ? ((): Record<string, string> => {
        const require = createRequire(import.meta.url);
        return require(resolve(args.fixtures)) as Record<string, string>;
      })()
    : {};

  const controller = createMockHostApi({
    http: (request) => {
      const canned = fixtureMap[request.url];
      return {
        status: canned ? 200 : 404,
        url: request.url,
        redirectLocation: null,
        headers: {},
        body: canned ? new TextEncoder().encode(canned) : new Uint8Array(),
      };
    },
  });
  setHostApi(controller.api);

  const logs: Array<Record<string, unknown>> = [];
  const context = createPluginContext({
    url: args.url,
    hostApi: controller.api,
    onLog: (record) =>
      logs.push({ level: record.level, message: record.message, metadata: record.metadata }),
  });

  if (plugin.kind === "hoster") {
    if (!plugin.extract) {
      throw new Error(`plugin ${plugin.id} has no extract function`);
    }
    const formats = await plugin.extract(context);
    process.stdout.write(
      JSON.stringify({ plugin: plugin.id, formats, logs }, null, 2) + "\n",
    );
    return;
  }

  if (!plugin.decrypt) {
    throw new Error(`plugin ${plugin.id} has no decrypt function`);
  }
  const links = await plugin.decrypt(context);
  process.stdout.write(
    JSON.stringify({ plugin: plugin.id, links, logs }, null, 2) + "\n",
  );
}
