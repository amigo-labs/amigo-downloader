import type { PluginContext } from "../context/context.js";
import type { AccountCredentials, AccountStatus, Session } from "./types.js";

export interface AccountConfig {
  login(context: PluginContext, credentials: AccountCredentials): Promise<Session>;
  check(context: PluginContext, session: Session): Promise<AccountStatus>;
  logout?(context: PluginContext, session: Session): Promise<void>;
}
