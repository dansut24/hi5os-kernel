from pathlib import Path
import sys, zlib
with Path(sys.argv[1]).open("xb") as f:
    f.write(b"HI5OS-DATA-V1-7D1F6C0284A9E3B5".ljust(512,b"\0"))
    f.truncate(16*1024*1024)
    if "--setup" in sys.argv[2:]:
        b=bytearray(512);b[:8]=b"H5SETUP1";b[508:]=zlib.crc32(b[:508]).to_bytes(4,"little");f.seek(14*512);f.write(b)
