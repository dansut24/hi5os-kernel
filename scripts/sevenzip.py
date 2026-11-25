#!/usr/bin/env python3
"""Pinned official 7-Zip x64 binary, read-only import comparison against pefile.
Never executes or republishes the third-party executable. CI isolated guest HTTP only.
"""
import collections, hashlib, json, pathlib, subprocess, sys, tempfile, re
import pefile
from smoke import ROOT
VERSION="26.06"
URL="https://github.com/ip7z/7zip/releases/download/26.06/7z2606-extra.7z"
SHA="396c180f12d2247c24b0b1fcd482c6fc2a2700f61211ffede1061dd971f5e104"
with tempfile.TemporaryDirectory(prefix="hi5os-7zip-") as tmp:
    t=pathlib.Path(tmp);archive=t/"extra.7z"
    subprocess.run(["curl","--fail","--location","--retry","3","--max-time","120","--output",str(archive),URL],check=True)
    assert hashlib.sha256(archive.read_bytes()).hexdigest()==SHA,"Official release digest mismatch"
    subprocess.run(["7z","x","-y","-o"+str(t/"extract"),str(archive)],check=True,stdout=subprocess.DEVNULL)
    candidates=[]
    for p in (t/"extract").rglob("*.exe"):
        if p.name.lower()!="7za.exe":continue
        pe=pefile.PE(str(p),fast_load=True)
        if pe.FILE_HEADER.Machine==0x8664:candidates.append(p)
        pe.close()
    assert len(candidates)==1,f"Expected one x64 7za.exe: {candidates}"
    exe=candidates[0];raw=exe.read_bytes()
    pe=pefile.PE(data=raw)
    imports=[]
    for entry in pe.DIRECTORY_ENTRY_IMPORT:
        dll=entry.dll.decode("ascii")
        for symbol in entry.imports:
            # pefile can annotate ordinal imports with known names. Compare the actual thunk kind.
            key="#"+str(symbol.ordinal) if symbol.import_by_ordinal else symbol.name.decode("ascii")
            imports.append((dll.lower(),key))
    assert imports,"7za.exe has no imports"
    assert {s for d,s in imports if d=="oleaut32.dll"}=={"#2","#4","#6","#7","#9","#10"},"Pinned ordinal inventory changed"
    print("Static dependency counts:",dict(collections.Counter(d for d,s in imports)),flush=True)
    report={"version":VERSION,"source":URL,"archive_sha256":SHA,"exe_sha256":hashlib.sha256(raw).hexdigest(),"bytes":len(raw),"machine":"AMD64","imports":[{"dll":d,"symbol":s} for d,s in imports],"delay_imports":bool(getattr(pe,"DIRECTORY_ENTRY_DELAY_IMPORT",[])),"relocations":bool(getattr(pe,"DIRECTORY_ENTRY_BASERELOC",[])),"execution":"not attempted"}
    (ROOT/"dist"/"7zip-imports.json").write_text(json.dumps(report,indent=2)+"\n")
    print(f"7-Zip {VERSION}: {len(raw)} bytes, {len(set(d for d,s in imports))} DLLs, {len(imports)} static imports",flush=True)
    subprocess.run([sys.executable,str(ROOT/"scripts/network.py"),"sevenzip",str(exe)],check=True)
    log=(ROOT/"dist"/"network-sevenzip.log").read_text(errors="replace")
    guest=[]
    for d,s in re.findall(r"H5K: PE_DEP\|([^|\r\n]+)\|([^\r\n]+)",log):
        guest.append((d.lower(),"#"+str(int(s[1:],16)) if s.startswith("#0x") else s))
    assert len(guest)==len(imports)==145,"Pinned 7-Zip import count changed"
    assert len(set(d for d,s in guest))==5,"Pinned DLL count changed"
    assert collections.Counter(guest)==collections.Counter(imports),f"Import mismatch: missing {collections.Counter(imports)-collections.Counter(guest)}, extra {collections.Counter(guest)-collections.Counter(imports)}"
    print(f"PASS: native PE inventory exactly matches host pefile for all {len(imports)} imports")
