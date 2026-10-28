//! Original RustCraft 16x16 pixel artwork, MIT OR Apache-2.0.
//! Shared by UI and thin held-item meshes; no terrain cube/texture reuse.
use crate::inventory::Item;
pub const BUCKET: [&str; 16] = [
    "................",
    ".....222222.....",
    "...22......22...",
    "..2..........2..",
    "..2..........2..",
    "..211111111112..",
    "..134444444431..",
    "...1344444431...",
    "...1355555431...",
    "...1355555431...",
    "....13555431....",
    "....13555431....",
    "....13555431....",
    ".....133331.....",
    "......1111......",
    "................",
];
pub fn bucket_pixel(item: Item, x: usize, y: usize) -> Option<[u8; 4]> {
    if !matches!(
        item,
        Item::EmptyBucket | Item::WaterBucket | Item::WaterSource
    ) {
        return None;
    }
    let mut symbol = BUCKET.get(y)?.as_bytes().get(x).copied()?;
    if matches!(item, Item::WaterBucket | Item::WaterSource)
        && (6..=8).contains(&y)
        && (4..=11).contains(&x)
        && matches!(symbol, b'3' | b'4' | b'5')
    {
        symbol = if y == 6 || (x + y).is_multiple_of(5) {
            b'7'
        } else {
            b'6'
        };
    }
    Some(match symbol {
        b'1' => [41, 51, 64, 255],
        b'2' => [111, 130, 142, 255],
        b'3' => [190, 208, 214, 255],
        b'4' => [69, 83, 96, 255],
        b'5' => [137, 157, 168, 255],
        b'6' => [38, 91, 194, 255],
        b'7' => [86, 161, 226, 255],
        _ => return None,
    })
}
/// Tileable stepped ripples, authored on a 16x16 grid without external assets.
pub const WATER: [&str; 16] = [
    "1111122221111111",
    "1112233322111111",
    "1123311122111111",
    "1222111111111111",
    "1111111111222211",
    "1111111122333221",
    "2111111223111122",
    "2211111221111112",
    "1222111111111111",
    "1123322111111111",
    "1112233221111111",
    "1111122211111111",
    "1111111111122211",
    "1111111112233321",
    "1111111122311122",
    "1111111122111112",
];
pub fn water_pixels() -> Vec<u8> {
    let palette = [[38, 79, 162, 255], [42, 91, 178, 255], [53, 112, 194, 255]];
    WATER
        .iter()
        .flat_map(|row| row.bytes().flat_map(|b| palette[usize::from(b - b'1')]))
        .collect()
}
