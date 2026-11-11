`krake` lädt Dateien direkt herunter oder steuert die Warteschlange eines Krake-Servers. Es ist ein Client: Der Server mit Web-Oberfläche ist `krake-server` (oder das Docker-Image).

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

Diese Befehle arbeiten auf der Warteschlange des Servers.

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

## Ein Server auf einem anderen Rechner

```bash
krake login https://krake.example.org --name home   # mit einem Server koppeln
krake remote list
krake remote use home
krake remote remove home
```

## Plugins

```bash
krake plugins list
krake plugins search <query>
krake plugins install <id>
krake plugins update [id]
krake plugins enable <id>
krake plugins login <id>                 # bei einem Hoster-Konto anmelden
krake plugins test <plugin.ts> [url]     # Spec eines Plugins ausführen oder eine URL damit auflösen
```

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
