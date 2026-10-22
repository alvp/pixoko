import sys, collections, re
import l2gen
from regions import rs_files, ROOT
from rslex import lex, ident_name
from vocab import KW
crate=sys.argv[1]
left=collections.Counter()
amb=l2gen.ambiguous()
for p in rs_files():
    rel=str(p.relative_to(ROOT))
    if not rel.startswith(f"crates/{crate}/"): continue
    for t in lex(p.read_text(encoding='utf-8')):
        if t.kind=='ident':
            n=ident_name(t)
            if n in KW: continue
            ws=[l2gen.fold(w.lower()) for part in l2gen.words_of(n) for w in part]
            if any(w in l2gen.GLOSS and l2gen.GLOSS[w]!=w and w not in amb for w in ws): left[n]+=1
print(len(left)); print(' '.join(sorted(left)[:80]))
