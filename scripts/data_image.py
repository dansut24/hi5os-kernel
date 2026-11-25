from pathlib import Path
import sys
with Path(sys.argv[1]).open("xb") as f:
    f.write(b"HI5OS-DATA-V1-7D1F6C0284A9E3B5".ljust(512,b"\0"))
    f.truncate(16*1024*1024)
