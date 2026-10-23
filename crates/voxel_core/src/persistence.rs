//! Versioned snapshots, streamed identities and recoverable filesystem commits. No Bevy
//! types, transient revisions, enum discriminants, meshes or vector layout are
//! serialized. See docs/SAVE_FORMAT.md for the byte-level contract.
use crate::{Block, BlockPos, CHUNK_SIZE, EDIT_MAX, EDIT_MIN, LocalPos, World, demo_world};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub mod catalog;
mod player;
mod regional;
pub use regional::REGIONAL_VERSION;

pub const MAGIC: &[u8; 8] = b"RUSTWRLD";
/// Current legacy full-snapshot writer. Streamed identity/overlays use v3.
pub const FORMAT_VERSION: u32 = 2;
pub const STREAM_FORMAT_VERSION: u32 = 3;
pub const HEADER_SIZE: usize = 24;
pub const RECORD_SIZE: usize = 14;
pub const MAX_RECORDS: usize = 64 * 48 * 64;
pub const MAX_STREAM_RECORDS: usize = 128 * 48 * 128;
pub const STREAM_HEADER_SIZE: usize = 40;
pub const MAX_FILE_SIZE: usize = STREAM_HEADER_SIZE + MAX_STREAM_RECORDS * RECORD_SIZE;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormatError {
    Magic,
    Version(u32),
    Length,
    TooLarge,
    Checksum,
    BlockId(u16),
    Coordinates,
    Identity,
    UnsortedOrDuplicate,
}
impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid world save: {self:?}")
    }
}
impl std::error::Error for FormatError {}

fn in_bounds(p: [i32; 3]) -> bool {
    (0..3).all(|axis| p[axis] >= EDIT_MIN[axis] && p[axis] < EDIT_MAX[axis])
}
fn block_id(block: Block) -> u16 {
    match block {
        Block::Air => 0,
        Block::Grass => 1,
        Block::Dirt => 2,
        Block::Stone => 3,
        Block::Wood => 4,
        Block::Leaves => 5,
    }
}
fn stored_block(id: u16, version: u32) -> Result<Block, FormatError> {
    match id {
        0 if version == STREAM_FORMAT_VERSION => Ok(Block::Air),
        1 => Ok(Block::Grass),
        2 => Ok(Block::Dirt),
        3 => Ok(Block::Stone),
        4 if version >= 2 => Ok(Block::Wood),
        5 if version >= 2 => Ok(Block::Leaves),
        _ => Err(FormatError::BlockId(id)),
    }
}
fn checksum(bytes: &[u8]) -> u32 {
    let mut hash = crc32fast::Hasher::new();
    hash.update(&bytes[..20]);
    hash.update(&bytes[HEADER_SIZE..]);
    hash.finalize()
}

