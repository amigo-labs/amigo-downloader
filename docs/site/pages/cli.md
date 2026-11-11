`krake` downloads files from the terminal. It works on its own: it does not control a running Krake server. For a server's queue, use its Web UI or its REST API.

## Download directly

Pass one or more URLs. They download straight away, with a progress bar.

```bash
krake <URL> [URL...]
krake <URL> -o ./downloads     # output folder
krake <URL> --chunks 8         # eight parallel chunks
```

| Option | Short | Default | Meaning |
|--------|-------|---------|---------|
| `--output` | `-o` | `.` | Folder the files go to |
| `--chunks` | `-c` | `0` | Parallel chunks per download; 0 picks a number from the file size |

## Queue

These commands keep a queue of their own, in a `krake.db` file in the folder you run them in. They do not show or change the downloads of a running server.

```bash
krake add <URL>                # queue a URL
krake add --nzb file.nzb       # import an NZB
krake add --dlc links.dlc      # import a DLC container
krake list                     # active downloads
krake queue                    # the queue
krake status                   # overview: active, queued, speed
krake speed                    # current speed
krake pause <ID>
krake resume <ID>
krake cancel <ID>              # cancel and remove
```

## DLC containers

```bash
krake add --dlc links.dlc            # import
krake export-dlc                     # export everything
krake export-dlc --ids id1,id2       # export some downloads
```

## Pairing with a server

```bash
krake login https://krake.example.org --name home   # pair with a server
krake remote list
krake remote use home
krake remote remove home
```

Pairing is used by `krake plugins install`, which installs a plugin on the paired server. The queue commands above stay local.

## Plugins

```bash
krake plugins install <id>               # install from the registry, on this machine or a paired server
krake plugins test <plugin.ts> [url]     # run a plugin's spec, or resolve a URL with it
```

Listing, searching, updating and enabling plugins and signing in to a hoster account are done in the Web UI (Plugins page); the matching `krake plugins` subcommands only point there for now.

## Settings

```bash
krake config get <key>
krake config set <key> <value>
```

## Updates

```bash
krake update check
krake update apply --yes
```
