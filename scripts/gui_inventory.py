#!/usr/bin/env python3
"""Inspect official installer and its real GUI payload; no Windows binary is executed or redistributed."""
import hashlib,json,pathlib,subprocess,tempfile,collections,re,zlib,sys
import olefile
import pefile
from smoke import ROOT
url="https://github.com/ip7z/7zip/releases/download/26.06/"
pins={"7z2606-x64.exe":"9b697f65ecc2d9b0152f5ee96d72dde522a7827690349d9307935dacf40f42ec","7z2606-x64.msi":"6dc822569e777d9a3ecd5ffcbc22b55600af87e6c5d9810a64e8566cf5a4af9f"}
def inspect(path):
    p=pefile.PE(str(path));items=[]
    for desc in getattr(p,"DIRECTORY_ENTRY_IMPORT",[]):
        for symbol in desc.imports:items.append({"dll":desc.dll.decode("ascii"),"name":("#"+str(symbol.ordinal) if symbol.import_by_ordinal else symbol.name.decode("ascii"))})
    return {"filename":path.name,"sha256":hashlib.sha256(path.read_bytes()).hexdigest(),"machine":hex(p.FILE_HEADER.Machine),"image_bytes":p.OPTIONAL_HEADER.SizeOfImage,"subsystem":p.OPTIONAL_HEADER.Subsystem,"imports":items,"dll_counts":dict(collections.Counter(v["dll"].lower() for v in items)),"exports":[e.name.decode("ascii",errors="replace") if e.name else "#"+str(e.ordinal) for e in getattr(getattr(p,"DIRECTORY_ENTRY_EXPORT",None),"symbols",[])],"delay_imports":bool(getattr(p,"DIRECTORY_ENTRY_DELAY_IMPORT",[])),"execution":"not attempted"}
with tempfile.TemporaryDirectory() as td:
    d=pathlib.Path(td);report={"version":"26.06","status":"inventory only; GUI/installer not qualified"}
    for name,sha in pins.items():
        p=d/name;subprocess.run(["curl","-fsSL","--retry","3","--max-time","120","-o",str(p),url+name],check=True)
        assert hashlib.sha256(p.read_bytes()).hexdigest()==sha
    exe=d/"7z2606-x64.exe";msi=d/"7z2606-x64.msi"
    report["exe_installer"]=inspect(exe)
    dest=d/"payload";subprocess.run(["7z","x","-y","-o"+str(dest),str(exe)],check=True,stdout=subprocess.DEVNULL)
    report["payload"]=[inspect(p) for p in dest.rglob("*") if p.is_file() and p.suffix.lower() in (".exe",".dll")]
    assert any(p["filename"].lower()=="7zfm.exe" for p in report["payload"]),"7-Zip File Manager missing"
    assert any(p["filename"].lower()=="7z.dll" for p in report["payload"]),"7-Zip codec DLL missing"
    subprocess.run([sys.executable,str(ROOT/"scripts/network.py"),"msi",str(msi)],check=True)
    log=(ROOT/"dist"/"network-msi.log").read_text(errors="replace")
    guest=collections.Counter((name,int(size,16),int(crc,16)) for name,size,crc in re.findall(r"H5K: MSI_STREAM\|([0-9a-f]+)\r?\nH5K: MSI_STREAM_SIZE=(0x[0-9a-f]+)\r?\nH5K: MSI_STREAM_CRC=(0x[0-9a-f]+)",log))
    with olefile.OleFileIO(str(msi)) as ole:
        host=collections.Counter()
        for name in ole.listdir(streams=True,storages=False):
            raw=ole.openstream(name).read();key="".join(f"{ord(c):04x}" for c in name[-1]);host[(key,len(raw),zlib.crc32(raw))]+=1
    assert guest==host,f"Native MSI stream mismatch: missing {host-guest}, extra {guest-host}"
    report["native_msi_stream_crc_comparison"]={"passed":True,"streams":sum(host.values())}
    tables=subprocess.run(["msiinfo","tables",str(msi)],capture_output=True,text=True,check=True).stdout.splitlines()
    report["msi"]={"sha256":pins[msi.name],"tables":tables,"execution":"not attempted","row_counts":{}}
    for table in ["Property","Directory","Component","File","Feature","FeatureComponents","Registry","Shortcut","InstallExecuteSequence","CustomAction","Media"]:
        if table in tables:
            raw=subprocess.run(["msiinfo","export",str(msi),table],capture_output=True,text=True,check=True).stdout
            report["msi"]["row_counts"][table]=max(0,len(raw.splitlines())-3)
            (ROOT/"dist"/("7zip-msi-"+table+".idt")).write_text(raw)
    (ROOT/"dist"/"7zip-gui-installer-imports.json").write_text(json.dumps(report,indent=2)+"\n")
    print("Official installer and original GUI inventory:",[(p["filename"],len(p["imports"])) for p in report["payload"]],report["msi"]["row_counts"])
