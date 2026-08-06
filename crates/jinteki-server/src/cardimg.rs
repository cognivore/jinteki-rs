//! Card art, served from OUR box.
//!
//! Every card image the UI draws used to be an `<img>` pointed straight at
//! `card-images.netrunnerdb.com`. That is somebody else's CDN, reached by two
//! thousand parallel requests from a deck builder's grid: it rate-limits, it
//! 403s a client whose User-Agent it does not like, and every request that
//! loses lands on a player as a BLANK card — the text scaffold with no art
//! behind it. A card the player cannot recognise is exactly what UX.md THE
//! LAW §1 exists to prevent.
//!
//! So the art is ours. One route, `GET /img/card/<code>.jpg`, serves the
//! image out of a local directory under the server's own data dir. Three
//! properties make that safe and cheap:
//!
//! * **The path is never the caller's.** `<code>` is resolved through
//!   `carddata` first — as a current NRDB code, an earlier printing's code,
//!   or the catalog's NSG v2 id (the deck builder speaks those, not codes) —
//!   and the file name is the CODE WE FOUND. A request for `../../etc/passwd`
//!   resolves to no card and gets a 404 before any path is built.
//! * **A miss fetches upstream exactly once.** A per-code gate means a cold
//!   grid asking for the same card forty times makes one request; a code
//!   upstream does not have is remembered for [`MISS_TTL`] so a missing card
//!   cannot turn into a hammer.
//! * **A hit is a file read with immutable headers.** The art for a printing
//!   never changes, so the response says so (`immutable`, a year of
//!   `max-age`, an ETag of the code) and a warm browser stops asking.
//!
//! [`spawn_prewarm`] walks the whole catalog at a polite handful of requests
//! per second on start, so the steady state is a cache that already has every
//! card before any player asks for one. It is idempotent — a warm cache is a
//! directory listing and nothing else — and it can never take the server
//! down: it is spawned, never awaited, and every failure inside it is counted
//! and logged rather than propagated.

use crate::carddata;
use axum::extract::Path as AxPath;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};

/// Where the art comes from when we do not have it yet. Same size the UI
/// draws its readers at (`v2/large`).
const UPSTREAM: &str = "https://card-images.netrunnerdb.com/v2/large/";

/// The cache directory, under the data dir beside `jinteki.db`.
const DIR_NAME: &str = "card-images";

/// A card image is ~40 KB; anything an order of magnitude past that is not a
/// card image and does not get written to disk.
const MAX_BYTES: usize = 4 << 20;

/// One upstream fetch's patience.
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a code upstream did not have stays "not there". Short enough
/// that a CDN outage heals by itself, long enough that a card with no art
/// cannot become a request per grid scroll.
const MISS_TTL: Duration = Duration::from_secs(600);

/// The pre-warm's politeness: this many fetches in flight, each worker
/// pausing this long after every request. Six requests a second at the
/// ceiling, and less in practice because the pause starts after the fetch —
/// the reference implementation throttles its own image sync to five
/// (`tasks/nrdb.clj:21-22`), and this is somebody else's CDN. The whole
/// catalog is ~2000 images, so a cold box is warm inside ten minutes and
/// then never asks again.
const WARM_CONCURRENCY: usize = 3;
const WARM_DELAY: Duration = Duration::from_millis(500);

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Turn the cache on, at `<data_dir>/card-images`.
///
/// Called once from `main` with the same directory the database lives in —
/// the RUNTIME data dir (`JINTEKI_DATA_DIR`), never the read-only nix store,
/// so the images survive a restart and never enter a build. A process that
/// does not call this (the test suite) keeps no cache at all: the route still
/// answers, straight from upstream, and writes nothing.
pub fn configure(data_dir: &Path) -> Option<PathBuf> {
    let dir = data_dir.join(DIR_NAME);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("no card-art cache: {} is not writable ({e})", dir.display());
        return None;
    }
    let _ = ROOT.set(dir.clone());
    Some(ROOT.get().cloned().unwrap_or(dir))
}

/// Where the images are being kept, if they are.
pub fn root() -> Option<&'static Path> {
    ROOT.get().map(|p| p.as_path())
}

