#!/usr/bin/env python3
"""Build pinned BearSSL as freestanding Win64 COFF and compile distribution CA trust anchors."""
import pathlib,subprocess,hashlib,tarfile,os,sys
from cryptography import x509
from cryptography.hazmat.primitives.asymmetric import rsa,ec
from cryptography.hazmat.primitives.serialization import Encoding,PublicFormat
ROOT=pathlib.Path(__file__).resolve().parents[1]
OUT=ROOT/"target/hi5-tls"
TEST=len(sys.argv)==2
ROOT_FILES=[pathlib.Path(sys.argv[1])] if TEST else sorted(pathlib.Path("/usr/share/ca-certificates/mozilla").glob("*.crt"))
OUT.mkdir(parents=True,exist_ok=True)
ARCHIVE=OUT/"bearssl-0.6.tar.gz"
if not ARCHIVE.exists():subprocess.run(["curl","--fail","--location","--retry","2","https://bearssl.org/bearssl-0.6.tar.gz","-o",str(ARCHIVE)],check=True)
assert hashlib.sha256(ARCHIVE.read_bytes()).hexdigest()=="6705bba1714961b41a728dfc5debbe348d2966c117649392f8c8139efc83ff14","BearSSL digest mismatch"
SRC=OUT/"bearssl-0.6"
if not SRC.exists():
    with tarfile.open(ARCHIVE) as f:f.extractall(OUT,filter="data")
# BearSSL 0.6 enters the intrinsic-header block before its per-algorithm
# availability guards. Disable that block for this freestanding target.
inner=SRC/"src/inner.h"
source=inner.read_text()
import re
guard=r"(?m)^#if BR_ENABLE_INTRINSICS && .*"
if "HI5_PORTABLE_INTRINSICS_DISABLED" not in source:
    source,n=re.subn(guard,"#if 0 /* HI5_PORTABLE_INTRINSICS_DISABLED */",source,count=1)
    assert n==1,"BearSSL intrinsic guard changed"
    inner.write_text(source)
def barray(name,data):
    return f"static const unsigned char {name}[]={{"+",".join(str(b) for b in data)+"};\n"
def number(n):return n.to_bytes((n.bit_length()+7)//8,"big")
lines=[];entries=[]
for path in ROOT_FILES:
    cert=x509.load_pem_x509_certificate(path.read_bytes())
    try:
        if not cert.extensions.get_extension_for_class(x509.BasicConstraints).value.ca:continue
    except x509.ExtensionNotFound:continue
    key=cert.public_key();i=len(entries);dn=f"dn{i}";k=f"k{i}";e=f"e{i}"
    if isinstance(key,rsa.RSAPublicKey):
        if key.key_size<2048:continue
        nums=key.public_numbers();lines.extend([barray(dn,cert.subject.public_bytes()),barray(k,number(nums.n)),barray(e,number(nums.e))])
        desc=f"{{BR_KEYTYPE_RSA,{{.rsa={{(unsigned char*){k},sizeof {k},(unsigned char*){e},sizeof {e}}}}}}}"
    elif isinstance(key,ec.EllipticCurvePublicKey):
        curve={"secp256r1":"BR_EC_secp256r1","secp384r1":"BR_EC_secp384r1","secp521r1":"BR_EC_secp521r1"}.get(key.curve.name)
        if not curve:continue
        lines.extend([barray(dn,cert.subject.public_bytes()),barray(k,key.public_bytes(Encoding.X962,PublicFormat.UncompressedPoint))])
        desc=f"{{BR_KEYTYPE_EC,{{.ec={{{curve},(unsigned char*){k},sizeof {k}}}}}}}"
    else:continue
    entries.append(f"{{{{(unsigned char*){dn},sizeof {dn}}},BR_X509_TA_CA,{desc}}}")
assert len(entries)==1 if TEST else len(entries)>50,"No CA roots"
DEST=OUT/"test" if TEST else OUT
DEST.mkdir(exist_ok=True)
(DEST/"roots.h").write_text("".join(lines)+"\nstatic const br_x509_trust_anchor ROOTS[]={\n"+",\n".join(entries)+"\n};\n#define ROOT_COUNT (sizeof ROOTS/sizeof ROOTS[0])\n")
flags=["clang","--target=x86_64-pc-windows-msvc","-ffreestanding","-fno-builtin","-fno-stack-protector","-mno-stack-arg-probe","-O2","-DBR_USE_UNIX_TIME=0","-DBR_USE_WIN32_TIME=0","-DBR_USE_WIN32_RAND=0","-DBR_USE_URANDOM=0","-DBR_USE_GETENTROPY=0","-DBR_RDRAND=0","-DBR_AES_X86NI=0","-DBR_SSE2=0","-DBR_POWER8=0","-DBR_INT128=0","-DBR_UMUL128=0","-I"+str(ROOT/"native/tls/include"),"-I"+str(SRC/"inc"),"-I"+str(SRC/"src"),"-I"+str(DEST)]
sources=sorted((SRC/"src").rglob("*.c"))+[ROOT/"native/tls/hi5_tls.c"]
objects=[]
for i,src in enumerate(sources):
    obj=(DEST if i==len(sources)-1 else OUT)/f"{i}.obj"
    if not TEST or i==len(sources)-1 or not obj.exists():subprocess.run(flags+["-c",str(src),"-o",str(obj)],check=True)
    objects.append(str(obj))
lib=DEST/"hi5tls.lib";lib.unlink(missing_ok=True);subprocess.run(["llvm-ar","rcs",str(lib),*objects],check=True)
print(f"Native TLS: {len(entries)} CA trust anchors, {len(objects)} objects")
