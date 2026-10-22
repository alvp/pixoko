import sys, collections
import l2gen
from regions import rs_files, ROOT
from rslex import lex, ident_name
from vocab import KW
crate=sys.argv[1]
words=collections.defaultdict(set)
for p in rs_files():
    rel=str(p.relative_to(ROOT))
    if not rel.startswith(f"crates/{crate}/"): continue
    for t in lex(p.read_text(encoding='utf-8')):
        if t.kind=='ident':
            n=ident_name(t)
            if n in KW: continue
            for part in l2gen.words_of(n):
                for w in part:
                    if not w.isdigit() and len(w)>2: words[w.lower()].add(n)
ok=l2gen.english_ok(list(words))
bad=sorted(set(words)-ok)
print(len(bad)); print(' '.join(bad))