/// Canonical mode-specific save: legacy full non-Air snapshot, or v3 identity
/// and complete edit overlay (including Air). For v4 this encodes ONLY the
/// identity manifest: use `SaveStore::save_player_session` for player-enabled
/// sessions, or `SaveStore::save` for world-only storage, to persist regions too.
/// Resident cache data never substitutes for authoritative edits.
pub fn encode(world: &World) -> Result<Vec<u8>, FormatError> {
    if let Some(id) = world.region_identity() {
        return Ok(regional::manifest(id));
    }
    if let Some(identity) = world.identity() {
        if identity.generator != 2 || world.edits().len() > MAX_STREAM_RECORDS {
            return Err(FormatError::Identity);
        }
        let edits = world.edits();
        let mut bytes = Vec::with_capacity(STREAM_HEADER_SIZE + edits.len() * RECORD_SIZE);
        bytes.extend(MAGIC);
        bytes.extend(STREAM_FORMAT_VERSION.to_le_bytes());
        bytes.extend((edits.len() as u32).to_le_bytes());
        bytes.extend((edits.len() as u32 * RECORD_SIZE as u32).to_le_bytes());
        bytes.extend(0_u32.to_le_bytes());
        bytes.extend(identity.seed.to_le_bytes());
        bytes.extend(identity.generator.to_le_bytes());
        bytes.extend(0_u32.to_le_bytes());
        for (&p, &block) in edits.iter() {
            if !world.in_bounds(p) {
                return Err(FormatError::Coordinates);
            }
            for c in p.0 {
                bytes.extend(c.to_le_bytes());
            }
            bytes.extend(block_id(block).to_le_bytes());
        }
        let crc = checksum(&bytes);
        bytes[20..24].copy_from_slice(&crc.to_le_bytes());
        return Ok(bytes);
    }
    let mut records = Vec::new();
    if world.chunks().count() > MAX_RECORDS / (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) {
        return Err(FormatError::TooLarge);
    }
    for (position, chunk) in world.chunks() {
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let local = LocalPos::new([x, y, z]).expect("bounded local coordinates");
                    let p = position
                        .block_position(local)
                        .ok_or(FormatError::Coordinates)?
                        .0;
                    if !in_bounds(p) {
                        return Err(FormatError::Coordinates);
                    }
                    let block = chunk.get(local);
                    if block.is_solid() {
                        records.push((p, block_id(block)));
                    }
                }
            }
        }
    }
    records.sort_unstable_by_key(|(p, _)| *p);
    let payload_length = records.len() * RECORD_SIZE;
    let mut bytes = Vec::with_capacity(HEADER_SIZE + payload_length);
    bytes.extend(MAGIC);
    bytes.extend(FORMAT_VERSION.to_le_bytes());
    bytes.extend((records.len() as u32).to_le_bytes());
    bytes.extend((payload_length as u32).to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    for (p, id) in records {
        for coordinate in p {
            bytes.extend(coordinate.to_le_bytes());
        }
        bytes.extend(id.to_le_bytes());
    }
    let crc = checksum(&bytes);
    bytes[20..24].copy_from_slice(&crc.to_le_bytes());
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<World, FormatError> {
    if bytes.len() > MAX_FILE_SIZE {
        return Err(FormatError::TooLarge);
    }
    if bytes.len() < HEADER_SIZE {
        return Err(FormatError::Length);
    }
    if &bytes[..8] != MAGIC {
        return Err(FormatError::Magic);
    }
    let u32_at = |offset| {
        u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("header length checked"),
        )
    };
    let version = u32_at(8);
    if version == REGIONAL_VERSION {
        return regional::decode_manifest(bytes);
    }
    if ![1, FORMAT_VERSION, STREAM_FORMAT_VERSION].contains(&version) {
        return Err(FormatError::Version(version));
    }
    let count = u32_at(12) as usize;
    if count
        > if version == STREAM_FORMAT_VERSION {
            MAX_STREAM_RECORDS
        } else {
            MAX_RECORDS
        }
    {
        return Err(FormatError::TooLarge);
    }
    let length = u32_at(16) as usize;
    let header = if version == STREAM_FORMAT_VERSION {
        STREAM_HEADER_SIZE
    } else {
        HEADER_SIZE
    };
    if length != count * RECORD_SIZE || bytes.len() != header + length {
        return Err(FormatError::Length);
    }
    if u32_at(20) != checksum(bytes) {
        return Err(FormatError::Checksum);
    }
    let mut world = if version == STREAM_FORMAT_VERSION {
        let seed = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        let generator = u32_at(32);
        if generator != 2 || u32_at(36) != 0 {
            return Err(FormatError::Identity);
        }
        World::streaming(crate::StreamIdentity { seed, generator })
    } else {
        World::default()
    };
    let mut previous = None;
    for record in bytes[header..].as_chunks::<RECORD_SIZE>().0 {
        let p: [i32; 3] = std::array::from_fn(|axis| {
            i32::from_le_bytes(
                record[axis * 4..axis * 4 + 4]
                    .try_into()
                    .expect("fixed record size"),
            )
        });
        if !world.in_bounds(BlockPos(p)) {
            return Err(FormatError::Coordinates);
        }
        if previous.is_some_and(|old| p <= old) {
            return Err(FormatError::UnsortedOrDuplicate);
        }
        previous = Some(p);
        let block = stored_block(
            u16::from_le_bytes(record[12..14].try_into().expect("fixed record size")),
            version,
        )?;
        if version == STREAM_FORMAT_VERSION {
            world.restore_edit(BlockPos(p), block);
        } else {
            world
                .set_block(BlockPos(p), block)
                .map_err(|_| FormatError::Coordinates)?;
        }
    }
    Ok(world)
}

