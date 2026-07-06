// Lightweight i18n for the web UI
import { writable, get } from "svelte/store";

export type Locale = "en" | "de";

export type TParams = Record<string, string | number>;

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

    // Common
    "common.save": "Save",
    "common.cancel": "Cancel",
    "common.add": "Add",
    "common.test": "Test",
    "common.delete": "Delete",
    "common.close": "Close",
    "common.adding": "Adding…",
    "common.saving": "Saving…",
    "common.on": "On",
    "common.off": "Off",

    // Add panel
    "add.title": "Add Download",
    "add.tab_url": "URL / Links",
    "add.tab_file": "File (NZB / DLC)",
    "add.placeholder": "Paste URL(s) here — one per line",
    "add.hint": "Ctrl+Enter to submit. Multiple URLs supported (one per line).",
    "add.submit": "Add Download",
    "add.links_one": "1 link detected",
    "add.links_many": "{count} links detected",
    "add.links_invalid": "{count} line(s) don't look like URLs",
    "add.drop_hint": "Drop file here or click to browse",
    "add.file_types": "NZB, DLC, or text file with URLs",
    "add.added": "Download added",
    "add.added_many": "{count} downloads added",
    "add.failed": "Failed to add download",
    "add.file_imported": "File imported",
    "add.file_failed": "Failed to import file",

    // Downloads page
    "downloads.search": "Search downloads...",
    "downloads.no_downloads": "No downloads yet",
    "downloads.add_first": "Add your first download",
    "downloads.add_hint": "Ctrl+N to add · Drag & drop supported",
    "downloads.selected": "selected",
    "downloads.select_all": "Select all",
    "downloads.clear_selection": "Clear selection",
    "downloads.sort_by": "Sort by",
    "downloads.grid_view": "Grid view",
    "downloads.list_view": "List view",
    "downloads.more": "More options",

    "filter.all": "All",
    "filter.downloading": "Downloading",
    "filter.queued": "Queued",
    "filter.paused": "Paused",
    "filter.completed": "Completed",
    "filter.failed": "Failed",
    "sort.status": "Status",
    "sort.name": "Name",
    "sort.size": "Size",
    "sort.date": "Date",

    // Empty states per filter
    "empty.downloading": "Nothing downloading right now",
    "empty.queued": "The queue is empty",
    "empty.paused": "No paused downloads",
    "empty.completed": "No completed downloads yet",
    "empty.failed": "No failed downloads — nice.",
    "empty.search": "No downloads match your search",

    // Batch
    "batch.pause": "Pause",
    "batch.resume": "Resume",
    "batch.delete": "Delete",
    "batch.confirm": "Sure?",
    "batch.paused": "Paused {count} downloads",
    "batch.resumed": "Resumed {count} downloads",
    "batch.deleted": "Deleted {count} downloads",
    "batch.paused_partial": "Paused {done}/{total} downloads ({failed} failed)",
    "batch.resumed_partial": "Resumed {done}/{total} downloads ({failed} failed)",
    "batch.deleted_partial": "Deleted {done}/{total} downloads ({failed} failed)",

    // Actions
    "action.pause": "Pause",
    "action.resume": "Resume",
    "action.delete": "Delete",
    "action.retry": "Retry",
    "action.report": "Report",
    "action.sure": "Sure?",
    "action.copy_url": "URL copied",
    "action.copy_failed": "Failed to copy",
    "action.open_browser": "Open in Browser",
    "action.copy": "Copy URL",
    "action.undo": "Undo",

    // Status
    "status.downloading": "downloading",
    "status.completed": "completed",
    "status.failed": "failed",
    "status.paused": "paused",
    "status.queued": "queued",

    // Toasts
    "toast.download_complete": "Download complete",
    "toast.download_failed": "Download failed",
    "toast.deleted_one": "Download deleted",
    "toast.delete_failed": "Failed to delete download",
    "toast.action_failed": "Action failed",

    // Side panel
    "panel.details": "Download Details",

    // Detail panel sections
    "detail.file_info": "File Info",
    "detail.protocol": "Protocol",
    "detail.size": "Size",
    "detail.progress": "Progress",
    "detail.chunks": "Chunks",
    "detail.chunks_paused": "Chunks (paused)",
    "detail.speed": "Speed",
    "detail.error": "Error",
    "detail.actions": "Actions",

    // Drop zone
    "drop.title": "Drop it like it's hot!",
    "drop.hint": "URLs, NZB, DLC, or text files",
    "drop.dlc_imported": "DLC imported",
    "drop.nzb_imported": "NZB imported",
    "drop.urls_added": "Added {count} URLs",
    "drop.import_failed": "Import failed",

    // Offline / connection-aware empty states
    "downloads.offline": "Server unreachable",
    "downloads.offline_hint": "Check that the amigo server is running, then reload.",

    // Plugins page
    "plugins.load_failed": "Failed to load plugins. Is the server running?",
    "plugins.core_update": "Core Update Available",
    "plugins.update": "Update",
    "plugins.updating": "Updating…",
    "plugins.update_started": "Update initiated — restart required.",
    "plugins.update_failed": "Update failed",
    "plugins.installed": "Installed Plugins",
    "plugins.none": "No plugins loaded.",
    "plugins.active": "Active",
    "plugins.disabled": "Disabled",
    "plugins.enabled_toast": "Plugin enabled",
    "plugins.disabled_toast": "Plugin disabled",
    "plugins.toggle_failed": "Failed to update plugin",
    "plugins.marketplace": "Plugin Marketplace",
    "plugins.marketplace_soon": "Plugin marketplace coming soon.",

    // RSS feed settings
    "rss.title": "RSS Feeds",
    "rss.hint": "Monitor RSS/Atom feeds for new NZB links. New items are automatically imported.",
    "rss.add": "+ Add Feed",
    "rss.name": "Feed Name",
    "rss.url": "Feed URL",
    "rss.category": "Category",
    "rss.interval": "Check Interval (min)",
    "rss.save": "Add Feed",
    "rss.added": "RSS feed added",
    "rss.add_failed": "Failed to add feed",
    "rss.removed": "Feed removed",
    "rss.remove_failed": "Failed to remove feed",
    "rss.empty": "No RSS feeds configured. Add a feed to automatically import NZBs.",
    "rss.category_prefix": "Category: {name}",
    "rss.every": "Every {minutes}m",

    // Usenet server settings
    "usenet.title": "Usenet Servers",
    "usenet.add": "+ Add Server",
    "usenet.name": "Name",
    "usenet.host": "Host",
    "usenet.port": "Port",
    "usenet.connections": "Connections",
    "usenet.priority": "Priority",
    "usenet.username": "Username",
    "usenet.password": "Password",
    "usenet.save": "Save Server",
    "usenet.added": "Server added",
    "usenet.add_failed": "Failed to add server",
    "usenet.removed": "Server removed",
    "usenet.remove_failed": "Failed to remove server",
    "usenet.empty": "No Usenet servers configured. Add a server to start downloading from Usenet.",
    "usenet.meta": "{count} connections · Priority {priority}",
    "usenet.stat_status": "Status",
    "usenet.stat_active": "Active",
    "usenet.stat_articles": "Articles",
    "usenet.stat_speed": "Speed",
    "usenet.idle": "Idle",

    // History page
    "history.completed": "Completed",
    "history.load_failed": "Failed to load history. Is the server running?",

    // Neon intensity labels (also used in the command palette)
    "intensity.off": "Off",
    "intensity.low": "Low",
    "intensity.mid": "Mid",
    "intensity.high": "High",
    "intensity.full": "Full",

    // Webhook form labels + toasts
    "webhook.name": "Name",
    "webhook.url": "URL",
    "webhook.secret": "Secret",
    "webhook.optional": "optional",
    "webhook.events": "Events",
    "webhook.events_hint": "comma-separated, * = all",
    "webhook.added": "Webhook added",
    "webhook.add_failed": "Failed to add webhook",
    "webhook.removed": "Webhook removed",
    "webhook.delete_failed": "Failed to delete webhook",
    "webhook.test_sent": "Test sent",
    "webhook.test_failed": "Test failed",
    "webhook.signed": "signed",

    // Feedback dialog
    "feedback.title": "Feedback",
    "feedback.crash_reported": "Crash auto-reported",
    "feedback.view_issue": "View issue",
    "feedback.report_bug": "Report a Bug",
    "feedback.request_feature": "Request a Feature",
    "feedback.opens_github": "Opens GitHub with pre-filled template",
    "feedback.auto_on": "Crashes are automatically reported.",
    "feedback.auto_off": "Set AMIGO_GITHUB_TOKEN for automatic crash reporting.",

    // History
    "history.empty": "No download history yet",
    "history.empty_hint": "Completed downloads will appear here",

    // Connection
    "connection.online": "Connected",
    "connection.offline": "Offline",

    // Captcha
    "captcha.title": "Solve Captcha",
    "captcha.enter": "Enter the characters you see",
    "captcha.solve": "Solve",
    "captcha.skip": "Skip",
    "captcha.time_left": "{seconds}s left",
    "captcha.solved": "Captcha submitted",
    "captcha.failed": "Failed to submit captcha",
    "captcha.expired": "Captcha timed out",

    // Settings — appearance
    "settings.appearance": "Appearance",
    "settings.theme": "Theme",
    "theme.dark": "Dark",
    "theme.light": "Light",
    "settings.color_palette": "Color Palette",
    "settings.neon_intensity": "Neon Intensity",

    // Settings — downloads
    "settings.downloads": "Downloads",
    "settings.download_dir": "Download Directory",
    "settings.max_concurrent": "Max Concurrent Downloads",
    "settings.speed_limit": "Global Speed Limit",
    "settings.speed_limit_hint": "B/s (0 = unlimited)",
    "settings.retry_behavior": "Retry Behavior",
    "settings.max_retries": "Max retries before giving up",
    "settings.initial_delay": "Initial delay (s)",
    "settings.max_delay": "Max delay (s)",

    // Settings — webhooks
    "settings.webhooks": "Webhooks",
    "settings.add_webhook": "Add Webhook",
    "settings.webhooks_empty":
      "No webhooks configured. Add one to receive notifications on Discord, Slack, Home Assistant, etc.",

    // Settings — misc
    "settings.language": "Language",
    "settings.about": "About",
    "settings.saved": "Settings saved",
    "settings.save_failed": "Failed to save settings",

    // Command palette
    "cmd.placeholder": "Type a command or search…",
    "cmd.no_results": "No matching commands",
    "cmd.group_navigate": "Navigate",
    "cmd.group_actions": "Actions",
    "cmd.group_appearance": "Appearance",
    "cmd.add_download": "Add download",
    "cmd.toggle_theme": "Toggle light / dark",
    "cmd.show_shortcuts": "Show keyboard shortcuts",
    "cmd.set_palette": "Palette: {name}",
    "cmd.set_intensity": "Neon intensity: {name}",
    "cmd.hint_open": "Open command palette",

    // Shortcuts
    "shortcuts.title": "Keyboard Shortcuts",
    "shortcuts.command_palette": "Command palette",
    "shortcuts.add": "Add download",
    "shortcuts.close": "Close panel / dialog",
    "shortcuts.navigate": "Navigate pages",
    "shortcuts.help": "Show this help",
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

    "common.save": "Speichern",
    "common.cancel": "Abbrechen",
    "common.add": "Hinzufügen",
    "common.test": "Testen",
    "common.delete": "Löschen",
    "common.close": "Schließen",
    "common.adding": "Wird hinzugefügt…",
    "common.saving": "Wird gespeichert…",
    "common.on": "An",
    "common.off": "Aus",

    "add.title": "Download hinzufügen",
    "add.tab_url": "URL / Links",
    "add.tab_file": "Datei (NZB / DLC)",
    "add.placeholder": "URL(s) hier einfügen — eine pro Zeile",
    "add.hint": "Strg+Enter zum Absenden. Mehrere URLs möglich (eine pro Zeile).",
    "add.submit": "Download hinzufügen",
    "add.links_one": "1 Link erkannt",
    "add.links_many": "{count} Links erkannt",
    "add.links_invalid": "{count} Zeile(n) sehen nicht wie URLs aus",
    "add.drop_hint": "Datei hierher ziehen oder klicken zum Auswählen",
    "add.file_types": "NZB, DLC oder Textdatei mit URLs",
    "add.added": "Download hinzugefügt",
    "add.added_many": "{count} Downloads hinzugefügt",
    "add.failed": "Download konnte nicht hinzugefügt werden",
    "add.file_imported": "Datei importiert",
    "add.file_failed": "Datei konnte nicht importiert werden",

    "downloads.search": "Downloads durchsuchen...",
    "downloads.no_downloads": "Noch keine Downloads",
    "downloads.add_first": "Ersten Download hinzufügen",
    "downloads.add_hint": "Strg+N zum Hinzufügen · Drag & Drop unterstützt",
    "downloads.selected": "ausgewählt",
    "downloads.select_all": "Alle auswählen",
    "downloads.clear_selection": "Auswahl aufheben",
    "downloads.sort_by": "Sortieren nach",
    "downloads.grid_view": "Kachelansicht",
    "downloads.list_view": "Listenansicht",
    "downloads.more": "Weitere Optionen",

    "filter.all": "Alle",
    "filter.downloading": "Lädt",
    "filter.queued": "Warteschlange",
    "filter.paused": "Pausiert",
    "filter.completed": "Fertig",
    "filter.failed": "Fehler",
    "sort.status": "Status",
    "sort.name": "Name",
    "sort.size": "Größe",
    "sort.date": "Datum",

    "empty.downloading": "Gerade läuft kein Download",
    "empty.queued": "Die Warteschlange ist leer",
    "empty.paused": "Keine pausierten Downloads",
    "empty.completed": "Noch keine abgeschlossenen Downloads",
    "empty.failed": "Keine fehlgeschlagenen Downloads — sehr gut.",
    "empty.search": "Keine Downloads passen zur Suche",

    "batch.pause": "Pause",
    "batch.resume": "Fortsetzen",
    "batch.delete": "Löschen",
    "batch.confirm": "Sicher?",
    "batch.paused": "{count} Downloads pausiert",
    "batch.resumed": "{count} Downloads fortgesetzt",
    "batch.deleted": "{count} Downloads gelöscht",
    "batch.paused_partial": "{done}/{total} Downloads pausiert ({failed} fehlgeschlagen)",
    "batch.resumed_partial": "{done}/{total} Downloads fortgesetzt ({failed} fehlgeschlagen)",
    "batch.deleted_partial": "{done}/{total} Downloads gelöscht ({failed} fehlgeschlagen)",

    "action.pause": "Pause",
    "action.resume": "Fortsetzen",
    "action.delete": "Löschen",
    "action.retry": "Wiederholen",
    "action.report": "Melden",
    "action.sure": "Sicher?",
    "action.copy_url": "URL kopiert",
    "action.copy_failed": "Kopieren fehlgeschlagen",
    "action.open_browser": "Im Browser öffnen",
    "action.copy": "URL kopieren",
    "action.undo": "Rückgängig",

    "status.downloading": "Lädt",
    "status.completed": "Fertig",
    "status.failed": "Fehler",
    "status.paused": "Pausiert",
    "status.queued": "Warteschl.",

    "toast.download_complete": "Download abgeschlossen",
    "toast.download_failed": "Download fehlgeschlagen",
    "toast.deleted_one": "Download gelöscht",
    "toast.delete_failed": "Download konnte nicht gelöscht werden",
    "toast.action_failed": "Aktion fehlgeschlagen",

    "panel.details": "Download-Details",

    "detail.file_info": "Dateiinfo",
    "detail.protocol": "Protokoll",
    "detail.size": "Größe",
    "detail.progress": "Fortschritt",
    "detail.chunks": "Chunks",
    "detail.chunks_paused": "Chunks (pausiert)",
    "detail.speed": "Geschwindigkeit",
    "detail.error": "Fehler",
    "detail.actions": "Aktionen",

    "drop.title": "Einfach fallen lassen!",
    "drop.hint": "URLs, NZB, DLC oder Textdateien",
    "drop.dlc_imported": "DLC importiert",
    "drop.nzb_imported": "NZB importiert",
    "drop.urls_added": "{count} URLs hinzugefügt",
    "drop.import_failed": "Import fehlgeschlagen",

    "downloads.offline": "Server nicht erreichbar",
    "downloads.offline_hint": "Prüfe, ob der amigo-Server läuft, und lade die Seite neu.",

    "plugins.load_failed": "Plugins konnten nicht geladen werden. Läuft der Server?",
    "plugins.core_update": "Core-Update verfügbar",
    "plugins.update": "Aktualisieren",
    "plugins.updating": "Wird aktualisiert…",
    "plugins.update_started": "Update gestartet — Neustart erforderlich.",
    "plugins.update_failed": "Update fehlgeschlagen",
    "plugins.installed": "Installierte Plugins",
    "plugins.none": "Keine Plugins geladen.",
    "plugins.active": "Aktiv",
    "plugins.disabled": "Deaktiviert",
    "plugins.enabled_toast": "Plugin aktiviert",
    "plugins.disabled_toast": "Plugin deaktiviert",
    "plugins.toggle_failed": "Plugin konnte nicht aktualisiert werden",
    "plugins.marketplace": "Plugin-Marktplatz",
    "plugins.marketplace_soon": "Plugin-Marktplatz kommt bald.",

    "rss.title": "RSS-Feeds",
    "rss.hint": "Überwacht RSS/Atom-Feeds auf neue NZB-Links. Neue Einträge werden automatisch importiert.",
    "rss.add": "+ Feed hinzufügen",
    "rss.name": "Feed-Name",
    "rss.url": "Feed-URL",
    "rss.category": "Kategorie",
    "rss.interval": "Prüfintervall (min)",
    "rss.save": "Feed hinzufügen",
    "rss.added": "RSS-Feed hinzugefügt",
    "rss.add_failed": "Feed konnte nicht hinzugefügt werden",
    "rss.removed": "Feed entfernt",
    "rss.remove_failed": "Feed konnte nicht entfernt werden",
    "rss.empty": "Keine RSS-Feeds konfiguriert. Füge einen Feed hinzu, um NZBs automatisch zu importieren.",
    "rss.category_prefix": "Kategorie: {name}",
    "rss.every": "Alle {minutes} min",

    "usenet.title": "Usenet-Server",
    "usenet.add": "+ Server hinzufügen",
    "usenet.name": "Name",
    "usenet.host": "Host",
    "usenet.port": "Port",
    "usenet.connections": "Verbindungen",
    "usenet.priority": "Priorität",
    "usenet.username": "Benutzername",
    "usenet.password": "Passwort",
    "usenet.save": "Server speichern",
    "usenet.added": "Server hinzugefügt",
    "usenet.add_failed": "Server konnte nicht hinzugefügt werden",
    "usenet.removed": "Server entfernt",
    "usenet.remove_failed": "Server konnte nicht entfernt werden",
    "usenet.empty": "Keine Usenet-Server konfiguriert. Füge einen Server hinzu, um von Usenet zu laden.",
    "usenet.meta": "{count} Verbindungen · Priorität {priority}",
    "usenet.stat_status": "Status",
    "usenet.stat_active": "Aktiv",
    "usenet.stat_articles": "Artikel",
    "usenet.stat_speed": "Geschw.",
    "usenet.idle": "Bereit",

    "history.completed": "Abgeschlossen",
    "history.load_failed": "Verlauf konnte nicht geladen werden. Läuft der Server?",

    "intensity.off": "Aus",
    "intensity.low": "Niedrig",
    "intensity.mid": "Mittel",
    "intensity.high": "Hoch",
    "intensity.full": "Voll",

    "webhook.name": "Name",
    "webhook.url": "URL",
    "webhook.secret": "Geheimnis",
    "webhook.optional": "optional",
    "webhook.events": "Ereignisse",
    "webhook.events_hint": "kommagetrennt, * = alle",
    "webhook.added": "Webhook hinzugefügt",
    "webhook.add_failed": "Webhook konnte nicht hinzugefügt werden",
    "webhook.removed": "Webhook entfernt",
    "webhook.delete_failed": "Webhook konnte nicht gelöscht werden",
    "webhook.test_sent": "Test gesendet",
    "webhook.test_failed": "Test fehlgeschlagen",
    "webhook.signed": "signiert",

    "feedback.title": "Feedback",
    "feedback.crash_reported": "Absturz automatisch gemeldet",
    "feedback.view_issue": "Issue ansehen",
    "feedback.report_bug": "Fehler melden",
    "feedback.request_feature": "Funktion vorschlagen",
    "feedback.opens_github": "Öffnet GitHub mit vorausgefülltem Template",
    "feedback.auto_on": "Abstürze werden automatisch gemeldet.",
    "feedback.auto_off": "AMIGO_GITHUB_TOKEN setzen für automatische Absturzberichte.",

    "history.empty": "Noch kein Download-Verlauf",
    "history.empty_hint": "Abgeschlossene Downloads erscheinen hier",

    "connection.online": "Verbunden",
    "connection.offline": "Offline",

    "captcha.title": "Captcha lösen",
    "captcha.enter": "Gib die angezeigten Zeichen ein",
    "captcha.solve": "Lösen",
    "captcha.skip": "Überspringen",
    "captcha.time_left": "{seconds}s übrig",
    "captcha.solved": "Captcha übermittelt",
    "captcha.failed": "Captcha konnte nicht übermittelt werden",
    "captcha.expired": "Captcha-Zeit abgelaufen",

    "settings.appearance": "Erscheinungsbild",
    "settings.theme": "Design",
    "theme.dark": "Dunkel",
    "theme.light": "Hell",
    "settings.color_palette": "Farbpalette",
    "settings.neon_intensity": "Neon-Intensität",

    "settings.downloads": "Downloads",
    "settings.download_dir": "Download-Verzeichnis",
    "settings.max_concurrent": "Max. gleichzeitige Downloads",
    "settings.speed_limit": "Globales Geschwindigkeitslimit",
    "settings.speed_limit_hint": "B/s (0 = unbegrenzt)",
    "settings.retry_behavior": "Wiederholungsverhalten",
    "settings.max_retries": "Max. Versuche vor Abbruch",
    "settings.initial_delay": "Anfangsverzögerung (s)",
    "settings.max_delay": "Max. Verzögerung (s)",

    "settings.webhooks": "Webhooks",
    "settings.add_webhook": "Webhook hinzufügen",
    "settings.webhooks_empty":
      "Keine Webhooks konfiguriert. Füge einen hinzu, um Benachrichtigungen auf Discord, Slack, Home Assistant usw. zu erhalten.",

    "settings.language": "Sprache",
    "settings.about": "Über",
    "settings.saved": "Einstellungen gespeichert",
    "settings.save_failed": "Einstellungen konnten nicht gespeichert werden",

    "cmd.placeholder": "Befehl eingeben oder suchen…",
    "cmd.no_results": "Keine passenden Befehle",
    "cmd.group_navigate": "Navigation",
    "cmd.group_actions": "Aktionen",
    "cmd.group_appearance": "Erscheinungsbild",
    "cmd.add_download": "Download hinzufügen",
    "cmd.toggle_theme": "Hell / Dunkel umschalten",
    "cmd.show_shortcuts": "Tastenkürzel anzeigen",
    "cmd.set_palette": "Palette: {name}",
    "cmd.set_intensity": "Neon-Intensität: {name}",
    "cmd.hint_open": "Befehlspalette öffnen",

    "shortcuts.title": "Tastenkürzel",
    "shortcuts.command_palette": "Befehlspalette",
    "shortcuts.add": "Download hinzufügen",
    "shortcuts.close": "Panel / Dialog schließen",
    "shortcuts.navigate": "Seiten wechseln",
    "shortcuts.help": "Diese Hilfe anzeigen",
  },
};

function interpolate(str: string, params?: TParams): string {
  if (!params) return str;
  return str.replace(/\{(\w+)\}/g, (_, k) =>
    k in params ? String(params[k]) : `{${k}}`,
  );
}

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

export function t(key: string, params?: TParams): string {
  const lang = get(locale);
  const str = translations[lang]?.[key] ?? translations.en[key] ?? key;
  return interpolate(str, params);
}

// Reactive version for Svelte components — pass the `$locale` store value.
export function tr(lang: Locale, key: string, params?: TParams): string {
  const str = translations[lang]?.[key] ?? translations.en[key] ?? key;
  return interpolate(str, params);
}
