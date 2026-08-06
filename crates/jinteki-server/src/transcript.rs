//! The operator's transcript: an unfiltered, append-only record of one game,
//! written to disk so a game that went wrong can be reconstructed after the
//! fact.
//!
//! This is the OTHER log, and it is nothing like the player's. The player's
//! log (`crlog`) is filtered twice — once for what is worth saying, once for
//! what each reader is entitled to know. This one is filtered not at all: it
//! carries every [`GameChange`] the kernel recorded, every [`DecisionSpec`]
//! the machine offered and every [`DecisionAnswer`] taken, in order, with the
//! game id and a monotonic sequence number. With the seed and the deck lists
//! in the opening record, the answers alone replay the game.
//!
//! **It is a debug artefact, not a feature.**
//!
//! * It is written under the server's own data dir (`JINTEKI_DATA_DIR`, the
//!   same directory `jinteki.db` lives in) and is NEVER served: no route
//!   reads it, and the static file service is pointed at the UI directory,
//!   not this one. It contains hidden information about a live game — both
//!   grips, both decks — so serving it to a player would be handing them the
//!   game.
//! * It is OFF unless a process turns it on. [`configure`] is called once, by
//!   `main`, with the resolved data dir; a library user (the test suite) that
//!   never calls it writes no files at all.
//! * It is bounded twice: [`MAX_BYTES`] per game, after which the file is
//!   closed with a `truncated` record and the game goes on unlogged, and
//!   [`MAX_FILES`] transcripts in the directory, the oldest pruned as new
//!   games start. A debug artefact may never be the reason a box fills up.
//!
//! The payload is the `Debug` rendering of the kernel's own types. That is
//! deliberate: `Debug` is total (every variant, every field, no bespoke
//! serializer to drift out of date) and the kernel's types are written to be
//! read. The line around it is JSON so `jq` can walk it.

use jinteki_cr::change::GameChange;
use jinteki_cr::object::Side;
use jinteki_cr::{DecisionAnswer, DecisionSpec};
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// One game's transcript stops here. Enough for a very long game of very
/// chatty cards; small enough that a runaway loop cannot eat the disk.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Transcripts kept in the directory. Starting a new game prunes the oldest
/// beyond this.
pub const MAX_FILES: usize = 128;

const DIR_NAME: &str = "transcripts";

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Turn transcripts on, under `<data_dir>/transcripts`.
///
/// Called once, from `main`, with the same directory the database is opened
/// in. Returns the directory if it could be created — a process that never
/// calls this, or whose data dir is unwritable, writes nothing and says so
/// rather than failing a game.
pub fn configure(data_dir: &Path) -> Option<PathBuf> {
    let dir = data_dir.join(DIR_NAME);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("no game transcripts: {} is not writable ({e})", dir.display());
        return None;
    }
    let _ = ROOT.set(dir.clone());
    Some(ROOT.get().cloned().unwrap_or(dir))
}

/// Where transcripts are being written, if they are.
pub fn root() -> Option<&'static Path> {
    ROOT.get().map(|p| p.as_path())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A game id is a hex token from `cr::new_token`, but it names a FILE, so it
/// is filtered to the characters a file name may have rather than trusted.
fn safe_id(game: &str) -> String {
    let s: String = game
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();
    if s.is_empty() {
        format!("game-{}", now_ms())
    } else {
        s
    }
}

/// Keep the directory to [`MAX_FILES`] transcripts, oldest first out.
fn prune(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().unwrap_or(UNIX_EPOCH), e.path()))
        })
        .collect();
    if files.len() <= MAX_FILES {
        return;
    }
    files.sort_by_key(|(t, _)| *t);
    let excess = files.len() - MAX_FILES;
    for (_, p) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(p);
    }
}

/// Records held in memory before a write. The buffer is spilled at every
/// decision point (see [`Transcript::flush`]) and on drop, so a crashed or
/// abandoned game loses at most one decision's worth of tail — while a busy
/// turn costs one syscall instead of a hundred.
const SPILL_BYTES: usize = 8 * 1024;

/// One game's append-only file. A transcript with no file is a no-op that
/// costs a branch per record, which is what "off" means here.
pub struct Transcript {
    game: String,
    path: Option<PathBuf>,
    file: Option<File>,
    buf: String,
    seq: u64,
    written: u64,
}

