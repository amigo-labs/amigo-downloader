// Lightweight i18n for the web UI
import { writable, get } from "svelte/store";

export type Locale = "en" | "de";

const translations: Record<Locale, Record<string, string>> = {
  en: {
    "nav.downloads": "Downloads",
    "nav.plugins": "Plugins",
    "nav.history": "History",
    "nav.settings": "Settings",
    "nav.management": "Management",
    "sidebar.add": "Add Download...",
    "sidebar.speed": "Speed",
    "sidebar.active": "Active",
    "sidebar.queued": "Queued",
    "sidebar.done": "Done",
    "sidebar.limit": "Limit",
    "settings.appearance": "Appearance",
    "settings.downloads": "Downloads",
    "settings.webhooks": "Webhooks",
    "settings.language": "Language",
    "settings.about": "About",
    "settings.saved": "Settings saved",
    "downloads.search": "Search downloads...",
    "downloads.no_downloads": "No downloads yet",
    "downloads.add_first": "Add your first download",
    "downloads.selected": "selected",
    "action.pause": "Pause",
    "action.resume": "Resume",
    "action.delete": "Delete",
    "action.retry": "Retry",
    "action.report": "Report",
    "action.sure": "Sure?",
    "action.copy_url": "URL copied",
    "status.downloading": "downloading",
    "status.completed": "completed",
    "status.failed": "failed",
    "status.paused": "paused",
    "status.queued": "queued",
    "toast.download_complete": "Download complete",
    "toast.download_failed": "Download failed",
    "history.empty": "No download history yet",
    "history.empty_hint": "Completed downloads will appear here",
    "connection.online": "Connected",
    "connection.offline": "Offline",
  },
  de: {
    "nav.downloads": "Downloads",
    "nav.plugins": "Plugins",
    "nav.history": "Verlauf",
    "nav.settings": "Einstellungen",
    "nav.management": "Verwaltung",
    "sidebar.add": "Download hinzufügen...",
    "sidebar.speed": "Geschw.",
    "sidebar.active": "Aktiv",
    "sidebar.queued": "Warteschl.",
    "sidebar.done": "Fertig",
    "sidebar.limit": "Limit",
    "settings.appearance": "Erscheinungsbild",
    "settings.downloads": "Downloads",
    "settings.webhooks": "Webhooks",
    "settings.language": "Sprache",
    "settings.about": "Über",
    "settings.saved": "Einstellungen gespeichert",
    "downloads.search": "Downloads durchsuchen...",
    "downloads.no_downloads": "Noch keine Downloads",
    "downloads.add_first": "Ersten Download hinzufügen",
    "downloads.selected": "ausgewählt",
    "action.pause": "Pause",
    "action.resume": "Fortsetzen",
    "action.delete": "Löschen",
    "action.retry": "Wiederholen",
    "action.report": "Melden",
    "action.sure": "Sicher?",
    "action.copy_url": "URL kopiert",
    "status.downloading": "Lädt",
    "status.completed": "Fertig",
    "status.failed": "Fehler",
    "status.paused": "Pausiert",
    "status.queued": "Warteschl.",
    "toast.download_complete": "Download abgeschlossen",
    "toast.download_failed": "Download fehlgeschlagen",
    "history.empty": "Noch kein Download-Verlauf",
    "history.empty_hint": "Abgeschlossene Downloads erscheinen hier",
    "connection.online": "Verbunden",
    "connection.offline": "Offline",
  },
};

function createLocaleStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("locale") : null;
  const initial: Locale = (stored as Locale) || "en";
  const { subscribe, set } = writable<Locale>(initial);
  return {
    subscribe,
    set(value: Locale) {
      if (typeof localStorage !== "undefined") localStorage.setItem("locale", value);
      set(value);
    },
  };
}

export const locale = createLocaleStore();

export function t(key: string): string {
  const lang = get(locale);
  return translations[lang]?.[key] ?? translations.en[key] ?? key;
}

// Reactive version for Svelte components
export function tr(lang: Locale, key: string): string {
  return translations[lang]?.[key] ?? translations.en[key] ?? key;
}
