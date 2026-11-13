#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Validate measured survey materials/snowline and actual ecosystem support/water.
Usage: check_biome_v2.py SURVEY.csv ECOSYSTEMS.csv OUTPUT.json
Stable existing block IDs are read from the CSV, not inferred from image colors.
"""
from pathlib import Path
import csv,collections,json,sys
survey, ecosystems, output = map(Path, sys.argv[1:])
c=collections.Counter()
for name in [survey]:
 for r in csv.DictReader(name.open()):
  b,h,sl,top,e,line=(int(r[k]) for k in ['biome','height','slope','top','elevation_q10','snowline'])
  c['samples']+=1
  eco=min((e*3+h*1024)//4,(h+8)*1024)
  if b==9:
   c['snowy_slope_samples']+=1;c['snowline_violations']+=eco<line*1024
   if sl<3:c['gentle_snowy_slope_samples']+=1;c['missing_snow_on_gentle_slopes']+=top!=21
  if b==10:c['stony_peak_samples']+=1;c['non_stone_peaks']+=top!=3
  c['grass_on_cliff']+=sl>=3 and top==1
  c['sand_on_highland']+=h>=38 and top==20
for r in csv.DictReader(ecosystems.open()):
 for k in ['trees_in_treeline_fade','trees_above_fade','invalid_roots','invalid_plants','grass_on_steep','sand_on_highland','water_label_conflicts']:c['ecosystem_'+k]+=int(r[k])
for k in ['snowline_violations','missing_snow_on_gentle_slopes','non_stone_peaks','grass_on_cliff','sand_on_highland','ecosystem_trees_above_fade','ecosystem_invalid_roots','ecosystem_invalid_plants','ecosystem_grass_on_steep','ecosystem_sand_on_highland','ecosystem_water_label_conflicts']:assert c[k]==0,(k,c[k])
output.write_text(json.dumps(c,indent=2)+'\n');print(dict(c))