impl Transcript {
    /// A transcript that writes nothing.
    pub fn off() -> Transcript {
        Transcript {
            game: String::new(),
            path: None,
            file: None,
            buf: String::new(),
            seq: 0,
            written: 0,
        }
    }

    /// This game's transcript in the configured directory, or [`off`] if the
    /// process never configured one.
    ///
    /// [`off`]: Transcript::off
    pub fn open(game: &str) -> Transcript {
        match root() {
            Some(dir) => Transcript::open_in(dir, game),
            None => Transcript::off(),
        }
    }

    /// This game's transcript in an explicit directory (what the tests use,
    /// and what keeps the module honest about where its bytes go).
    pub fn open_in(dir: &Path, game: &str) -> Transcript {
        let id = safe_id(game);
        if std::fs::create_dir_all(dir).is_err() {
            return Transcript::off();
        }
        prune(dir);
        let path = dir.join(format!("{id}.jsonl"));
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => {
                let written = f.metadata().map(|m| m.len()).unwrap_or(0);
                Transcript {
                    game: id,
                    path: Some(path),
                    file: Some(f),
                    buf: String::new(),
                    seq: 0,
                    written,
                }
            }
            Err(e) => {
                tracing::warn!("no transcript for game {id}: {e}");
                Transcript::off()
            }
        }
    }

    /// The file this transcript is being written to, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn write(&mut self, kind: &str, mut v: serde_json::Value) {
        if self.file.is_none() {
            return;
        }
        self.seq += 1;
        if let Some(m) = v.as_object_mut() {
            m.insert("seq".into(), json!(self.seq));
            m.insert("game".into(), json!(self.game));
            m.insert("ms".into(), json!(now_ms()));
            m.insert("kind".into(), json!(kind));
        }
        let mut line = v.to_string();
        line.push('\n');
        if self.written + line.len() as u64 > MAX_BYTES {
            // Say so IN the file: a transcript that just stops is one an
            // operator will misread as the moment the game stopped.
            self.buf.push_str(&format!(
                "{}\n",
                json!({"kind": "truncated", "game": self.game, "seq": self.seq})
            ));
            self.flush();
            self.file = None;
            return;
        }
        self.written += line.len() as u64;
        self.buf.push_str(&line);
        if self.buf.len() >= SPILL_BYTES {
            self.spill();
        }
    }

    fn spill(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        if let Some(f) = self.file.as_mut() {
            let _ = f.write_all(self.buf.as_bytes());
        }
        self.buf.clear();
    }

    /// The opening record: everything the game was started FROM. With this
    /// and the answers below, the game replays.
    pub fn started(&mut self, seed: u64, corp: &str, runner: &str, decks: serde_json::Value) {
        self.write(
            "start",
            json!({"seed": seed, "corp": corp, "runner": runner, "decks": decks}),
        );
    }

    /// One kernel record, verbatim.
    pub fn change(&mut self, c: &GameChange) {
        self.write("change", json!({"detail": format!("{c:?}")}));
    }

    /// One decision the machine offered, with whose it was.
    pub fn decision(&mut self, side: Side, spec: &DecisionSpec) {
        self.write(
            "decision",
            json!({"side": side_key(side), "detail": format!("{spec:?}")}),
        );
    }

    /// One decision taken. `by` is "bot" or "human" — which seat answered is
    /// the first thing an operator wants when a game went sideways.
    pub fn answer(&mut self, side: Side, by: &str, a: &DecisionAnswer) {
        self.write(
            "answer",
            json!({"side": side_key(side), "by": by, "detail": format!("{a:?}")}),
        );
    }

    /// Anything else worth a line (the result, a concession, a stall).
    pub fn note(&mut self, text: &str) {
        self.write("note", json!({"detail": text}));
    }

    /// Push the tail to disk. Called at every decision point, so a game that
    /// hangs has its last moments on disk rather than in a buffer — which is
    /// precisely the game an operator will be asked about.
    pub fn flush(&mut self) {
        self.spill();
        if let Some(f) = self.file.as_mut() {
            let _ = f.flush();
        }
    }
}

