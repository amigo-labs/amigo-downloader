//! Daemon side of the out-of-process plugin host (#79).
//!
//! [`ProcessHost`] spawns `amigo-plugin-host` lazily, speaks [`crate::ipc`]
//! with it, executes its host callbacks against the plugins' scoped
//! [`HostApi`]s, and replaces it when it dies:
//!
//! - **Crash.** When the child's stdout closes, every in-flight request fails
//!   with [`crate::Error::HostCrashed`] and the child is marked dead. The next
//!   request spawns a new one (a new *generation*); the loader re-loads each
//!   plugin into it on first use. Cookies and storage live here and survive.
//! - **Crash loop.** After [`MAX_SPAWNS_PER_WINDOW`] spawns within
//!   [`SPAWN_WINDOW`], spawning pauses for [`SPAWN_PAUSE`].
//! - **Liveness.** The plugin's deadline is enforced inside the child. Here a
//!   watchdog only kills a child that shows no activity on a request for its
//!   budget plus a grace period while no host callback is outstanding — so a
//!   `solveCaptcha` waiting on a human never trips it.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::sync::{Notify, mpsc, oneshot};
use tracing::{debug, error, warn};

use crate::host_api::HostApi;
use crate::host_fns::{self, HostArgs, HostResult};
use crate::ipc::{self, Handle, Payload, Request, ToChild, ToParent};

/// File name of the plugin host binary.
pub(crate) const HOST_BINARY: &str = "amigo-plugin-host";

/// Overrides where the plugin host binary is looked up.
pub(crate) const HOST_BINARY_ENV: &str = "AMIGO_PLUGIN_HOST_BIN";

const MAX_SPAWNS_PER_WINDOW: usize = 5;
const SPAWN_WINDOW: Duration = Duration::from_mins(1);
const SPAWN_PAUSE: Duration = Duration::from_secs(30);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const WATCHDOG_TICK: Duration = Duration::from_millis(250);
pub(crate) const DEFAULT_WATCHDOG_GRACE: Duration = Duration::from_secs(10);

/// Spawns, supervises and replaces the plugin host child.
pub(crate) struct ProcessHost {
    binary: Option<PathBuf>,
    current: tokio::sync::Mutex<Option<Arc<Child>>>,
    generation: AtomicU64,
    spawns: Mutex<VecDeque<Instant>>,
    watchdog_grace_ms: AtomicU64,
}

impl ProcessHost {
    /// `binary` overrides the lookup described in [`locate_binary`].
    pub(crate) fn new(binary: Option<PathBuf>) -> Self {
        Self {
            binary,
            current: tokio::sync::Mutex::new(None),
            generation: AtomicU64::new(0),
            spawns: Mutex::new(VecDeque::new()),
            watchdog_grace_ms: AtomicU64::new(DEFAULT_WATCHDOG_GRACE.as_millis() as u64),
        }
    }

    pub(crate) fn set_watchdog_grace(&self, grace: Duration) {
        self.watchdog_grace_ms
            .store(grace.as_millis() as u64, Ordering::Relaxed);
    }

    pub(crate) fn watchdog_grace(&self) -> Duration {
        Duration::from_millis(self.watchdog_grace_ms.load(Ordering::Relaxed))
    }

    /// The live child, if one is running. Never spawns.
    pub(crate) async fn live(&self) -> Option<Arc<Child>> {
        let current = self.current.lock().await;
        current.as_ref().filter(|c| c.is_alive()).cloned()
    }

    /// PID of the live child, if one is running.
    pub(crate) async fn pid(&self) -> Option<u32> {
        self.live().await.and_then(|c| c.pid)
    }

    /// The live child, spawning a new one if there is none.
    pub(crate) async fn child(&self) -> Result<Arc<Child>, crate::Error> {
        let mut current = self.current.lock().await;
        if let Some(child) = current.as_ref()
            && child.is_alive()
        {
            return Ok(Arc::clone(child));
        }
        self.check_spawn_budget()?;
        let binary = match &self.binary {
            Some(path) => path.clone(),
            None => locate_binary()?,
        };
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let child = Child::spawn(&binary, generation).await?;
        debug!(
            "Started plugin host generation {generation} (pid {:?})",
            child.pid
        );
        *current = Some(Arc::clone(&child));
        Ok(child)
    }

