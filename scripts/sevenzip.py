#!/usr/bin/env python3
"""Pinned official 7-Zip x64 binary, read-only import comparison against pefile.
Experimental original PE startup probe in isolated guest; never redistributes the executable.
"""
import collections, hashlib, json, pathlib, subprocess, sys, tempfile, re, struct, zlib
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
    # Independent reference: pefile supplies section and relocation records.
    # Preserve the original header bytes just as the inert guest mapper does.
    size=pe.OPTIONAL_HEADER.SizeOfImage
    preferred=pe.OPTIONAL_HEADER.ImageBase
    base=preferred+0x10000000
    image=bytearray(size)
    headers=pe.OPTIONAL_HEADER.SizeOfHeaders
    image[:headers]=raw[:headers]
    for sec in pe.sections:
        a=sec.VirtualAddress;rp=sec.PointerToRawData;n=sec.SizeOfRawData
        if n:image[a:a+n]=raw[rp:rp+n]
    count=0
    for block in pe.DIRECTORY_ENTRY_BASERELOC:
        for e in block.entries:
            if e.type==0:continue
            assert e.type==10,"Unsupported reference relocation"
            old=struct.unpack_from("<Q",image,e.rva)[0]
            struct.pack_into("<Q",image,e.rva,(old+base-preferred)&0xffffffffffffffff)
            count+=1
    for k,v in {"SIZE":size,"BASE":base,"COUNT":count,"CRC":zlib.crc32(image)}.items():
        assert f"PE_REBASE_{k}=0x{v:016x}" in log,f"Rebase {k} mismatch: expected {v:#x}"
    report["rebase"]={"image_bytes":size,"base":hex(base),"dir64_count":count,"crc32":hex(zlib.crc32(image))}
    (ROOT/"dist"/"7zip-imports.json").write_text(json.dumps(report,indent=2)+"\n")
    print(f"PASS: real 7-Zip inert image at {base:#x}, {count} relocations, exact mapped-image CRC32 matched reference")
    assert "H5K: TRIAL_RAMFS_OK" in log and "H5K: TRIAL_FAULT_RECOVERED" in log and "H5K: TRIAL_TIMEOUT_RECOVERED" in log
    assert "H5K: TRIAL_ENTER_7ZIP" in log and "H5K: TRIAL_RETURNED" in log
    assert "H5K: TRIAL_WIN64_ENTRY_OK" in log and "H5K: APP_7ZIP_INSTALLED" in log and "H5K: APP_7ZIP_LOADED" in log
    assert "H5K: TRIAL_CPU_FEATURES_OK" in log and "H5K: TRIAL_CRITICAL_SECTION_OK" in log and "H5K: TRIAL_VERSION_INFO_OK" in log and "H5K: TRIAL_RESTRICTED_TOKEN_OK" in log
    report["portable_install"]="byte-exact disk and verified reload after reboot"
    report["execution"]="help-qualified" if "H5K: SEVENZIP_HELP_OK" in log else "startup attempted; help not qualified"
    report["trial_status"]=re.findall(r"TRIAL_RESULT=(0x[0-9a-f]+)",log)
    report["fault_rip"]=re.findall(r"TRIAL_FAULT_RIP=(0x[0-9a-f]+)",log)
    report["last_api"]=re.findall(r"TRIAL_LAST_API\|([^\r\n]+)",log)
    report["api_calls"]=re.findall(r"TRIAL_API_CALLS=(0x[0-9a-f]+)",log)
    if report["fault_rip"]:
        rip=int(report["fault_rip"][-1],16)
        if 0x100000000<=rip<0x100000000+size:
            addr=preferred+rip-0x100000000
            proc=subprocess.run(["objdump","-d","--start-address="+hex(max(preferred,addr-32)),"--stop-address="+hex(addr+32),str(exe)],capture_output=True,text=True,check=True)
            (ROOT/"dist"/"7zip-fault-disassembly.txt").write_text(proc.stdout)
            print(proc.stdout)
    report["unsupported"]=re.findall(r"TRIAL_UNSUPPORTED\|([^\r\n]+)",log)
    (ROOT/"dist"/"7zip-imports.json").write_text(json.dumps(report,indent=2)+"\n")
    print("7-Zip execution result:",report["execution"],report["trial_status"],report["unsupported"])
    assert report["execution"]=="help-qualified","Regression: genuine help no longer qualified"


