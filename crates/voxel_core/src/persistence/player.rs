//! Versioned player state and a bounded save-level redo journal. World formats
//! remain unchanged. A pending known transaction is replayed before gameplay.
use super::*;
use crate::{
    inventory::{Inventory, Item, SLOT_COUNT, Stack},
    regions::{MAX_DIRTY_REGIONS, RegionPos},
};
const ID_MAGIC: &[u8; 8] = b"RUSTPID1";
const PLAYER_MAGIC: &[u8; 8] = b"RUSTPLY1";
const TX_MAGIC: &[u8; 8] = b"RUSTTXN1";
const PLAYER_SIZE: usize = 176;
const MAX_TX: usize = 32 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding([u8; 32]);
fn u32at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn pack(magic: &[u8; 8], count: u32, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(24 + payload.len());
    b.extend(magic);
    b.extend(1u32.to_le_bytes());
    b.extend(count.to_le_bytes());
    b.extend((payload.len() as u32).to_le_bytes());
    b.extend(0u32.to_le_bytes());
    b.extend(payload);
    let crc = checksum(&b);
    b[20..24].copy_from_slice(&crc.to_le_bytes());
    b
}
fn unpack<'a>(b: &'a [u8], magic: &[u8; 8], count: u32) -> Result<&'a [u8], FormatError> {
    if b.len() < 24 {
        return Err(FormatError::Length);
    }
    if &b[..8] != magic {
        return Err(FormatError::Magic);
    }
    if u32at(b, 8) != 1 {
        return Err(FormatError::Version(u32at(b, 8)));
    }
    if u32at(b, 12) != count || u32at(b, 16) as usize != b.len() - 24 {
        return Err(FormatError::Length);
    }
    if u32at(b, 20) != checksum(b) {
        return Err(FormatError::Checksum);
    }
    Ok(&b[24..])
}
impl Binding {
    fn new(w: &World) -> Self {
        let mut b = [0; 32];
        let mode = if w.region_identity().is_some() {
            4u32
        } else if w.identity().is_some() {
            3
        } else {
            2
        };
        b[..4].copy_from_slice(&mode.to_le_bytes());
        if let Some(i) = w.identity() {
            b[4..12].copy_from_slice(&i.seed.to_le_bytes());
            b[12..16].copy_from_slice(&i.generator.to_le_bytes());
        }
        let nonce = w.region_identity().map_or_else(
            || {
                static NEXT: AtomicU64 = AtomicU64::new(1);
                let t = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                (t ^ (u128::from(std::process::id()) << 64)
                    ^ u128::from(NEXT.fetch_add(1, Ordering::Relaxed)))
                .to_le_bytes()
            },
            |i| i.nonce,
        );
        b[16..].copy_from_slice(&nonce);
        Self(b)
    }
    fn matches(self, w: &World) -> bool {
        let mode = u32at(&self.0, 0);
        let matches = match mode {
            2 => w.identity().is_none() && self.0[4..16] == [0; 12],
            3 => {
                w.region_identity().is_none()
                    && w.identity().is_some_and(|i| {
                        self.0[4..12] == i.seed.to_le_bytes()
                            && self.0[12..16] == i.generator.to_le_bytes()
                    })
            }
            4 => w.region_identity().is_some_and(|i| {
                self.0[4..12] == i.seed.to_le_bytes()
                    && self.0[12..16] == 3u32.to_le_bytes()
                    && self.0[16..] == i.nonce
            }),
            _ => false,
        };
        matches && self.0[16..] != [0; 16]
    }
}
fn fingerprint(root: &[u8]) -> [u8; 8] {
    let mut b = [0; 8];
    b[..4].copy_from_slice(&(root.len() as u32).to_le_bytes());
    b[4..].copy_from_slice(&crc32fast::hash(root).to_le_bytes());
    b
}
fn player_bytes(binding: Binding, root: &[u8], inv: &Inventory) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend(binding.0);
    p.extend(fingerprint(root));
    p.push(inv.selected() as u8);
    for s in inv.slots().iter().copied().chain([inv.cursor()]) {
        if let Some(s) = s {
            p.extend(s.item().id().to_le_bytes());
            p.push(s.count());
        } else {
            p.extend([0; 3]);
        }
    }
    pack(PLAYER_MAGIC, SLOT_COUNT as u32, &p)
}
fn decode_player(b: &[u8]) -> Result<(Binding, [u8; 8], Inventory), FormatError> {
    if b.len() != PLAYER_SIZE {
        return Err(FormatError::Length);
    }
    let p = unpack(b, PLAYER_MAGIC, SLOT_COUNT as u32)?;
    let binding = Binding(p[..32].try_into().unwrap());
    let fingerprint = p[32..40].try_into().unwrap();
    let mut stacks = [None; SLOT_COUNT + 1];
    for (i, r) in p[41..].as_chunks::<3>().0.iter().enumerate() {
        let id = u16::from_le_bytes([r[0], r[1]]);
        stacks[i] = if id == 0 && r[2] == 0 {
            None
        } else {
            Some(
                Stack::new(Item::from_id(id).ok_or(FormatError::BlockId(id))?, r[2])
                    .ok_or(FormatError::Length)?,
            )
        };
    }
    let cursor = stacks[SLOT_COUNT];
    let slots = stacks[..SLOT_COUNT].try_into().unwrap();
    let inv =
        Inventory::from_parts(slots, cursor, usize::from(p[40])).ok_or(FormatError::Length)?;
    Ok((binding, fingerprint, inv))
}
struct Transaction {
    world: World,
    root: Vec<u8>,
    player: Vec<u8>,
    world_dirty: bool,
    binding: Binding,
}
fn transaction_bytes(
    world: &World,
    root: &[u8],
    player: &[u8],
    dirty: bool,
) -> Result<Vec<u8>, FormatError> {
    let mut p = vec![u8::from(dirty)];
    p.extend((root.len() as u32).to_le_bytes());
    p.extend(root);
    let regions: Vec<_> = world.dirty_region_snapshots().collect();
    p.extend((regions.len() as u32).to_le_bytes());
    for (pos, map) in regions {
        let b = super::regional::encode_region(world.region_identity().unwrap(), pos, &map)?;
        p.extend(pos.0[0].to_le_bytes());
        p.extend(pos.0[1].to_le_bytes());
        p.extend((b.len() as u32).to_le_bytes());
        p.extend(b);
    }
    p.extend((player.len() as u32).to_le_bytes());
    p.extend(player);
    if p.len() + 24 > MAX_TX {
        return Err(FormatError::TooLarge);
    }
    Ok(pack(TX_MAGIC, 0, &p))
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], FormatError> {
        let b = self.0.get(..n).ok_or(FormatError::Length)?;
        self.0 = &self.0[n..];
        Ok(b)
    }
    fn num(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn bytes(&mut self) -> Result<&'a [u8], FormatError> {
        let n = self.num()? as usize;
        self.take(n)
    }
}
fn decode_transaction(b: &[u8]) -> Result<Transaction, FormatError> {
    if b.len() > MAX_TX {
        return Err(FormatError::TooLarge);
    }
    let mut r = Reader(unpack(b, TX_MAGIC, 0)?);
    let dirty = match r.take(1)?[0] {
        0 => false,
        1 => true,
        _ => return Err(FormatError::Length),
    };
    let root = r.bytes()?.to_vec();
    let mut world = decode(&root)?;
    let n = r.num()? as usize;
    if n > MAX_DIRTY_REGIONS {
        return Err(FormatError::TooLarge);
    }
    let mut previous = None;
    for _ in 0..n {
        let pos = RegionPos([r.num()? as i32, r.num()? as i32]);
        if previous.is_some_and(|p| pos <= p) {
            return Err(FormatError::UnsortedOrDuplicate);
        }
        previous = Some(pos);
        let id = world.region_identity().ok_or(FormatError::Identity)?;
        let edits = super::regional::decode_region(r.bytes()?, id, pos)?;
        world.restore_dirty_region(pos, edits);
    }
    let player = r.bytes()?.to_vec();
    if !r.0.is_empty() {
        return Err(FormatError::Length);
    }
    let (binding, fp, _) = decode_player(&player)?;
    if !binding.matches(&world) || fp != fingerprint(&root) || (!dirty && n != 0) {
        return Err(FormatError::Identity);
    }
    Ok(Transaction {
        world,
        root,
        player,
        world_dirty: dirty,
        binding,
    })
}
fn recovery(message: &str) -> SaveError {
    SaveError::RecoveryRequired(message.into())
}
impl SaveStore {
    fn player_path(&self) -> PathBuf {
        self.directory.join("player.rcp")
    }
    fn pending_path(&self) -> PathBuf {
        self.directory.join("session.pending")
    }
    fn binding_path(&self) -> PathBuf {
        self.directory.join("player-world.rcid")
    }
    fn binding(&self) -> Result<Option<Binding>, SaveError> {
        let Some(b) = read_limited(&self.binding_path(), 56)? else {
            return Ok(None);
        };
        let p = unpack(&b, ID_MAGIC, 0)?;
        if p.len() != 32 {
            return Err(FormatError::Length.into());
        }
        Ok(Some(Binding(p.try_into().unwrap())))
    }
    /// Load/recover before spawn/cache preparation. Missing initialized files fail
    /// closed; a valid pending journal is replayed, never treated as new inventory.
    pub fn load_player(&self, world: &mut World) -> Result<Inventory, SaveError> {
        if let Some(b) = read_limited(&self.pending_path(), MAX_TX)? {
            let tx = decode_transaction(&b)?;
            let id = self.binding()?.ok_or_else(|| {
                recovery("pending session has no player-world identity; recover whole directory")
            })?;
            if tx.binding != id || !id.matches(world) {
                return Err(FormatError::Identity.into());
            }
            self.apply_transaction(&tx, &mut |_| Ok(()))?;
            *world = decode(
                &read_save(&self.path())?.ok_or_else(|| recovery("world missing after replay"))?,
            )?;
        }
        let file = read_limited(&self.player_path(), PLAYER_SIZE)?;
        let binding = self.binding()?;
        if let Some(b) = file {
            let (id, fp, inv) = decode_player(&b)?;
            if binding != Some(id) || !id.matches(world) {
                return Err(FormatError::Identity.into());
            }
            let root =
                read_save(&self.path())?.ok_or_else(|| recovery("player exists without world"))?;
            if fp != fingerprint(&root) {
                return Err(recovery(
                    "world/player generations disagree; recover a complete backup",
                ));
            }
            return Ok(inv);
        }
        if binding.is_some() || self.player_path().with_extension("rcp.bak").try_exists()? {
            return Err(recovery(
                "initialized player primary missing; restore matching backup or complete directory",
            ));
        }
        let id = Binding::new(world);
        atomic_replace(
            &self.binding_path(),
            &pack(ID_MAGIC, 0, &id.0),
            true,
            &mut |_| Ok(()),
        )?;
        let inv = Inventory::starter();
        self.save_player_session(world, &inv, read_save(&self.path())?.is_none())?;
        Ok(inv)
    }
    pub fn save_player_session(
        &self,
        world: &World,
        inventory: &Inventory,
        world_dirty: bool,
    ) -> Result<(), SaveError> {
        self.save_player_with(world, inventory, world_dirty, |_| Ok(()))
    }
    fn save_player_with(
        &self,
        world: &World,
        inventory: &Inventory,
        world_dirty: bool,
        mut hook: impl FnMut(SaveStage) -> io::Result<()>,
    ) -> Result<(), SaveError> {
        let binding = self.binding()?.ok_or_else(|| {
            recovery("player-world identity missing; refuse initialization during save")
        })?;
        if !binding.matches(world) {
            return Err(FormatError::Identity.into());
        }
        if let Some(b) = read_limited(&self.pending_path(), MAX_TX)?
            && decode_transaction(&b)?.binding != binding
        {
            return Err(FormatError::Identity.into());
        }
        let root = if world_dirty {
            encode(world)?
        } else {
            read_save(&self.path())?
                .ok_or_else(|| recovery("clean session has no durable world"))?
        };
        let player = player_bytes(binding, &root, inventory);
        let bytes = transaction_bytes(world, &root, &player, world_dirty)?;
        atomic_replace(&self.pending_path(), &bytes, true, &mut hook)?;
        hook(SaveStage::SessionPrepared)?;
        self.apply_transaction(&decode_transaction(&bytes)?, &mut hook)
    }
    fn apply_transaction(
        &self,
        tx: &Transaction,
        hook: &mut impl FnMut(SaveStage) -> io::Result<()>,
    ) -> Result<(), SaveError> {
        if !tx.binding.matches(&tx.world) {
            return Err(FormatError::Identity.into());
        }
        if tx.world_dirty {
            // Idempotent legacy retries must not rotate a valid older backup away.
            if tx.world.region_identity().is_some()
                || read_save(&self.path())?.as_deref() != Some(&tx.root)
            {
                self.save_with(&tx.world, &mut *hook)?;
            }
        } else if read_save(&self.path())?.as_deref() != Some(&tx.root) {
            return Err(recovery("clean-world journal generation mismatch"));
        }
        hook(SaveStage::WorldSaved)?;
        let path = self.player_path();
        let backup = path.with_extension("rcp.bak");
        match read_limited(&path, PLAYER_SIZE)? {
            Some(old) => {
                if decode_player(&old)?.0 != tx.binding {
                    return Err(FormatError::Identity.into());
                }
                if old != tx.player {
                    atomic_replace(&backup, &old, false, hook)?;
                    atomic_replace(&path, &tx.player, true, hook)?;
                }
            }
            None if backup.try_exists()? => {
                return Err(recovery(
                    "missing player primary with backup; explicit recovery required",
                ));
            }
            None => atomic_replace(&path, &tx.player, true, hook)?,
        }
        hook(SaveStage::PlayerSaved)?;
        fs::remove_file(self.pending_path())?;
        #[cfg(unix)]
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }
    pub(super) fn pending_world(&self) -> Result<World, SaveError> {
        let b = read_limited(&self.pending_path(), MAX_TX)?
            .ok_or_else(|| recovery("pending session vanished"))?;
        Ok(decode_transaction(&b)?.world)
    }
}
#[cfg(test)]
mod tests;
