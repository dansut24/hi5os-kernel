#!/usr/bin/env python3
"""Acquire only pinned original GUI/codec binaries from the official release."""
import hashlib, pathlib, subprocess, tempfile, json
import pefile
root=pathlib.Path(__file__).resolve().parent.parent
out=root/"target/hi5-fixture"
out.mkdir(parents=True,exist_ok=True)
pins={
    "7zfm.exe":"a12710799d5cb9b3076b86dd0f92d4a503b7062bc3efac62b7f4cb8a74e05931",
    "7z.dll":"500db9eb17f3c21b729ba41dd02ecfe1823da05028c7e4ba129084cc74cc364e",
}
with tempfile.TemporaryDirectory() as d:
    p=pathlib.Path(d); installer=p/"7z2606-x64.exe"
    subprocess.run(["curl","-fsSL","--retry","3","--max-time","120","-o",str(installer),"https://github.com/ip7z/7zip/releases/download/26.06/7z2606-x64.exe"],check=True)
    assert hashlib.sha256(installer.read_bytes()).hexdigest()=="9b697f65ecc2d9b0152f5ee96d72dde522a7827690349d9307935dacf40f42ec"
    dest=p/"payload"
    subprocess.run(["7z","x","-y","-o"+str(dest),str(installer)],check=True,stdout=subprocess.DEVNULL)
    report={}
    for name,digest in pins.items():
        candidates=[v for v in dest.rglob("*") if v.is_file() and v.name.lower()==name]
        assert len(candidates)==1,name
        blob=candidates[0].read_bytes();assert hashlib.sha256(blob).hexdigest()==digest,name
        pe=pefile.PE(data=blob)
        assert pe.FILE_HEADER.Machine==0x8664
        (out/name).write_bytes(blob);(out/(name+".sha256")).write_bytes(bytes.fromhex(digest))
        report[name]={"sha256":digest,"image_bytes":pe.OPTIONAL_HEADER.SizeOfImage,"imports":sum(len(x.imports) for x in getattr(pe,"DIRECTORY_ENTRY_IMPORT",[])),"delay_imports":[{"dll":x.dll.decode("ascii"),"symbols":[str(i.ordinal) if i.import_by_ordinal else i.name.decode("ascii") for i in x.imports]} for x in getattr(pe,"DIRECTORY_ENTRY_DELAY_IMPORT",[])]}
    assert report["7zfm.exe"]["imports"]==292 and report["7z.dll"]["imports"]==89
    (out/"gui-package.json").write_text(json.dumps(report,indent=2)+"\n")
    print("Pinned original GUI and DLL:",json.dumps(report))