#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    Invalid(FormatError),
    RecoveryRequired(String),
}
impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Invalid(error) => write!(f, "{error}"),
            Self::RecoveryRequired(message) => write!(f, "{message}"),
        }
    }
}
impl std::error::Error for SaveError {}
impl From<io::Error> for SaveError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<FormatError> for SaveError {
    fn from(error: FormatError) -> Self {
        Self::Invalid(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadSource {
    NewDemo,
    NewWorld,
    Saved,
}

/// Holds an exclusive OS file lock for the session. Do not delete world.lock
/// while instances run. Temporary files are never used as implicit load data.
pub struct SaveStore {
    directory: PathBuf,
    _lock: File,
}
impl SaveStore {
    pub fn open(directory: impl AsRef<Path>) -> Result<(Self, World, LoadSource), SaveError> {
        Self::open_using(directory, || Ok((demo_world(), LoadSource::NewDemo)))
    }
    /// The factory is evaluated ONLY when neither primary nor backup exists.
    /// Loaded worlds never consult seeds or a generator, and are never rewritten.
    pub fn open_with(
        directory: impl AsRef<Path>,
        create: impl FnOnce() -> Result<World, SaveError>,
    ) -> Result<(Self, World, LoadSource), SaveError> {
        Self::open_using(directory, || {
            create().map(|world| (world, LoadSource::NewWorld))
        })
    }
    fn open_using(
        directory: impl AsRef<Path>,
        create: impl FnOnce() -> Result<(World, LoadSource), SaveError>,
    ) -> Result<(Self, World, LoadSource), SaveError> {
        let directory = directory.as_ref().to_path_buf();
        if !directory.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "data directory must be absolute",
            )
            .into());
        }
        fs::create_dir_all(&directory)?;
        let store = Self::lock_directory(&directory)?;
        let (world, source) = match read_save(&store.path()) {
            Ok(Some(bytes)) => (decode(&bytes)?, LoadSource::Saved),
            Ok(None) if store.backup_path().try_exists()? => {
                return Err(SaveError::RecoveryRequired(format!(
                    "primary save missing; backup retained at {}. Recover it explicitly before restarting",
                    store.backup_path().display()
                )));
            }
            Ok(None) if store.directory.join("session.pending").try_exists()? => {
                (store.pending_world()?, LoadSource::Saved)
            }
            Ok(None)
                if store.directory.join("player.rcp").try_exists()?
                    || store.directory.join("player-world.rcid").try_exists()?
                    || store.directory.join("player.rcp.bak").try_exists()? =>
            {
                return Err(SaveError::RecoveryRequired(
                    "player files exist without world or recovery journal".into(),
                ));
            }
            Ok(None) if store.directory.join("regions").try_exists()? => {
                return Err(SaveError::RecoveryRequired(
                    "region directory exists without a world manifest; refusing new identity"
                        .into(),
                ));
            }
            Ok(None) => create()?,
            Err(error) => return Err(error),
        };
        Ok((store, world, source))
    }
    /// Acquire a bundle lock without loading, replaying or initializing data.
    pub(super) fn lock_directory(directory: &Path) -> Result<Self, SaveError> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("world.lock"))?;
        lock.try_lock().map_err(|e| {
            io::Error::other(format!("world already in use or lock unavailable: {e}"))
        })?;
        Ok(Self {
            directory: directory.to_path_buf(),
            _lock: lock,
        })
    }
    /// Open existing data only. Missing/corrupt data can never invoke a factory.
    pub fn open_existing(
        directory: impl AsRef<Path>,
    ) -> Result<(Self, World, LoadSource), SaveError> {
        Self::open_with(directory, || {
            Err(SaveError::RecoveryRequired(
                "existing world missing; creation refused".into(),
            ))
        })
    }
    pub fn path(&self) -> PathBuf {
        self.directory.join("world.rcw")
    }
    pub fn backup_path(&self) -> PathBuf {
        self.directory.join("world.rcw.bak")
    }
    pub fn save(&self, world: &World) -> Result<(), SaveError> {
        if self.directory.join("player-world.rcid").try_exists()?
            || self.directory.join("player.rcp").try_exists()?
            || self.directory.join("session.pending").try_exists()?
        {
            return Err(SaveError::RecoveryRequired(
                "paired player state exists; use save_player_session rather than world-only saving"
                    .into(),
            ));
        }
        self.save_with(world, |_| Ok(()))
    }

    fn save_with(
        &self,
        world: &World,
        mut hook: impl FnMut(SaveStage) -> io::Result<()>,
    ) -> Result<(), SaveError> {
        if world.region_identity().is_some() {
            return self.save_regional(world, &mut hook);
        }
        let bytes = encode(world)?;
        match read_save(&self.path())? {
            Some(previous) => {
                let prior = decode(&previous)?; // Never replace corrupt/unknown data.
                if prior.identity() != world.identity()
                    || prior.region_identity() != world.region_identity()
                {
                    return Err(SaveError::RecoveryRequired(
                        "world identity/mode changed; refusing implicit migration".into(),
                    ));
                }
                atomic_replace(&self.backup_path(), &previous, false, &mut hook)?;
                hook(SaveStage::BackupCommitted)?;
            }
            None if self.backup_path().try_exists()? => {
                return Err(SaveError::RecoveryRequired(
                    "primary disappeared while backup exists; refusing to overwrite recovery state"
                        .into(),
                ));
            }
            None => {}
        }
        atomic_replace(&self.path(), &bytes, true, &mut hook)?;
        Ok(())
    }
}