    fn check_spawn_budget(&self) -> Result<(), crate::Error> {
        let now = Instant::now();
        let mut spawns = self.spawns.lock().unwrap_or_else(|e| e.into_inner());
        while spawns
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) > SPAWN_WINDOW)
        {
            spawns.pop_front();
        }
        if spawns.len() >= MAX_SPAWNS_PER_WINDOW
            && let Some(last) = spawns.back()
        {
            let since = now.saturating_duration_since(*last);
            if since < SPAWN_PAUSE {
                return Err(crate::Error::HostCrashed(format!(
                    "plugin host restarted {} times within {}s; next attempt in {}s",
                    spawns.len(),
                    SPAWN_WINDOW.as_secs(),
                    (SPAWN_PAUSE.saturating_sub(since)).as_secs().max(1)
                )));
            }
        }
        spawns.push_back(now);
        Ok(())
    }
}

/// Find `amigo-plugin-host`: `$AMIGO_PLUGIN_HOST_BIN`, else next to the
/// current executable, else one level up when the executable lives in
/// Cargo's `deps/` (integration-test binaries).
fn locate_binary() -> Result<PathBuf, crate::Error> {
    if let Some(path) = std::env::var_os(HOST_BINARY_ENV) {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe()
        .map_err(|e| crate::Error::Other(format!("cannot locate the current executable: {e}")))?;
    let name = format!("{HOST_BINARY}{}", std::env::consts::EXE_SUFFIX);
    let dir = exe.parent().unwrap_or(exe.as_path());
    let mut candidates = vec![dir.join(&name)];
    if dir.file_name().is_some_and(|n| n == "deps")
        && let Some(up) = dir.parent()
    {
        candidates.push(up.join(&name));
    }
    candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| {
        crate::Error::Other(format!(
            "plugin host binary `{name}` not found next to {}; plugins cannot run. \
             Install it alongside the daemon or set {HOST_BINARY_ENV}.",
            exe.display()
        ))
    })
}

/// A plugin's daemon-side identity, looked up by the handle the child
/// stamps on each host call. The child never names a plugin id itself.
struct Scope {
    plugin_id: String,
    host: HostApi,
}

/// Progress of one request, for the watchdog.
struct Activity {
    last: Mutex<Instant>,
    host_calls: AtomicU32,
}

impl Activity {
    fn touch(&self) {
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.last
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
    }
}

struct Pending {
    reply: oneshot::Sender<Result<Payload, crate::Error>>,
    plugin_id: String,
    activity: Arc<Activity>,
}

#[derive(Default)]
struct State {
    alive: bool,
    /// Why the daemon killed the child, if it did.
    kill_reason: Option<String>,
    pending: HashMap<u64, Pending>,
    scopes: HashMap<Handle, Scope>,
}

/// One running plugin host process.
pub(crate) struct Child {
    pub(crate) generation: u64,
    pid: Option<u32>,
    writer: mpsc::UnboundedSender<Vec<u8>>,
    shared: Arc<Shared>,
    next_id: AtomicU64,
}

/// State shared with the reader task, which must not keep the writer (and
/// thereby the child's stdin) alive once the `Child` is dropped.
struct Shared {
    state: Mutex<State>,
    kill: Notify,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn kill(&self, reason: String) {
        {
            let mut state = self.lock();
            if !state.alive {
                return;
            }
            state.kill_reason.get_or_insert(reason);
        }
        self.kill.notify_one();
    }
}

