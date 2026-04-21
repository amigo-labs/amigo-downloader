export interface PluginTemplateOptions {
  readonly id: string;
  readonly kind: "hoster" | "decrypter";
  readonly sdkVersion: string;
}

export function manifestToml(options: PluginTemplateOptions): string {
  return `id = "${options.id}"
name = "${options.id}"
version = "0.1.0"
kind = "${options.kind}"
sdk_version = "${options.sdkVersion}"
match = ["https://${options.id}.example/*"]
permissions = []
`;
}

export function pluginTs(options: PluginTemplateOptions): string {
  if (options.kind === "decrypter") {
    return `import { plugin, types } from "@amigo/plugin-sdk";

export default plugin.defineDecrypter({
  id: "${options.id}",
  version: "0.1.0",
  match: [/${options.id}\\.example/],
  async decrypt(context) {
    const page = await context.browser.getPage(context.url);
    const hrefs = page.find("a[href]").map((element) => element.attr("href") ?? "");
    return hrefs.filter((href) => href.length > 0);
  },
});
`;
  }
  return `import { plugin, types } from "@amigo/plugin-sdk";

export default plugin.definePlugin({
  id: "${options.id}",
  version: "0.1.0",
  match: [/${options.id}\\.example/],
  async extract(context) {
    const page = await context.browser.getPage(context.url);
    const fileUrl = page.regex(/https:\\/\\/cdn\\.[^"']+/).getMatch(0);
    if (!fileUrl) {
      throw new Error("download URL not found");
    }
    return [types.formatInfo({ url: fileUrl })];
  },
});
`;
}

export function tsconfigJson(): string {
  return `{
  "extends": null,
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "lib": ["ES2022", "DOM"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "declaration": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "verbatimModuleSyntax": true
  },
  "include": ["src/**/*"]
}
`;
}

export function packageJson(options: PluginTemplateOptions): string {
  return `{
  "name": "${options.id}",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "main": "./dist/index.js",
  "scripts": {
    "build": "tsc"
  },
  "devDependencies": {
    "@amigo/plugin-sdk": "^${options.sdkVersion}",
    "typescript": "^5.6.0"
  }
}
`;
}

export function readme(options: PluginTemplateOptions): string {
  return `# ${options.id}

An amigo-downloader plugin.

## Build

\`\`\`
npm install
npm run build
\`\`\`

## Test

\`\`\`
amigo-plugin test https://${options.id}.example/sample --plugin ./dist/index.js
\`\`\`
`;
}
