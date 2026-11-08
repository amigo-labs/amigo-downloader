//! One-time moves of files and directories that still carry their
//! pre-rename `amigo` names (`amigo.db`, `~/.config/amigo`, …).
//!
//! Each move happens only when the new path does not exist yet and the old
//! one does, so calling these on every start is cheap and idempotent. A move
//! is a rename; only when that fails because old and new live on different
//! filesystems is the data copied and the original removed.

use std::io;
use std::path::Path;

use tracing::info;

/// SQLite companion files that must travel with a database file.
const DB_SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// Move `old` (file or directory) to `new` if `new` does not exist yet and
/// `old` does. Returns whether anything was moved.
pub fn migrate_path(old: &Path, new: &Path) -> io::Result<bool> {
    if exists(new) || !exists(old) {
        return Ok(false);
    }
    move_path(old, new)?;
    info!("Moved {} to {}", old.display(), new.display());
    Ok(true)
}

/// [`migrate_path`] for a SQLite database: the `-wal`, `-shm` and
/// `-journal` files next to `old` move along with it, so no committed
/// transaction is left behind.
pub fn migrate_db(old: &Path, new: &Path) -> io::Result<bool> {
    if exists(new) || !exists(old) {
        return Ok(false);
    }
    for suffix in DB_SIDECARS {
        let old_side = with_suffix(old, suffix);
        let new_side = with_suffix(new, suffix);
        if exists(&old_side) && !exists(&new_side) {
            move_path(&old_side, &new_side)?;
        }
    }
    move_path(old, new)?;
    info!("Moved database {} to {}", old.display(), new.display());
    Ok(true)
}

fn exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

fn with_suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

fn move_path(old: &Path, new: &Path) -> io::Result<()> {
    if let Some(parent) = new.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(old, new) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => copy_then_remove(old, new),
        Err(e) => Err(e),
    }
}

/// Copy `old` to `new`, then remove `old`. On a failed copy the partial
/// `new` is removed again and `old` stays untouched.
fn copy_then_remove(old: &Path, new: &Path) -> io::Result<()> {
    let is_dir = old.symlink_metadata()?.is_dir();
    let copied = if is_dir {
        copy_dir(old, new)
    } else {
        std::fs::copy(old, new).map(|_| ())
    };
    if let Err(e) = copied {
        let _ = if is_dir {
            std::fs::remove_dir_all(new)
        } else {
            std::fs::remove_file(new)
        };
        return Err(e);
    }
    if is_dir {
        std::fs::remove_dir_all(old)
    } else {
        std::fs::remove_file(old)
    }
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_path_moves_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("amigo");
        let new = tmp.path().join("krake");
        std::fs::create_dir(&old).unwrap();
        std::fs::write(old.join("remotes.toml"), "x = 1").unwrap();

        assert!(migrate_path(&old, &new).unwrap());
        assert!(!old.exists());
        assert_eq!(
            std::fs::read_to_string(new.join("remotes.toml")).unwrap(),
            "x = 1"
        );
    }

    #[test]
    fn migrate_path_creates_missing_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join(".amigo-tmp");
        let new = tmp.path().join("a").join("b").join(".krake-tmp");
        std::fs::write(&old, "part").unwrap();

        assert!(migrate_path(&old, &new).unwrap());
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "part");
    }

    #[test]
    fn migrate_path_keeps_existing_new_path() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("amigo");
        let new = tmp.path().join("krake");
        std::fs::create_dir(&old).unwrap();
        std::fs::create_dir(&new).unwrap();
        std::fs::write(new.join("remotes.toml"), "new").unwrap();

        assert!(!migrate_path(&old, &new).unwrap());
        assert!(old.exists(), "the old path is left alone");
        assert_eq!(
            std::fs::read_to_string(new.join("remotes.toml")).unwrap(),
            "new"
        );
    }

    #[test]
    fn migrate_path_without_old_path_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let new = tmp.path().join("krake");
        assert!(!migrate_path(&tmp.path().join("amigo"), &new).unwrap());
        assert!(!new.exists());
    }

    #[test]
    fn migrate_db_moves_sqlite_sidecars() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("amigo.db");
        let new = tmp.path().join("krake.db");
        std::fs::write(&old, "db").unwrap();
        std::fs::write(tmp.path().join("amigo.db-wal"), "wal").unwrap();
        std::fs::write(tmp.path().join("amigo.db-shm"), "shm").unwrap();

        assert!(migrate_db(&old, &new).unwrap());
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "db");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("krake.db-wal")).unwrap(),
            "wal"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("krake.db-shm")).unwrap(),
            "shm"
        );
        assert!(!old.exists());
        assert!(!tmp.path().join("amigo.db-wal").exists());
        // A second start finds krake.db and does nothing.
        assert!(!migrate_db(&old, &new).unwrap());
    }

    #[test]
    fn migrate_db_keeps_existing_database() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("amigo.db");
        let new = tmp.path().join("krake.db");
        std::fs::write(&old, "old").unwrap();
        std::fs::write(&new, "new").unwrap();

        assert!(!migrate_db(&old, &new).unwrap());
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "old");
    }

    #[test]
    fn migrate_db_opens_with_sqlite_afterwards() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("amigo.db");
        let new = tmp.path().join("krake.db");
        {
            let conn = rusqlite::Connection::open(&old).unwrap();
            conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('kept');")
                .unwrap();
        }

        assert!(migrate_db(&old, &new).unwrap());
        let conn = rusqlite::Connection::open(&new).unwrap();
        let v: String = conn
            .query_row("SELECT v FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(v, "kept");
    }

    #[test]
    fn copy_then_remove_moves_tree() {
        // The cross-device fallback of `move_path`, exercised directly: a
        // test cannot portably create a second filesystem.
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("amigo");
        let new = tmp.path().join("krake");
        std::fs::create_dir_all(old.join("cache").join("plugin-bytecode")).unwrap();
        std::fs::write(
            old.join("cache").join("plugin-bytecode").join("a.qjs"),
            "bc",
        )
        .unwrap();
        std::fs::write(old.join("config.toml"), "cfg").unwrap();

        copy_then_remove(&old, &new).unwrap();
        assert!(!old.exists());
        assert_eq!(
            std::fs::read_to_string(new.join("cache").join("plugin-bytecode").join("a.qjs"))
                .unwrap(),
            "bc"
        );
        assert_eq!(
            std::fs::read_to_string(new.join("config.toml")).unwrap(),
            "cfg"
        );
    }

    #[test]
    fn copy_then_remove_moves_file() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("amigo.db");
        let new = tmp.path().join("krake.db");
        std::fs::write(&old, "db").unwrap();

        copy_then_remove(&old, &new).unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "db");
    }
}
