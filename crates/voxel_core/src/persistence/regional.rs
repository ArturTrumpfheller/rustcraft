//! v4 identity manifests and individually atomic regional overlays.
use super::*;
use crate::regions::{LARGE_EXTENT, MAX_REGION_RECORDS, RegionEdits, RegionIdentity, RegionPos};
use std::{collections::BTreeMap, sync::Arc};
pub const REGIONAL_VERSION: u32 = 4;
const SIZE: usize = 64;
const REGION_MAGIC: &[u8; 8] = b"RUSTREGN";
const MAX_REGION_FILE: usize = SIZE + MAX_REGION_RECORDS * RECORD_SIZE;
fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
}
fn finish(mut b: Vec<u8>) -> Vec<u8> {
    let crc = checksum(&b);
    b[20..24].copy_from_slice(&crc.to_le_bytes());
    b
}
pub(super) fn manifest(identity: RegionIdentity) -> Vec<u8> {
    let mut b = vec![0; SIZE];
    b[..8].copy_from_slice(MAGIC);
    b[8..12].copy_from_slice(&REGIONAL_VERSION.to_le_bytes());
    b[24..32].copy_from_slice(&identity.seed.to_le_bytes());
    b[32..36].copy_from_slice(&3u32.to_le_bytes());
    b[40..44].copy_from_slice(&(LARGE_EXTENT as u32).to_le_bytes());
    b[44..60].copy_from_slice(&identity.nonce);
    finish(b)
}
pub(super) fn decode_manifest(b: &[u8]) -> Result<World, FormatError> {
    if b.len() != SIZE {
        return Err(FormatError::Length);
    }
    if u32_at(b, 20) != checksum(b) {
        return Err(FormatError::Checksum);
    }
    if u32_at(b, 12) != 0
        || u32_at(b, 16) != 0
        || u32_at(b, 32) != 3
        || u32_at(b, 36) != 0
        || u32_at(b, 40) != LARGE_EXTENT as u32
        || u32_at(b, 60) != 0
        || b[44..60] == [0; 16]
    {
        return Err(FormatError::Identity);
    }
    Ok(World::regional(RegionIdentity {
        seed: u64_at(b, 24),
        nonce: b[44..60].try_into().unwrap(),
    }))
}
fn encode_region(
    id: RegionIdentity,
    p: RegionPos,
    edits: &RegionEdits,
) -> Result<Vec<u8>, FormatError> {
    if !p.valid() || edits.len() > MAX_REGION_RECORDS {
        return Err(FormatError::TooLarge);
    }
    let mut b = vec![0; SIZE];
    b[..8].copy_from_slice(REGION_MAGIC);
    b[8..12].copy_from_slice(&1u32.to_le_bytes());
    b[12..16].copy_from_slice(&(edits.len() as u32).to_le_bytes());
    b[16..20].copy_from_slice(&(edits.len() as u32 * 14).to_le_bytes());
    b[24..40].copy_from_slice(&id.nonce);
    b[40..48].copy_from_slice(&id.seed.to_le_bytes());
    b[48..52].copy_from_slice(&3u32.to_le_bytes());
    b[52..56].copy_from_slice(&p.0[0].to_le_bytes());
    b[56..60].copy_from_slice(&p.0[1].to_le_bytes());
    for (&pos, &block) in edits.iter() {
        if RegionPos::of(pos) != p || !(-16..32).contains(&pos.0[1]) {
            return Err(FormatError::Coordinates);
        }
        for v in pos.0 {
            b.extend(v.to_le_bytes());
        }
        b.extend(block_id(block).to_le_bytes());
    }
    Ok(finish(b))
}
fn decode_region(b: &[u8], id: RegionIdentity, p: RegionPos) -> Result<RegionEdits, FormatError> {
    if b.len() > MAX_REGION_FILE {
        return Err(FormatError::TooLarge);
    }
    if b.len() < SIZE {
        return Err(FormatError::Length);
    }
    if &b[..8] != REGION_MAGIC {
        return Err(FormatError::Magic);
    }
    if u32_at(b, 8) != 1 {
        return Err(FormatError::Version(u32_at(b, 8)));
    }
    let count = u32_at(b, 12) as usize;
    if count > MAX_REGION_RECORDS
        || u32_at(b, 16) as usize != count * 14
        || b.len() != SIZE + count * 14
    {
        return Err(FormatError::Length);
    }
    if u32_at(b, 20) != checksum(b) {
        return Err(FormatError::Checksum);
    }
    if !p.valid()
        || b[24..40] != id.nonce
        || u64_at(b, 40) != id.seed
        || u32_at(b, 48) != 3
        || u32_at(b, 60) != 0
    {
        return Err(FormatError::Identity);
    }
    if b[52..56] != p.0[0].to_le_bytes() || b[56..60] != p.0[1].to_le_bytes() {
        return Err(FormatError::Coordinates);
    }
    let mut edits = BTreeMap::new();
    let mut previous = None;
    for r in b[SIZE..].as_chunks::<14>().0 {
        let pos = BlockPos(std::array::from_fn(|i| {
            i32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())
        }));
        if RegionPos::of(pos) != p || !(-16..32).contains(&pos.0[1]) {
            return Err(FormatError::Coordinates);
        }
        if previous.is_some_and(|old| pos <= old) {
            return Err(FormatError::UnsortedOrDuplicate);
        }
        previous = Some(pos);
        edits.insert(
            pos,
            stored_block(
                u16::from_le_bytes(r[12..14].try_into().unwrap()),
                STREAM_FORMAT_VERSION,
            )?,
        );
    }
    Ok(Arc::new(edits))
}
impl SaveStore {
    pub fn region_path(&self, p: RegionPos) -> PathBuf {
        self.directory
            .join("regions")
            .join(format!(
                "{}_{}",
                p.0[0].div_euclid(32),
                p.0[1].div_euclid(32)
            ))
            .join(format!("{}_{}.rcw", p.0[0], p.0[1]))
    }
    pub fn read_region(&self, id: RegionIdentity, p: RegionPos) -> Result<RegionEdits, SaveError> {
        if !p.valid() {
            return Err(FormatError::Coordinates.into());
        }
        let path = self.region_path(p);
        match read_limited(&path, MAX_REGION_FILE)? {
            Some(b) => Ok(decode_region(&b, id, p)?),
            None if path.with_extension("rcw.bak").try_exists()? => {
                Err(SaveError::RecoveryRequired(format!(
                    "missing region primary {}; restore its backup explicitly",
                    path.display()
                )))
            }
            None => Ok(Arc::new(BTreeMap::new())),
        }
    }
    pub(super) fn save_regional(
        &self,
        world: &World,
        hook: &mut impl FnMut(SaveStage) -> io::Result<()>,
    ) -> Result<(), SaveError> {
        let id = world.region_identity().unwrap();
        let bytes = manifest(id);
        match read_save(&self.path())? {
            Some(old) => {
                if decode(&old)?.region_identity() != Some(id) {
                    return Err(SaveError::RecoveryRequired(
                        "regional world identity changed; migration refused".into(),
                    ));
                }
            }
            None if self.backup_path().try_exists()? => {
                return Err(SaveError::RecoveryRequired(
                    "missing world manifest with backup; recover explicitly".into(),
                ));
            }
            None => atomic_replace(&self.path(), &bytes, true, hook)?,
        }
        #[cfg(unix)]
        File::open(&self.directory)?.sync_all()?;
        for (p, edits) in world.dirty_region_snapshots() {
            let path = self.region_path(p);
            let backup = path.with_extension("rcw.bak");
            let bytes = encode_region(id, p, &edits)?;
            match read_limited(&path, MAX_REGION_FILE)? {
                Some(old) => {
                    decode_region(&old, id, p)?;
                    if old == bytes {
                        #[cfg(unix)]
                        File::open(path.parent().unwrap())?.sync_all()?;
                        continue;
                    } // Idempotent retry retains the older backup.
                    atomic_replace(&backup, &old, false, hook)?;
                    hook(SaveStage::BackupCommitted)?;
                }
                None if backup.try_exists()? => {
                    return Err(SaveError::RecoveryRequired(format!(
                        "missing region {}; recovery required",
                        path.display()
                    )));
                }
                None => {
                    fs::create_dir_all(path.parent().unwrap())?;
                    // Persist newly created directory entries as well as files.
                    #[cfg(unix)]
                    for dir in [
                        path.parent().unwrap().to_path_buf(),
                        self.directory.join("regions"),
                        self.directory.clone(),
                    ] {
                        File::open(dir)?.sync_all()?;
                    }
                }
            }
            atomic_replace(&path, &bytes, true, hook)?;
        }
        Ok(())
    }
    /// Bounded spawn-region read before App startup. Normal travel reads on a
    /// worker, and never consults disk from block/physics queries.
    pub fn prepare_regional_spawn(&self, world: &mut World) -> Result<(), SaveError> {
        self.prepare_regional_column(world, [0, 0])
    }
    pub fn prepare_regional_column(
        &self,
        world: &mut World,
        column: [i32; 2],
    ) -> Result<(), SaveError> {
        if let Some(id) = world.region_identity() {
            let p = RegionPos([
                column[0].div_euclid(crate::regions::REGION_CHUNKS),
                column[1].div_euclid(crate::regions::REGION_CHUNKS),
            ]);
            world.install_region(p, self.read_region(id, p)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
