#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Linux wait4 process CPU/RSS around verified benchmark executables.
Usage: measure_biome_process.py core|stream <normal benchmark arguments>
Build first. Only benchmark children are observed; no compiler RSS is reported.
The benchmark's own raw stage/latency observations remain unchanged.
"""
import hashlib,json,os,subprocess,sys,time
from pathlib import Path

def main():
 if not hasattr(os,'wait4'):raise SystemExit('wait4 resource observations require Linux/Unix; use native platform tools on Windows')
 mode=sys.argv[1];args=sys.argv[2:];out=Path(args[args.index('--output')+1]).resolve();out.mkdir(parents=True,exist_ok=True)
 module=__import__('benchmark_biome_v2' if mode=='core' else 'benchmark_streaming')
 original=subprocess.run
 def run(command,*positional,**kw):
  if not isinstance(command,list) or not str(command[0]).endswith(('-bench','-lifecycle')):return original(command,*positional,**kw)
  if positional or kw.get('stdout')==subprocess.PIPE or kw.get('stderr')==subprocess.PIPE:raise ValueError('observed benchmark output must go to the ordinary file logs')
  check=kw.pop('check',False);start=time.monotonic();proc=subprocess.Popen(command,**kw)
  _,status,usage=os.wait4(proc.pid,0);proc.returncode=os.waitstatus_to_exitcode(status)
  record=dict(executable=Path(command[0]).name,sha256=hashlib.sha256(Path(command[0]).read_bytes()).hexdigest(),arguments=command[1:],returncode=proc.returncode,wall_seconds=time.monotonic()-start,user_seconds=usage.ru_utime,system_seconds=usage.ru_stime,max_rss_bytes=usage.ru_maxrss*1024,platform=sys.platform)
  with (out/'process-resources.jsonl').open('a') as f:f.write(json.dumps(record)+'\n')
  if check and proc.returncode:raise subprocess.CalledProcessError(proc.returncode,command)
  return subprocess.CompletedProcess(command,proc.returncode)
 subprocess.run=run
 sys.argv=[module.__file__,*args]
 try:module.main()
 finally:subprocess.run=original
 (out/'process-method.json').write_text(json.dumps(dict(method='Linux wait4 rusage, per verified executable invocation; all worker threads included',max_rss='Kernel high-water RSS, includes libraries/allocator; excludes GPU storage and other processes',source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()),indent=2)+'\n')
if __name__=='__main__':main()
