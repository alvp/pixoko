import sys, collections
import l2gen
from l2batch import load_ov, nw
crate=sys.argv[1]; start=int(sys.argv[2]); n=int(sys.argv[3])
uses=l2gen.collect(); ov=load_ov()
names=sorted(o for o in uses if l2gen.is_spanish(o) and o not in ov and any(c.startswith(f"crates/{crate}/") for c in uses[o]))
print(len(names),'pendientes de revisar (borrador por glosario)')
for o in names[start:start+n]:
    print(f"{o} -> {l2gen.translate(o)}")