/// One client, one honest User-Agent. NRDB's bot shield 403s the default one
/// (ACCOUNTS-AND-DECKS.md §1.4) — the same reason the importer sets it.
fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(crate::nrdb::USER_AGENT)
            .timeout(FETCH_TIMEOUT)
            .build()
            .unwrap_or_default()
    })
}

/// The printing whose art answers for `id`, as a code out of our own card
/// data. Accepts the three vocabularies the client speaks: the game state's
/// current NRDB code, an older printing's code (an imported decklist's), and
/// the deck-builder catalog's NSG v2 id (`account_siphon`).
///
/// The returned string is always ours, never the caller's — every file path
/// in this module is built from it, so a caller cannot steer one.
pub fn resolve(id: &str) -> Option<&'static str> {
    carddata::by_code(id)
        .or_else(|| carddata::by_previous_code(id))
        .or_else(|| carddata::by_nsg_id(id))
        .map(|c| c.code.as_str())
}

fn path_of(code: &str) -> Option<PathBuf> {
    root().map(|r| r.join(format!("{code}.jpg")))
}

/// Is this code already on disk? The pre-warm's whole idempotence.
fn cached(code: &str) -> bool {
    path_of(code).map(|p| p.is_file()).unwrap_or(false)
}

/// Codes upstream answered "no" for, and when. Bounded by the catalog.
fn missing() -> &'static StdMutex<HashMap<&'static str, Instant>> {
    static M: OnceLock<StdMutex<HashMap<&'static str, Instant>>> = OnceLock::new();
    M.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn recently_missing(code: &'static str) -> bool {
    let m = missing().lock().unwrap_or_else(|e| e.into_inner());
    m.get(code).map(|t| t.elapsed() < MISS_TTL).unwrap_or(false)
}

/// One gate per code, so a cold grid asking for the same card forty times
/// makes ONE upstream request and thirty-nine disk reads.
fn gate(code: &'static str) -> Arc<tokio::sync::Mutex<()>> {
    static G: OnceLock<StdMutex<HashMap<&'static str, Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    let g = G.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut map = g.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(code).or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))).clone()
}

/// Why an image is not available. Both are ordinary — a card with no art
/// upstream is a scaffold, not an error (UX.md THE LAW §1's fallback).
#[derive(Debug)]
pub enum Miss {
    /// Upstream does not have this printing's art.
    NoArt,
    /// Upstream could not be reached / refused us.
    Upstream(String),
}

/// The bytes for `code`: from disk, or fetched once and written there.
pub async fn ensure(code: &'static str) -> Result<Vec<u8>, Miss> {
    if let Some(p) = path_of(code) {
        if let Ok(b) = tokio::fs::read(&p).await {
            if !b.is_empty() {
                return Ok(b);
            }
        }
    }
    if recently_missing(code) {
        return Err(Miss::NoArt);
    }
    let lock = gate(code);
    let _held = lock.lock().await;
    // Someone may have filled it while we queued.
    if let Some(p) = path_of(code) {
        if let Ok(b) = tokio::fs::read(&p).await {
            if !b.is_empty() {
                return Ok(b);
            }
        }
    }
    if recently_missing(code) {
        return Err(Miss::NoArt);
    }
    let bytes = fetch(code).await?;
    if let Some(p) = path_of(code) {
        write_atomic(&p, &bytes).await;
    }
    Ok(bytes)
}

/// Remember that upstream has nothing for this code, and say once — at info,
/// not as a warning — that this is a normal thing for a card to be. The nine
/// player-aid cards ("Agenda Points", "Mark", "Corp Turn"…) and a couple of
/// promo identities have no printed art anywhere; they are SCAFFOLDS, and a
/// scaffold is a supported way for a card to look.
fn note_no_art(code: &'static str) {
    static SAID: AtomicBool = AtomicBool::new(false);
    if !SAID.swap(true, Ordering::Relaxed) {
        tracing::info!(
            "card art: upstream has no image for {code} (nor, most likely, for the \
             other player-aid and promo printings) — those cards draw the text \
             scaffold, which is fine, and is said once"
        );
    }
    missing().lock().unwrap_or_else(|e| e.into_inner()).insert(code, Instant::now());
}

async fn fetch(code: &'static str) -> Result<Vec<u8>, Miss> {
    let url = format!("{UPSTREAM}{code}.jpg");
    let resp = client().get(&url).send().await.map_err(|e| Miss::Upstream(e.to_string()))?;
    let status = resp.status();
    // 404/410 is the plain answer; 403 is the one the image bucket actually
    // gives for a key it does not have (an S3 `AccessDenied` XML body, which
    // is how a bucket without list permission says "no such object"). Reading
    // it as an error meant nine player-aid cards were retried on every single
    // request forever and counted as failures. If a real bot shield ever 403s
    // us, MISS_TTL is ten minutes and it heals itself.
    if matches!(
        status,
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE | reqwest::StatusCode::FORBIDDEN
    ) {
        note_no_art(code);
        return Err(Miss::NoArt);
    }
    if !status.is_success() {
        return Err(Miss::Upstream(format!("HTTP {status}")));
    }
    let body = resp.bytes().await.map_err(|e| Miss::Upstream(e.to_string()))?;
    if body.is_empty() {
        note_no_art(code);
        return Err(Miss::NoArt);
    }
    if body.len() > MAX_BYTES {
        return Err(Miss::Upstream(format!("{} bytes is not a card image", body.len())));
    }
    // A JPEG starts FF D8 FF. An HTML error page dressed as a 200 does not,
    // and caching one would put a broken image on a card forever.
    if body.len() < 4 || body[0] != 0xFF || body[1] != 0xD8 {
        return Err(Miss::Upstream("upstream sent something that is not a JPEG".into()));
    }
    Ok(body.to_vec())
}

/// Write through a temporary name so a killed process can never leave a
/// half-image behind for the next one to serve.
async fn write_atomic(path: &Path, bytes: &[u8]) {
    let tmp = path.with_extension(format!("jpg.part{}", std::process::id()));
    if let Err(e) = tokio::fs::write(&tmp, bytes).await {
        tracing::warn!("card art: cannot write {}: {e}", tmp.display());
        return;
    }
    if let Err(e) = tokio::fs::rename(&tmp, path).await {
        tracing::warn!("card art: cannot place {}: {e}", path.display());
        let _ = tokio::fs::remove_file(&tmp).await;
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The route
// ───────────────────────────────────────────────────────────────────────────

/// `GET /img/card/{code}.jpg`. Merged into the app AFTER the dev-mode
/// no-store layer, because this is the one thing in the whole server that
/// genuinely never changes.
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().route("/img/card/{file}", get(serve))
}

/// Said once, not per request: a card the client asks for that we cannot map
/// to a printing is a SCAFFOLD, which is a supported way for a card to look,
/// not a fault to alarm about.
fn note_unknown(id: &str) {
    static SAID: AtomicBool = AtomicBool::new(false);
    if !SAID.swap(true, Ordering::Relaxed) {
        tracing::info!(
            "card art: \"{id}\" is not a code or id we know — the client draws \
             the text scaffold for it (this is fine, and is said once)"
        );
    } else {
        tracing::debug!("card art: unknown id \"{id}\"");
    }
}

fn immutable_headers(etag: &str) -> [(header::HeaderName, HeaderValue); 3] {
    [
        (header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg")),
        // A printing's art is fixed for the life of the printing.
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ),
        (header::ETAG, HeaderValue::from_str(etag).unwrap_or(HeaderValue::from_static("\"?\""))),
    ]
}

async fn serve(AxPath(file): AxPath<String>, headers: HeaderMap) -> Response {
    let id = file.strip_suffix(".jpg").unwrap_or(&file);
    let Some(code) = resolve(id) else {
        note_unknown(id);
        return (StatusCode::NOT_FOUND, "no such card").into_response();
    };
    let etag = format!("\"{code}\"");
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(',').any(|t| t.trim() == etag || t.trim() == "*"))
        .unwrap_or(false)
    {
        return (StatusCode::NOT_MODIFIED, immutable_headers(&etag)).into_response();
    }
    match ensure(code).await {
        Ok(bytes) => (StatusCode::OK, immutable_headers(&etag), bytes).into_response(),
        Err(Miss::NoArt) => (StatusCode::NOT_FOUND, "no art for this printing").into_response(),
        Err(Miss::Upstream(e)) => {
            tracing::warn!("card art: {code} unavailable: {e}");
            (StatusCode::BAD_GATEWAY, "card art is not reachable right now").into_response()
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The pre-warm
// ───────────────────────────────────────────────────────────────────────────

/// Fill the cache with the whole catalog, in the background, politely.
///
/// Spawned by `main` and never awaited: startup does not wait for it, a
/// failure inside it is a log line, and a cache that is already warm makes no
/// requests at all.
pub fn spawn_prewarm() {
    if root().is_none() {
        tracing::warn!("card art: no cache directory — art is fetched per request");
        return;
    }
    tokio::spawn(async move { prewarm().await });
}

async fn prewarm() {
    let started = Instant::now();
    let all: Vec<&'static str> = carddata::all().iter().map(|c| c.code.as_str()).collect();
    let total = all.len();
    let todo: Arc<Vec<&'static str>> = Arc::new(all.into_iter().filter(|c| !cached(c)).collect());
    if todo.is_empty() {
        tracing::info!("card art: cache is warm — {total} images already on disk");
        return;
    }
    tracing::info!(
        "card art: warming {} of {total} images into {} ({} concurrent, ~{}/s)",
        todo.len(),
        root().map(|p| p.display().to_string()).unwrap_or_default(),
        WARM_CONCURRENCY,
        (WARM_CONCURRENCY as u128 * 1000) / WARM_DELAY.as_millis().max(1),
    );
    let next = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let noart = Arc::new(AtomicUsize::new(0));
    let mut workers = Vec::new();
    for _ in 0..WARM_CONCURRENCY {
        let (todo, next, done, failed, noart) =
            (todo.clone(), next.clone(), done.clone(), failed.clone(), noart.clone());
        workers.push(tokio::spawn(async move {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&code) = todo.get(i) else { return };
                match ensure(code).await {
                    Ok(_) => {
                        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if n % 200 == 0 {
                            tracing::info!("card art: {n}/{} warmed", todo.len());
                        }
                    }
                    Err(Miss::NoArt) => {
                        noart.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!("card art: upstream has no image for {code}");
                    }
                    Err(Miss::Upstream(e)) => {
                        failed.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!("card art: {code} failed: {e}");
                    }
                }
                tokio::time::sleep(WARM_DELAY).await;
            }
        }));
    }
    for w in workers {
        let _ = w.await;
    }
    let (d, f, n) = (
        done.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed),
        noart.load(Ordering::Relaxed),
    );
    tracing::info!(
        "card art: warm — {d} fetched, {n} with no art upstream, {f} failed, \
         {total} in the catalog, in {:.0}s",
        started.elapsed().as_secs_f64()
    );
    if f > 0 {
        tracing::info!(
            "card art: {f} images did not come down this time; the next start \
             picks them up (nothing is retried in between)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vocabulary_the_client_speaks_resolves_to_a_printing() {
        // The game state sends the current NRDB code…
        let sg = carddata::by_title("Sure Gamble").unwrap();
        assert_eq!(resolve(&sg.code), Some(sg.code.as_str()));
        // …an imported decklist an older printing's…
        assert_eq!(resolve("01050"), Some(sg.code.as_str()));
        // …and the deck builder the catalog's NSG v2 id.
        assert_eq!(resolve("sure_gamble"), Some(sg.code.as_str()));
    }

    #[test]
    fn a_caller_cannot_steer_the_path() {
        for hostile in ["../../etc/passwd", "..", "", "01018/../../x", "%2e%2e"] {
            assert!(resolve(hostile).is_none(), "{hostile} must not resolve");
        }
    }

    #[test]
    fn the_whole_catalog_has_something_to_warm() {
        // The pre-warm's work list is every printing's code, and every code
        // resolves to itself — so warming and serving key the same files.
        let all = carddata::all();
        assert!(all.len() > 1000);
        for c in all.iter().take(50) {
            assert_eq!(resolve(&c.code), Some(c.code.as_str()));
        }
    }
}
