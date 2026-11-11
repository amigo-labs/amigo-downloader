`krake` downloads files directly, or controls the queue of a Krake server. It is a client: the server with the Web UI is `krake-server` (or the Docker image).

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

These commands work on the server's queue.

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

## A server on another machine

```bash
krake login https://krake.example.org --name home   # pair with a server
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
krake plugins login <id>                 # sign in to a hoster account
krake plugins test <plugin.ts> [url]     # run a plugin's spec, or resolve a URL with it
```

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
