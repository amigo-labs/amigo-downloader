Die meisten Einstellungen stehen auf der Seite „Settings“ der Web-Oberfläche. Gespeichert werden sie in `config.toml`, die der Server beim ersten Start anlegt; du kannst die Datei auch von Hand ändern und den Server danach neu starten.

## Die Datei

```toml
download_dir = "downloads"
temp_dir = "downloads/.tmp"
max_concurrent_downloads = 10

[http]
max_chunks_per_download = 8
max_connections_per_host = 4
timeout_connect_secs = 30
timeout_read_secs = 120

[bandwidth]
global_limit = 0          # Bytes pro Sekunde, 0 = unbegrenzt
schedule_enabled = true

[[bandwidth.schedules]]
name = "Night"
start = "01:00"
end = "07:00"
limit = 0                 # unbegrenzt

[[bandwidth.schedules]]
name = "Day"
start = "07:00"
end = "01:00"
limit = 5000000           # 5 MB/s

[postprocessing]
auto_extract = true       # RAR, ZIP, 7z, tar.gz entpacken
delete_archives = true

[retry]
max_retries = 5
base_delay_secs = 1.0
max_delay_secs = 60.0

[update]
auto_check = true
check_interval_hours = 24
auto_update_plugins = false

[features]
usenet = false            # NZB-Import, NNTP-Server, Überwachungsordner
rss_feeds = false
```

## Geschwindigkeitslimits

`global_limit` begrenzt alle Downloads zusammen. Mit `schedule_enabled` bestimmt stattdessen der Eintrag, in dessen Zeitspanne die aktuelle Uhrzeit fällt, das Limit, zum Beispiel nachts unbegrenzt und tagsüber 5 MB/s. In der Seitenleiste der Web-Oberfläche gibt es einen Schalter für ein vorübergehendes Limit.

## Usenet

Usenet ist standardmäßig aus. Schalte `features.usenet` ein (oder den Schalter auf der Seite „Settings“) und trage dort deine NNTP-Server ein. Krake importiert NZB-Dateien, dekodiert yEnc und prüft und repariert mit PAR2.

## Plugins

Plugins sind TypeScript-Dateien. Der Server lädt beim Start die aus seinem Ordner `plugins`; die Seite „Plugins“ installiert und aktualisiert sie aus der Registry. Jedes Plugin läuft abgeschottet in WebAssembly und erreicht das Netz nur über Krake. Ein Plugin, das seine Hosts angibt, erreicht nur diese, und bei der Installation siehst du, was es verlangt; eines ohne Angabe ist als „Unscoped“ markiert.

Für ein eigenes Plugin: Starte mit [`plugins/template/plugin.ts`](https://github.com/amigo-labs/amigo-krake/blob/main/plugins/template/plugin.ts) und lies die [Plugin-API](https://github.com/amigo-labs/amigo-krake/blob/main/docs/plugin-api.md).

## Andere Werkzeuge

- **Browser-Erweiterungen:** Click'n'Load lauscht auf Port 9666, wie bei JDownloader.
- **NZBGet-Clients:** Der Server beantwortet NZBGets JSON-RPC unter `/jsonrpc`.
- **Skripte:** eine REST-API unter `/api/v1/` und ein WebSocket unter `/api/v1/ws` für den Fortschritt in Echtzeit.