fn read_save(path: &Path) -> Result<Option<Vec<u8>>, SaveError> {
    read_limited(path, MAX_FILE_SIZE)
}
fn read_limited(path: &Path, limit: usize) -> Result<Option<Vec<u8>>, SaveError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "save path is not a regular file",
        )
        .into());
    }
    if file.metadata()?.len() > limit as u64 {
        return Err(FormatError::TooLarge.into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(FormatError::TooLarge.into());
    }
    Ok(Some(bytes))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SaveStage {
    PartialTemp,
    TempSynced,
    BackupCommitted,
    BeforePrimaryRename,
    PrimaryCommitted,
    SessionPrepared,
    WorldSaved,
    PlayerSaved,
}
struct TempFile {
    path: PathBuf,
    file: Option<File>,
}
impl Drop for TempFile {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn atomic_replace(
    path: &Path,
    bytes: &[u8],
    primary: bool,
    hook: &mut impl FnMut(SaveStage) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "save needs parent directory")
    })?;
    let mut temp = (0..16)
        .find_map(|_| {
            let mut name = path.file_name()?.to_os_string();
            name.push(format!(
                ".tmp.{}.{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            let temp = parent.join(name);
            match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => Some(Ok(TempFile {
                    path: temp,
                    file: Some(file),
                })),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "temporary file name collision",
            ))
        })?;
    let file = temp.file.as_mut().expect("new temporary file");
    let split = bytes.len() / 2;
    file.write_all(&bytes[..split])?;
    hook(SaveStage::PartialTemp)?;
    file.write_all(&bytes[split..])?;
    file.flush()?;
    file.sync_all()?;
    hook(SaveStage::TempSynced)?;
    drop(temp.file.take()); // Close before rename on every platform.
    if primary {
        hook(SaveStage::BeforePrimaryRename)?;
    }
    fs::rename(&temp.path, path)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    if primary {
        hook(SaveStage::PrimaryCommitted)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