/// A game that is dropped — pruned from the registry, or the process going
/// down cleanly — still lands its tail.
impl Drop for Transcript {
    fn drop(&mut self) {
        self.spill();
    }
}

fn side_key(s: Side) -> &'static str {
    match s {
        Side::Corp => "corp",
        Side::Runner => "runner",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jinteki_cr::object::ObjectId;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jinteki-transcript-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_transcript_is_jsonl_with_a_monotonic_sequence() {
        let dir = tmpdir("jsonl");
        let mut t = Transcript::open_in(&dir, "abc123");
        t.started(7, "you", "bot", json!({"corp": "gauntlet"}));
        t.change(&GameChange::GameBegan);
        t.change(&GameChange::CardDrawn { side: Side::Corp, obj: ObjectId(3), source: None });
        t.decision(Side::Runner, &DecisionSpec::Mulligan);
        t.answer(Side::Runner, "human", &DecisionAnswer::KeepHand);
        t.flush();

        let text = std::fs::read_to_string(t.path().unwrap()).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("every line is JSON"))
            .collect();
        assert_eq!(lines.len(), 5);
        for (i, l) in lines.iter().enumerate() {
            assert_eq!(l["seq"], json!(i as u64 + 1), "monotonic sequence");
            assert_eq!(l["game"], json!("abc123"), "every line names the game");
            assert!(l["ms"].as_u64().unwrap() > 0);
        }
        assert_eq!(lines[0]["kind"], json!("start"));
        assert_eq!(lines[0]["seed"], json!(7));
        assert_eq!(lines[1]["detail"], json!("GameBegan"));
        assert!(lines[2]["detail"].as_str().unwrap().contains("CardDrawn"));
        assert_eq!(lines[3]["kind"], json!("decision"));
        assert_eq!(lines[3]["side"], json!("runner"));
        assert_eq!(lines[4]["by"], json!("human"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The library default is silence: a process that never calls
    /// [`configure`] must not write anywhere, which is what keeps the test
    /// suite from strewing files through the repository.
    #[test]
    fn an_unconfigured_transcript_writes_nothing() {
        let mut t = Transcript::off();
        assert!(t.path().is_none());
        t.started(1, "a", "b", json!(null));
        t.change(&GameChange::GameBegan);
        t.note("nothing happens");
        // No panic, no file, no path — the point is that there is nothing.
        assert!(t.path().is_none());
    }

    #[test]
    fn a_game_id_can_never_escape_the_transcript_directory() {
        assert_eq!(safe_id("../../etc/passwd"), "etcpasswd");
        assert_eq!(safe_id("ab-12_CD"), "ab-12_CD");
        assert!(safe_id("////").starts_with("game-"));
    }

    /// It is a debug artefact, not a feature. A transcript holds BOTH grips
    /// and BOTH decks of a live game, so serving one would be handing a
    /// player the game — and the way to be sure nothing serves it is that
    /// nothing but the writer names it at all.
    #[test]
    fn nothing_that_answers_a_client_ever_names_a_transcript() {
        for (name, src) in [
            ("api.rs", include_str!("api.rs")),
            ("local.rs", include_str!("local.rs")),
            ("lobby.rs", include_str!("lobby.rs")),
            ("bridge/mod.rs", include_str!("bridge/mod.rs")),
        ] {
            assert!(
                !src.contains("transcript"),
                "{name} reaches for the transcript; nothing that answers a client may"
            );
        }
        // `main` turns them on, beside the database — and points the static
        // file service at the UI directory, never at the data directory.
        let main = include_str!("main.rs");
        assert!(main.contains("transcript::configure(&data_dir)"));
        assert!(main.contains("ServeDir::new(ui_dir)"));
        assert!(!main.contains("ServeDir::new(data_dir"));
    }

    #[test]
    fn the_directory_is_pruned_to_a_bound() {
        let dir = tmpdir("prune");
        for i in 0..(MAX_FILES + 5) {
            let mut t = Transcript::open_in(&dir, &format!("g{i:04}"));
            t.note("hello");
            t.flush();
        }
        let n = std::fs::read_dir(&dir).unwrap().count();
        assert!(
            n <= MAX_FILES + 1,
            "a debug artefact stays bounded: {n} files in {}",
            dir.display()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
