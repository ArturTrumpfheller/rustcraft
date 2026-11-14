#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compare shape/depth error to an executed reference; paired seed bootstrap.
Positive improvement means the new candidate has lower error. Uses unconditional
windows only; conditional reference/subject coordinates need not be paired.
"""
import argparse,json
from pathlib import Path
import numpy as np
p=argparse.ArgumentParser(description=__doc__)
for label in ['previous','reference','revised']:p.add_argument('--'+label,type=Path,required=True)
p.add_argument('--output',type=Path,required=True);a=p.parse_args()
rows={label:[r for line in getattr(a,label).read_text().splitlines() if (r:=json.loads(line))['stratum'].endswith('-unconditional')] for label in ['previous','reference','revised']}
seeds=sorted({r['seed'] for r in rows['reference']})
assert all({(r['id'],r['seed'],r['x'],r['z']) for r in v}=={(r['id'],r['seed'],r['x'],r['z']) for r in rows['reference']} for v in rows.values())
keys=['width_bins','headroom_bins','normalized_geology','normalized_cavity']
sums={label:{k:np.array([np.array([r[k] for r in v if r['seed']==s]).sum(axis=0) for s in seeds],float) for k in keys} for label,v in rows.items()}
def metrics(indices):
 totals={label:{k:val[indices].sum(axis=0) for k,val in data.items()} for label,data in sums.items()}
 result={}
 for key in ['width_bins','headroom_bins']:
  h={label:data[key]/data[key].sum() for label,data in totals.items()}
  result[key]={label:float(abs(h[label]-h['reference']).sum()/2) for label in ['previous','revised']}
 rates={label:100*data['normalized_cavity']/np.maximum(1,data['normalized_geology']) for label,data in totals.items()}
 result['normalized_depth_mean_absolute_error_percentage_points']={label:float(abs(rates[label]-rates['reference']).mean()) for label in ['previous','revised']}
 return result
estimate=metrics(np.arange(len(seeds)));rng=np.random.default_rng(392027);boots={k:[] for k in estimate}
for indices in rng.integers(0,len(seeds),(4096,len(seeds))):
 for key,v in metrics(indices).items():boots[key].append(v['previous']-v['revised'])
for key,v in estimate.items():v['error_reduction']=dict(estimate=v['previous']-v['revised'],low=float(np.percentile(boots[key],2.5)),high=float(np.percentile(boots[key],97.5)))
a.output.write_text(json.dumps(dict(metrics=estimate,windows=len(rows['reference']),seed_clusters=len(seeds),method='4096 paired seed-cluster resamples; RNG392027; histogram total variation; depth-bin equal-weight MAE',limits='Descriptive evaluation cohort, not held-out proof of Minecraft parity. Vertical domains differ; remapped sensitivity is separate.'),indent=2)+'\n')
