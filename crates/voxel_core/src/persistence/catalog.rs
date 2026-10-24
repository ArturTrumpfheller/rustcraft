//! Safe multi-world directory catalog. Existing bundle formats are untouched.
//! Only explicitly requested creation/import publishes a new directory. Failed
//! operations retain staging data for investigation, never recursively delete.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

const META: &str = "world.meta";
// User preferences are root-owned, never copied into imported world bundles.
const RESERVED: [&str; 8] = [
    "worlds",
    "archive",
    ".staging",
    ".catalog.lock",
    "settings.rcs",
    ".settings.lock",
    "settings.rcs.tmp",
    "settings.rcs.rejected",
];
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldKey {
    Managed(String),
    Legacy,
    Incomplete(String),
}
#[derive(Clone, Debug)]
pub struct WorldEntry {
    pub key: WorldKey,
    pub name: String,
    pub path: PathBuf,
    pub seed: Option<u64>,
    pub generator: Option<u32>,
    pub kind: String,
    /// Catalog creation/import time, never guessed from filesystem timestamps.
    pub created: Option<u64>,
    pub error: Option<String>,
}
#[derive(Clone, Debug)]
pub struct WorldCatalog {
    root: PathBuf,
}
fn fail(message: impl Into<String>) -> SaveError {
    SaveError::RecoveryRequired(message.into())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn fresh_id() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{:032x}",
        t ^ (u128::from(std::process::id()) << 64)
            ^ u128::from(NEXT_TEMP.fetch_add(1, Ordering::Relaxed))
    )
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
pub fn validate_name(name: &str) -> Result<(), SaveError> {
    if name.trim().is_empty() || name.chars().count() > 96 || name.chars().any(char::is_control) {
        return Err(fail(
            "Name must contain 1 to 96 characters without control characters",
        ));
    }
    Ok(())
}
pub fn parse_seed(seed: &str) -> Result<Option<u64>, SaveError> {
    if seed.is_empty() {
        return Ok(None);
    }
    if !seed.bytes().all(|c| c.is_ascii_digit()) {
        return Err(fail(
            "Seed must be a decimal integer from 0 to 18446744073709551615",
        ));
    }
    seed.parse()
        .map(Some)
        .map_err(|_| fail("Seed is outside the u64 range"))
}
pub fn random_seed() -> u64 {
    // Unpredictability is not a security boundary; seed is saved before playing.
    let id = fresh_id();
    let n = u128::from_str_radix(&id, 16).unwrap();
    let mut x = n as u64 ^ (n >> 64) as u64;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
/// Reject symlinks and Windows reparse points (including directory junctions).
fn plain(path: &Path) -> Result<fs::Metadata, SaveError> {
    let m = fs::symlink_metadata(path)?;
    let link = m.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err(fail("Windows reparse point refused"));
        }
    }
    if link {
        return Err(fail(format!(
            "Symlink/reparse point refused: {}",
            path.display()
        )));
    }
    if !m.is_dir() && !m.is_file() {
        return Err(fail("Only regular files/directories supported"));
    }
    Ok(m)
}
fn check_existing(path: &Path) -> Result<(), SaveError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            plain(path)?;
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn ancestors(path: &Path) -> Result<(), SaveError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(fail("Absolute path without parent traversal required"));
    }
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(_) => {
                plain(p)?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn sync_dir(path: &Path) -> Result<(), SaveError> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn tree(path: &Path, skip_catalog: bool) -> Result<Vec<PathBuf>, SaveError> {
    fn visit(
        path: &Path,
        root: &Path,
        skip: bool,
        depth: usize,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), SaveError> {
        if depth > 32 {
            return Err(fail("Bundle directory nesting exceeds 32"));
        }
        plain(path)?;
        for item in fs::read_dir(path)? {
            let p = item?.path();
            if path == root
                && skip
                && RESERVED
                    .iter()
                    .any(|n| p.file_name() == Some(std::ffi::OsStr::new(n)))
            {
                continue;
            }
            let m = plain(&p)?;
            if m.is_dir() {
                visit(&p, root, skip, depth + 1, files)?;
            }
            files.push(p);
        }
        Ok(())
    }
    let mut files = vec![];
    visit(path, path, skip_catalog, 0, &mut files)?;
    Ok(files)
}
fn metadata(path: &Path, id: &str) -> Result<(String, u64), SaveError> {
    plain(path)?;
    let b = read_limited(path, 512)?.ok_or_else(|| fail("Metadata is missing"))?;
    if b.len() < 52 || &b[..8] != b"RCMETA01" || b[8..40] != *id.as_bytes() {
        return Err(fail("Invalid metadata / ID"));
    }
    let crc = u32::from_le_bytes(b[b.len() - 4..].try_into().unwrap());
    if crc32fast::hash(&b[..b.len() - 4]) != crc {
        return Err(fail("Metadata checksum mismatch"));
    }
    let created = u64::from_le_bytes(b[40..48].try_into().unwrap());
    let name = std::str::from_utf8(&b[48..b.len() - 4])
        .map_err(|_| fail("Invalid metadata UTF-8"))?
        .to_owned();
    validate_name(&name)?;
    Ok((name, created))
}
fn write_metadata(path: &Path, id: &str, name: &str, created: u64) -> Result<(), SaveError> {
    validate_name(name)?;
    let mut b = b"RCMETA01".to_vec();
    b.extend(id.as_bytes());
    b.extend(created.to_le_bytes());
    b.extend(name.as_bytes());
    b.extend(crc32fast::hash(&b).to_le_bytes());
    atomic_replace(path, &b, true, &mut |_| Ok(()))?;
    Ok(())
}
impl WorldCatalog {
    pub fn new(root: PathBuf) -> Result<Self, SaveError> {
        ancestors(&root)?;
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn area(&self, name: &str) -> Result<PathBuf, SaveError> {
        ancestors(&self.root)?;
        let p = self.root.join(name);
        check_existing(&p)?;
        Ok(p)
    }
    fn transaction(&self) -> Result<File, SaveError> {
        ancestors(&self.root)?;
        fs::create_dir_all(&self.root)?;
        let p = self.root.join(".catalog.lock");
        check_existing(&p)?;
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(p)?;
        f.try_lock()
            .map_err(|e| fail(format!("World catalog is locked: {e}")))?;
        Ok(f)
    }
    fn managed(&self, id: &str) -> Result<PathBuf, SaveError> {
        if !valid_id(id) {
            return Err(fail("Invalid world ID"));
        }
        let p = self.area("worlds")?.join(id);
        if !plain(&p)?.is_dir() {
            return Err(fail("World directory is missing"));
        }
        Ok(p)
    }
    pub fn list(&self) -> Result<Vec<WorldEntry>, SaveError> {
        ancestors(&self.root)?;
        if !self.root.try_exists()? {
            return Ok(vec![]);
        }
        let mut out = vec![];
        let legacy = fs::read_dir(&self.root)?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.starts_with("world.")
                    || n.starts_with("player.")
                    || n.starts_with("player-world.")
                    || n.starts_with("session.")
                    || n == "regions"
            });
        if legacy {
            out.push(self.entry(
                WorldKey::Legacy,
                self.root.clone(),
                "Legacy Single World".into(),
            ));
        }
        for (area, incomplete) in [("worlds", false), (".staging", true)] {
            let p = self.area(area)?;
            if !p.try_exists()? {
                continue;
            }
            for e in fs::read_dir(p)? {
                let e = e?;
                let id = e.file_name().to_string_lossy().into_owned();
                let key = if incomplete {
                    WorldKey::Incomplete(id.clone())
                } else {
                    WorldKey::Managed(id.clone())
                };
                out.push(self.entry(key, e.path(), id));
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
        Ok(out)
    }
    fn entry(&self, key: WorldKey, path: PathBuf, fallback: String) -> WorldEntry {
        let mut e = WorldEntry {
            key,
            name: fallback,
            path,
            seed: None,
            generator: None,
            kind: "Unknown".into(),
            created: None,
            error: None,
        };
        let result = (|| {
            if matches!(e.key, WorldKey::Incomplete(_)) {
                return Err(fail("Incomplete operation; files preserved for inspection"));
            }
            if let WorldKey::Managed(id) = &e.key {
                if !valid_id(id) {
                    return Err(fail("Invalid directory ID"));
                }
                plain(&e.path)?;
                let (name, created) = metadata(&e.path.join(META), id)?;
                e.name = name;
                e.created = Some(created);
            }
            // Full validation is on the catalog worker, never in rendering/input.
            let (_store, world) = validate(&e.path, matches!(e.key, WorldKey::Legacy))?;
            e.seed = world.identity().map(|i| i.seed);
            e.generator = world.identity().map(|i| i.generator);
            e.kind = if world.region_identity().is_some() {
                "Regional v4"
            } else if world.identity().is_some() {
                "Finite v3"
            } else {
                "Legacy v1/v2"
            }
            .into();
            Ok(())
        })();
        e.error = result.err().map(|error: SaveError| error.to_string());
        e
    }
    fn stage(&self) -> Result<(String, PathBuf), SaveError> {
        let p = self.area(".staging")?;
        fs::create_dir_all(&p)?;
        let worlds = self.area("worlds")?;
        fs::create_dir_all(&worlds)?;
        let archives = self.area("archive")?;
        for _ in 0..16 {
            let id = fresh_id();
            if worlds.join(&id).try_exists()? || archives.join(&id).try_exists()? {
                continue;
            }
            let stage = p.join(&id);
            match fs::create_dir(&stage) {
                Ok(()) => {
                    sync_dir(&p)?;
                    sync_dir(&self.root)?;
                    return Ok((id, stage));
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        Err(fail("World ID collision"))
    }
    fn publish(&self, id: &str, stage: &Path) -> Result<(), SaveError> {
        let target = self.area("worlds")?.join(id);
        if fs::symlink_metadata(&target).is_ok() {
            return Err(fail("Target already exists"));
        }
        fs::rename(stage, target)?;
        sync_dir(&self.area("worlds")?)?;
        sync_dir(&self.area(".staging")?)
    }
    pub fn create(&self, name: &str, seed: u64) -> Result<String, SaveError> {
        self.create_with(name, seed, || Ok(()))
    }
    fn create_with(
        &self,
        name: &str,
        seed: u64,
        before_publish: impl FnOnce() -> io::Result<()>,
    ) -> Result<String, SaveError> {
        validate_name(name)?;
        let _guard = self.transaction()?;
        let (id, stage) = self.stage()?;
        let (store, mut world, _) =
            SaveStore::open_with(&stage, || Ok(crate::streaming::new_large_world(seed)))?;
        store.load_player(&mut world)?;
        drop(store);
        write_metadata(&stage.join(META), &id, name, now())?;
        drop(validate(&stage, false)?);
        before_publish()?;
        self.publish(&id, &stage)?;
        Ok(id)
    }
    /// Caller must obtain explicit confirmation. Source is locked throughout
    /// read-only validation/copy; journal replay occurs on the private copy only.
    pub fn import_legacy(&self, name: &str) -> Result<String, SaveError> {
        self.import_with(name, || Ok(()))
    }
    fn import_with(
        &self,
        name: &str,
        mut copied_file: impl FnMut() -> io::Result<()>,
    ) -> Result<String, SaveError> {
        validate_name(name)?;
        let _guard = self.transaction()?;
        let (source_lock, _) = validate(&self.root, true)?;
        let (id, stage) = self.stage()?;
        for path in tree(&self.root, true)? {
            let dest = stage.join(
                path.strip_prefix(&self.root)
                    .map_err(|_| fail("Copy containment"))?,
            );
            if plain(&path)?.is_dir() {
                fs::create_dir_all(&dest)?;
            } else {
                fs::create_dir_all(dest.parent().unwrap())?;
                let mut dst = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&dest)?;
                if path == self.root.join("world.lock") {
                    // Windows byte-range locks deny reads through a separately
                    // opened handle. Copy via the handle that owns the lock.
                    io::copy(&mut &source_lock._lock, &mut dst)?;
                } else {
                    io::copy(&mut File::open(&path)?, &mut dst)?;
                }
                dst.sync_all()?;
                copied_file()?;
            }
        }
        // Keep every journal/backup byte intact. Recovery is validated read-only
        // here and replayed only when the imported copy is explicitly played.
        if stage.join(META).try_exists()? {
            return Err(fail(
                "Legacy source contains reserved world.meta; inspect manually",
            ));
        }
        write_metadata(&stage.join(META), &id, name, now())?;
        drop(validate(&stage, false)?);
        for p in tree(&stage, false)? {
            if p.is_dir() {
                sync_dir(&p)?;
            }
        }
        sync_dir(&stage)?;
        self.publish(&id, &stage)?;
        Ok(id)
    }
    pub fn rename(&self, id: &str, name: &str) -> Result<(), SaveError> {
        validate_name(name)?;
        let _guard = self.transaction()?;
        let p = self.managed(id)?;
        tree(&p, false)?;
        let _lock = SaveStore::lock_directory(&p)?;
        let (_, created) = metadata(&p.join(META), id)?;
        write_metadata(&p.join(META), id, name, created)
    }
    pub fn archive(&self, id: &str) -> Result<PathBuf, SaveError> {
        let _guard = self.transaction()?;
        let p = self.managed(id)?;
        tree(&p, false)?;
        let lock = SaveStore::lock_directory(&p)?;
        let archive = self.area("archive")?;
        fs::create_dir_all(&archive)?;
        let target = archive.join(id);
        if fs::symlink_metadata(&target).is_ok() {
            return Err(fail("Archive target already exists"));
        }
        // Windows refuses a directory rename with an open descendant, even
        // with delete sharing. The catalog mutex excludes all production opens
        // across this checked unlock/rename. A competing external open makes
        // rename fail without moving or deleting data. Unix keeps the lock.
        #[cfg(windows)]
        drop(lock);
        fs::rename(&p, &target)?;
        #[cfg(not(windows))]
        drop(lock);
        sync_dir(&archive)?;
        sync_dir(&self.area("worlds")?)?;
        Ok(target)
    }
    /// All production opens share the catalog mutex with archive/rename/import.
    pub fn open(&self, id: &str) -> Result<(SaveStore, World, LoadSource), SaveError> {
        let _guard = self.transaction()?;
        let p = self.managed(id)?;
        metadata(&p.join(META), id)?;
        let (store, _) = validate(&p, false)?;
        drop(store);
        SaveStore::open_existing(p)
    }
}
/// Read-only, full-bundle validation under the world lock. Historical backups
/// and temporary recovery artifacts are preserved; active primaries must validate.
fn validate(path: &Path, legacy: bool) -> Result<(SaveStore, World), SaveError> {
    ancestors(path)?;
    tree(path, legacy)?;
    let store = SaveStore::lock_directory(path)?;
    let files = tree(path, legacy)?;
    let regions = path.join("regions");
    if regions.try_exists()? && !plain(&regions)?.is_dir() {
        return Err(fail("regions must be a directory"));
    }
    let root = read_save(&store.path())?;
    let world = match root {
        Some(b) => decode(&b)?,
        None if store.backup_path().try_exists()? => {
            return Err(fail("Primary world file is missing; backup preserved"));
        }
        None => store.pending_world()?,
    };
    store.validate_player_bundle(&world)?;
    for file in files {
        if !file.starts_with(path.join("regions")) || !plain(&file)?.is_file() {
            continue;
        }
        let name = file.file_name().unwrap().to_string_lossy();
        let primary = if name.ends_with(".rcw.bak") {
            file.with_extension("")
        } else {
            file.clone()
        };
        if primary.extension() != Some(std::ffi::OsStr::new("rcw")) {
            continue;
        }
        let stem = primary.file_stem().unwrap().to_string_lossy();
        let (x, z) = stem
            .split_once('_')
            .ok_or_else(|| fail("Invalid region path"))?;
        let pos = crate::regions::RegionPos([
            x.parse().map_err(|_| fail("Invalid region coordinate"))?,
            z.parse().map_err(|_| fail("Invalid region coordinate"))?,
        ]);
        if store.region_path(pos) != primary {
            return Err(fail("Incorrect region path"));
        }
        let id = world.region_identity().ok_or(FormatError::Identity)?;
        store.read_region(id, pos)?;
    }
    Ok((store, world))
}
#[cfg(test)]
mod tests;
