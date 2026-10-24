#!/usr/bin/env python3
"""Export exact Slice 17 sources and adapt only the new gallery harness to them."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

BASE = 'dc519cdbd6f1d37d037da514ce7144b1d7b749de'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path, help='new, nonexistent directory')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryFile() as archive:
        subprocess.run(['git', 'archive', BASE], cwd=root, stdout=archive, check=True)
        archive.seek(0)
        with tarfile.open(fileobj=archive) as bundle:
            bundle.extractall(args.output, filter='data')
    for name in ['atmosphere.rs', 'atmosphere.wesl', 'sky.wesl']:
        shutil.copyfile(root / 'client/src' / name, args.output / 'client/src' / name)
    gallery = (root / 'client/examples/atmosphere_gallery.rs').read_text()
    # Old material has no atmosphere binding. Original terrain shader is untouched.
    gallery = gallery.replace('            atmosphere: params,\n', '')
    (args.output / 'client/examples/atmosphere_gallery.rs').write_text(gallery)
    atmosphere = args.output / 'client/src/atmosphere.rs'
    source = atmosphere.read_text().split('pub(crate) fn update_materials(', 1)[0]
    atmosphere.write_text(source + '''pub(crate) fn update_materials(
    _params: AtmosphereUniform,
    _terrain: &Handle<crate::terrain_material::TerrainSurface>,
    _sky: Option<&Handle<SkyMaterial>>,
    _materials: &mut Assets<crate::terrain_material::TerrainSurface>,
    _sky_materials: &mut Assets<SkyMaterial>,
) -> usize { 0 }
''')
    print(f'Prepared {BASE} at {args.output.resolve()}; build gallery, run with --baseline.')


if __name__ == '__main__':
    main()