impl Child {
    async fn spawn(binary: &PathBuf, generation: u64) -> Result<Arc<Self>, crate::Error> {
        let mut cmd = tokio::process::Command::new(binary);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            // The child needs nothing from the daemon's environment: no
            // credentials, no config paths.
            .env_clear()
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.current_dir("/");

        let mut process = cmd.spawn().map_err(|e| {
            crate::Error::Other(format!(
                "failed to start plugin host {}: {e}",
                binary.display()
            ))
        })?;
        let pid = process.id();
        let mut stdin = process
            .stdin
            .take()
            .ok_or_else(|| crate::Error::Other("plugin host has no stdin pipe".into()))?;
        let mut stdout = process
            .stdout
            .take()
            .ok_or_else(|| crate::Error::Other("plugin host has no stdout pipe".into()))?;

        let hello = tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            ipc::read_frame_async::<_, ToParent>(&mut stdout),
        )
        .await;
        match hello {
            Ok(Ok(Some(ToParent::Hello { protocol }))) if protocol == ipc::PROTOCOL_VERSION => {}
            Ok(Ok(Some(ToParent::Hello { protocol }))) => {
                return Err(crate::Error::Other(format!(
                    "plugin host {} speaks protocol {protocol}, expected {}; \
                     it does not belong to this build",
                    binary.display(),
                    ipc::PROTOCOL_VERSION
                )));
            }
            other => {
                return Err(crate::Error::Other(format!(
                    "plugin host {} did not complete the handshake: {other:?}",
                    binary.display()
                )));
            }
        }

        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                alive: true,
                ..State::default()
            }),
            kill: Notify::new(),
        });

        let (writer, mut outbox) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(frame) = outbox.recv().await {
                if stdin.write_all(&frame).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
            // Dropping stdin tells the child to exit.
        });

        let (status_tx, status_rx) = oneshot::channel();
        let supervisor = Arc::clone(&shared);
        tokio::spawn(async move {
            let status = tokio::select! {
                status = process.wait() => status,
                () = supervisor.kill.notified() => {
                    let _ = process.start_kill();
                    process.wait().await
                }
            };
            let _ = status_tx.send(status);
        });

        let reader_shared = Arc::clone(&shared);
        let reader_writer = writer.downgrade();
        tokio::spawn(async move {
            loop {
                match ipc::read_frame_async::<_, ToParent>(&mut stdout).await {
                    Ok(Some(msg)) => route(&reader_shared, &reader_writer, msg),
                    Ok(None) => break,
                    Err(e) => {
                        error!("Plugin host generation {generation} sent a bad frame: {e}");
                        reader_shared.kill(format!("protocol error: {e}"));
                        break;
                    }
                }
            }
            reader_shared.kill.notify_one();
            let status = tokio::time::timeout(Duration::from_secs(5), status_rx)
                .await
                .ok()
                .and_then(Result::ok);
            fail_all(&reader_shared, generation, status);
        });

        Ok(Arc::new(Self {
            generation,
            pid,
            writer,
            shared,
            next_id: AtomicU64::new(1),
        }))
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.shared.lock().alive
    }

    /// Bind a loaded handle to the plugin's daemon-side identity. Host calls
    /// on that handle run as `plugin_id` against `host`.
    pub(crate) fn register(&self, handle: Handle, plugin_id: &str, host: HostApi) {
        self.shared.lock().scopes.insert(
            handle,
            Scope {
                plugin_id: plugin_id.to_string(),
                host,
            },
        );
    }

    /// Free a handle in the child.
    pub(crate) fn drop_handle(&self, handle: Handle) {
        self.shared.lock().scopes.remove(&handle);
        if let Ok(frame) = ipc::encode_frame(&ToChild::Drop { handle }) {
            let _ = self.writer.send(frame);
        }
    }

    /// Send `body` and wait for its response. `budget` is how long the child
    /// may legitimately run the request between host callbacks; the watchdog
    /// kills the child after `budget + grace` without progress.
    pub(crate) async fn request(
        &self,
        body: Request,
        plugin_id: &str,
        budget: Duration,
        grace: Duration,
    ) -> Result<Payload, crate::Error> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let activity = Arc::new(Activity {
            last: Mutex::new(Instant::now()),
            host_calls: AtomicU32::new(0),
        });
        let (reply, mut response) = oneshot::channel();
        {
            let mut state = self.shared.lock();
            if !state.alive {
                return Err(crate::Error::HostCrashed(
                    "plugin host exited before the call started".into(),
                ));
            }
            state.pending.insert(
                id,
                Pending {
                    reply,
                    plugin_id: plugin_id.to_string(),
                    activity: Arc::clone(&activity),
                },
            );
        }

        let frame = match ipc::encode_frame(&ToChild::Request { id, body }) {
            Ok(frame) => frame,
            Err(e) => {
                self.shared.lock().pending.remove(&id);
                return Err(crate::Error::Execution(format!(
                    "cannot encode request: {e}"
                )));
            }
        };
        if self.writer.send(frame).is_err() {
            self.shared.lock().pending.remove(&id);
            return Err(crate::Error::HostCrashed("plugin host input closed".into()));
        }

        let limit = budget.saturating_add(grace);
        let mut tick = tokio::time::interval(WATCHDOG_TICK);
        loop {
            tokio::select! {
                result = &mut response => {
                    return result.unwrap_or_else(|_| {
                        Err(crate::Error::HostCrashed("plugin host dropped the call".into()))
                    });
                }
                _ = tick.tick() => {
                    if activity.host_calls.load(Ordering::Relaxed) == 0
                        && activity.idle_for() > limit
                    {
                        warn!(
                            "Plugin host unresponsive: plugin {plugin_id} made no progress \
                             for {}s; killing it",
                            limit.as_secs()
                        );
                        self.shared.kill(format!(
                            "unresponsive: plugin {plugin_id} made no progress for {}s",
                            limit.as_secs()
                        ));
                    }
                }
            }
        }
    }
}

