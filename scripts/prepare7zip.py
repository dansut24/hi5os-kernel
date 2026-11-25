#!/usr/bin/env python3
"""Obtain exact official pinned archive, retain unmodified standalone executable and upstream notices for test images."""
import hashlib,pathlib,subprocess,tempfile
root=pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory() as td:
    p=pathlib.Path(td);a=p/"extra.7z"
    subprocess.run(["curl","-fsSL","--retry","3","--max-time","120","-o",str(a),"https://github.com/ip7z/7zip/releases/download/26.06/7z2606-extra.7z"],check=True)
    assert hashlib.sha256(a.read_bytes()).hexdigest()=="396c180f12d2247c24b0b1fcd482c6fc2a2700f61211ffede1061dd971f5e104"
    subprocess.run(["7z","x","-y","-o"+str(p/"extract"),str(a)],check=True,stdout=subprocess.DEVNULL)
    import struct
    matches=[]
    for x in (p/"extract").rglob("*.exe"):
        if x.name.lower()!="7za.exe":continue
        b=x.read_bytes();pe=struct.unpack_from("<I",b,60)[0]
        if struct.unpack_from("<H",b,pe+4)[0]==0x8664:matches.append(b)
    assert len(matches)==1 and len(matches[0])==1328640
    out=root/"target"/"hi5-fixture";out.mkdir(parents=True,exist_ok=True)
    digest=hashlib.sha256(matches[0]).digest();(out/"7zip.sha256").write_bytes(digest)
    assert digest.hex()=="dd3757f31c97e33776f2590317d4fb12f2da075db3dbb1e3f94d0f2a01d84ffa"
    (out/"7za.exe").write_bytes(matches[0])
    notices=out/"7zip-notices";notices.mkdir(exist_ok=True)
    # Preserve all text/documentation files in the official Extra distribution.
    docs=[x for x in (p/"extract").rglob("*") if x.is_file() and x.suffix.lower() in (".txt",".md",".htm",".html")]
    assert any("license" in x.name.lower() for x in docs),"Missing upstream license"
    import shutil
    for x in docs:
        dst=notices/x.relative_to(p/"extract");dst.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(x,dst)
    (notices/"HI5OS-NOTICE.txt").write_text("Hi5OS uses the unmodified 7-Zip 26.06 x64 standalone program, copyright Igor Pavlov, under the GNU LGPL and other applicable terms in the accompanying upstream notices. Source code: https://www.7-zip.org/ and https://github.com/ip7z/7zip/tree/26.06 . Original binary distribution: https://github.com/ip7z/7zip/releases/tag/26.06 . The separately executed binary is unmodified; Hi5Compat is independent.\\n")
    # Include LGPL text and corresponding official source archive in distribution evidence.
    # GitHub tag archive is the exact upstream source for this release.
    source=notices/"7zip-26.06-source.tar.gz"
    subprocess.run(["curl","-fsSL","--retry","3","--max-time","120","-o",str(source),"https://github.com/ip7z/7zip/archive/refs/tags/26.06.tar.gz"],check=True)
    import tarfile
    with tarfile.open(source) as tf:
        candidates=[m for m in tf.getmembers() if m.isfile() and m.name.lower().endswith("/copying.txt")]
        if candidates:(notices/"COPYING.txt").write_bytes(tf.extractfile(candidates[0]).read())
    assert any(x.name.lower()=="copying.txt" for x in notices.rglob("*")),"Missing LGPL text"
    print("Pinned x64 7za.exe SHA256:",digest.hex())
