`krake` lädt Dateien im Terminal herunter. Es arbeitet für sich: Einen laufenden Krake-Server steuert es nicht. Für die Warteschlange eines Servers nimmst du dessen Web-Oberfläche oder dessen REST-API.

## Direkt herunterladen

Gib eine oder mehrere URLs an. Sie werden sofort geladen, mit Fortschrittsbalken.

```bash
krake <URL> [URL...]
krake <URL> -o ./downloads     # Zielordner
krake <URL> --chunks 8         # acht parallele Chunks
```

| Option | Kurz | Standard | Bedeutung |
|--------|------|----------|-----------|
| `--output` | `-o` | `.` | Ordner, in den die Dateien kommen |
| `--chunks` | `-c` | `0` | Parallele Chunks pro Download; 0 wählt die Zahl nach Dateigröße |

## Warteschlange

Diese Befehle führen eine eigene Warteschlange, in einer Datei `krake.db` in dem Ordner, in dem du sie aufrufst. Die Downloads eines laufenden Servers zeigen oder ändern sie nicht.

```bash
krake add <URL>                # URL einreihen
krake add --nzb file.nzb       # NZB importieren
krake add --dlc links.dlc      # DLC-Container importieren
krake list                     # aktive Downloads
krake queue                    # die Warteschlange
krake status                   # Überblick: aktiv, wartend, Geschwindigkeit
krake speed                    # aktuelle Geschwindigkeit
krake pause <ID>
krake resume <ID>
krake cancel <ID>              # abbrechen und entfernen
```

## DLC-Container

```bash
krake add --dlc links.dlc            # importieren
krake export-dlc                     # alles exportieren
krake export-dlc --ids id1,id2       # einzelne Downloads exportieren
```

## Mit einem Server koppeln

```bash
krake login https://krake.example.org --name home   # mit einem Server koppeln
krake remote list
krake remote use home
krake remote remove home
```

Die Kopplung nutzt `krake plugins install`: Es installiert ein Plugin auf dem gekoppelten Server. Die Warteschlangen-Befehle oben bleiben lokal.

## Plugins

```bash
krake plugins install <id>               # aus der Registry installieren, auf diesem Rechner oder einem gekoppelten Server
krake plugins test <plugin.ts> [url]     # Spec eines Plugins ausführen oder eine URL damit auflösen
```

Plugins auflisten, suchen, aktualisieren und aktivieren sowie die Anmeldung bei einem Hoster-Konto erledigst du in der Web-Oberfläche (Seite „Plugins“); die passenden Unterbefehle von `krake plugins` verweisen vorerst nur dorthin.

## Einstellungen

```bash
krake config get <key>
krake config set <key> <value>
```

## Updates

```bash
krake update check
krake update apply --yes
```
