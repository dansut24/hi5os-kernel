#!/usr/bin/env python3
"""Obtain exact official pinned archive, generate binary digest only; no redistribution."""
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
    print("Pinned x64 7za.exe SHA256:",digest.hex())