/// Handle one frame from the child.
fn route(shared: &Arc<Shared>, writer: &mpsc::WeakUnboundedSender<Vec<u8>>, msg: ToParent) {
    match msg {
        ToParent::Response { id, result } => {
            let pending = shared.lock().pending.remove(&id);
            if let Some(pending) = pending {
                let _ = pending.reply.send(result);
            }
        }
        ToParent::HostCall {
            call_id,
            request,
            handle,
            name,
            args,
        } => {
            let (scope, activity) = {
                let state = shared.lock();
                let scope = state
                    .scopes
                    .get(&handle)
                    .map(|s| (s.plugin_id.clone(), s.host.clone()));
                let activity = state.pending.get(&request).map(|p| Arc::clone(&p.activity));
                (scope, activity)
            };
            let Some(writer) = writer.upgrade() else {
                return;
            };
            let Some((plugin_id, host)) = scope else {
                reply(
                    &writer,
                    call_id,
                    Err(format!("unknown plugin handle {handle}")),
                );
                return;
            };
            if let Err(e) = check_call(&name, &args) {
                reply(&writer, call_id, Err(e));
                return;
            }
            if let Some(a) = &activity {
                a.host_calls.fetch_add(1, Ordering::Relaxed);
            }
            tokio::spawn(async move {
                let result = host.dispatch(&plugin_id, &name, &args).await;
                if let Some(a) = &activity {
                    a.touch();
                    a.host_calls.fetch_sub(1, Ordering::Relaxed);
                }
                reply(&writer, call_id, result);
            });
        }
        ToParent::Hello { .. } => {}
    }
}

/// Refuse anything the table does not route to the daemon, with the arity
/// the table declares. The child is not trusted to have checked.
fn check_call(name: &str, args: &HostArgs) -> Result<(), String> {
    let f =
        host_fns::parent_fn(name).ok_or_else(|| format!("{name} is not a daemon host function"))?;
    if args.len() != f.required + f.optional || args[..f.required].iter().any(Option::is_none) {
        return Err(format!("{name}: wrong arguments"));
    }
    Ok(())
}

fn reply(writer: &mpsc::UnboundedSender<Vec<u8>>, call_id: u64, result: HostResult) {
    match ipc::encode_frame(&ToChild::HostReply { call_id, result }) {
        Ok(frame) => {
            let _ = writer.send(frame);
        }
        Err(e) => {
            // Too large to send: tell the plugin instead of leaving it blocked.
            if let Ok(frame) = ipc::encode_frame(&ToChild::HostReply {
                call_id,
                result: Err(format!("host result could not be sent: {e}")),
            }) {
                let _ = writer.send(frame);
            }
        }
    }
}

/// The child is gone: fail every in-flight request.
fn fail_all(
    shared: &Shared,
    generation: u64,
    status: Option<std::io::Result<std::process::ExitStatus>>,
) {
    let (pending, reason) = {
        let mut state = shared.lock();
        state.alive = false;
        state.scopes.clear();
        let reason = state.kill_reason.take().unwrap_or_else(|| match &status {
            Some(Ok(status)) => format!("exited ({status})"),
            Some(Err(e)) => format!("exited (status unavailable: {e})"),
            None => "closed its output".to_string(),
        });
        (std::mem::take(&mut state.pending), reason)
    };
    if pending.is_empty() {
        debug!("Plugin host generation {generation} {reason}");
        return;
    }
    for (_, p) in pending {
        error!(
            "Plugin host generation {generation} {reason} while running plugin {}",
            p.plugin_id
        );
        let _ = p.reply.send(Err(crate::Error::HostCrashed(format!(
            "plugin host {reason} while running plugin {}",
            p.plugin_id
        ))));
    }
}
