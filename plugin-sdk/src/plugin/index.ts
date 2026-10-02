export {
  HOST_API_VERSION,
  defineDecrypter,
  domainPatternProblem,
  definePlugin,
  type DecrypterPluginDefinition,
  type HosterPluginDefinition,
  type Plugin,
  type PluginKind,
  type PluginManifest,
  type PluginPermissions,
} from "./plugin.js";
export { compilePattern, matchesAny, type UrlPattern } from "./matching.js";
