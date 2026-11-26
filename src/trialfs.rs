//! Volatile, single-process private file namespace for the pinned Windows trial.
//! No access to host, real disk, saved downloads, other apps, devices or network shares.
//! Sixteen 256 KiB file slots, sixteen directories, 32 handles, aggregate 2.75 MiB quota.
const FC:usize=16;const DC:usize=16;const HC:usize=32;const CAP:usize=262144;
pub const QUOTA:usize=2883584;
pub const INVALID:u64=u64::MAX;
#[derive(Clone,Copy,PartialEq,Eq)]pub struct Path{bytes:[u8;96],len:usize}
impl Path{
    pub const fn empty()->Self{Self{bytes:[0;96],len:0}}
    pub fn parse(s:&[u8])->Option<Self>{
        let mut p=Self::empty();let mut start=0;
        if s.len()>=2&&s[1]==b':'{if !s[0].eq_ignore_ascii_case(&b'c'){return None;}start=2;}
        // Only C:\ or a relative path under private root. Reject UNC, .., ADS and device syntax.
        if s.get(start)==Some(&b'\\')||s.get(start)==Some(&b'/'){start+=1;}
        if s.get(start)==Some(&b'\\')||s.get(start)==Some(&b'/'){return None;}
        for part in s[start..].split(|c|*c==b'\\'||*c==b'/'){
            if part.is_empty()||part==b"."{continue;}
            if part==b".."||part.last()==Some(&b'.')||part.last()==Some(&b' ')||part.iter().any(|c|!(32..127).contains(c)||b":*?\"<>|".contains(c)){return None;}
            let base=part.split(|c|*c==b'.').next().unwrap_or(&[]);
            if [b"con".as_slice(),b"prn",b"aux",b"nul",b"com1",b"lpt1"].iter().any(|x|base.eq_ignore_ascii_case(x)){return None;}
            if p.len!=0{if p.len>=96{return None;}p.bytes[p.len]=b'\\';p.len+=1;}
            if p.len+part.len()>96{return None;}
            for c in part{p.bytes[p.len]=c.to_ascii_lowercase();p.len+=1;}
        }Some(p)
    }
    pub fn name(&self)->&[u8]{let n=self.bytes[..self.len].iter().rposition(|c|*c==b'\\').map_or(0,|i|i+1);&self.bytes[n..self.len]}
    pub fn bytes(&self)->&[u8]{&self.bytes[..self.len]}
    fn parent(self)->Self{let mut p=Self::empty();if let Some(n)=self.bytes[..self.len].iter().rposition(|c|*c==b'\\'){p.bytes[..n].copy_from_slice(&self.bytes[..n]);p.len=n;}p}
    fn child(self,parent:Self)->bool{if parent.len==0{return self.len!=0;}self.len>parent.len&&self.bytes[..parent.len]==parent.bytes[..parent.len]&&self.bytes[parent.len]==b'\\'}
}
#[derive(Clone,Copy)]struct File{path:Path,used:bool,len:usize,times:[u64;3],attrs:u32}
const FILE:File=File{path:Path::empty(),used:false,len:0,times:[0;3],attrs:128};
#[derive(Clone,Copy)]struct Dir{path:Path,used:bool}
const DIR:Dir=Dir{path:Path::empty(),used:false};
#[derive(Clone,Copy)]struct Handle{file:usize,pos:usize,read:bool,write:bool,used:bool,share:u32}
const HANDLE:Handle=Handle{file:0,pos:0,read:false,write:false,used:false,share:0};
#[derive(Clone,Copy)]pub struct Info{pub path:Path,pub size:u32,pub attrs:u32,pub times:[u64;3]}
pub const SNAP_CAP:usize=QUOTA+8192;
pub struct Fs{files:[File;FC],dirs:[Dir;DC],handles:[Handle;HC],data:[[u8;CAP];FC]}
impl Fs{
    pub const fn new()->Self{Self{files:[FILE;FC],dirs:[DIR;DC],handles:[HANDLE;HC],data:[[0;CAP];FC]}}
    pub fn reset(&mut self){self.files.fill(FILE);self.dirs.fill(DIR);self.handles.fill(HANDLE);for b in &mut self.data{b.fill(0);}self.dirs[0]=Dir{path:Path::empty(),used:true};}
    fn dir(&self,p:Path)->bool{self.dirs.iter().any(|d|d.used&&d.path==p)}
    fn file(&self,p:Path)->Option<usize>{self.files.iter().position(|f|f.used&&f.path==p)}
    pub fn mkdir(&mut self,p:Path)->Result<(),u32>{if self.dir(p)||self.file(p).is_some(){return Err(183);}if !self.dir(p.parent()){return Err(3);}let d=self.dirs.iter_mut().find(|d|!d.used).ok_or(112u32)?;*d=Dir{path:p,used:true};Ok(())}
    pub fn rmdir(&mut self,p:Path)->Result<(),u32>{if p.len==0{return Err(5);}let i=self.dirs.iter().position(|d|d.used&&d.path==p).ok_or(3u32)?;
        if self.dirs.iter().any(|d|d.used&&d.path.child(p))||self.files.iter().any(|f|f.used&&f.path.child(p)){return Err(145);}self.dirs[i]=DIR;Ok(())}
    pub fn delete(&mut self,p:Path)->Result<(),u32>{let i=self.file(p).ok_or(2u32)?;if self.handles.iter().any(|h|h.used&&h.file==i){return Err(32);}self.files[i]=FILE;self.data[i].fill(0);Ok(())}
    pub fn attrs(&self,p:Path)->Result<u32,u32>{if self.dir(p){Ok(16)}else if let Some(i)=self.file(p){Ok(self.files[i].attrs)}else{Err(2)}}
    pub fn open(&mut self,p:Path,access:u32,share:u32,mode:u32)->Result<(u64,u32),u32>{
        if p.len==0||self.dir(p){return Err(5);}if !self.dir(p.parent()){return Err(3);}
        if access&!0xc0000000!=0||share&!7!=0||!(1..=5).contains(&mode){return Err(87);}
        let read=access&0x80000000!=0;let write=access&0x40000000!=0;
        let h=self.handles.iter().position(|h|!h.used).ok_or(4u32)?;
        let exists=self.file(p);
        if mode==1&&exists.is_some(){return Err(80);}if matches!(mode,3|5)&&exists.is_none(){return Err(2);}
        if matches!(mode,2|5)&&!write{return Err(5);}
        if let Some(i)=exists{for other in self.handles.iter().filter(|h|h.used&&h.file==i){
            if read&&other.share&1==0||write&&other.share&2==0||other.read&&share&1==0||other.write&&share&2==0{return Err(32);}
        }}
        let i=if let Some(i)=exists{i}else{let i=self.files.iter().position(|f|!f.used).ok_or(112u32)?;
            self.files[i]=File{path:p,used:true,len:0,times:[stamp();3],attrs:128};self.data[i].fill(0);i};
        if matches!(mode,2|5){self.files[i].len=0;self.data[i].fill(0);}
        self.handles[h]=Handle{file:i,pos:0,read,write,used:true,share};
        Ok((0x100+h as u64,if exists.is_some()&&matches!(mode,2|4){183}else{0}))
    }
    fn index(&self,h:u64)->Result<usize,u32>{let i=h.checked_sub(0x100).ok_or(6u32)? as usize;if i>=HC||!self.handles[i].used{Err(6)}else{Ok(i)}}
    pub fn close(&mut self,h:u64)->Result<(),u32>{let i=self.index(h)?;self.handles[i]=HANDLE;Ok(())}
    pub fn size(&self,h:u64)->Result<u32,u32>{let i=self.index(h)?;Ok(self.files[self.handles[i].file].len as u32)}
    pub fn seek(&mut self,h:u64,off:i64,origin:u32)->Result<u32,u32>{let i=self.index(h)?;let a=self.handles[i];let b=match origin{0=>0,1=>a.pos as i64,2=>self.files[a.file].len as i64,_=>return Err(87)};
        let pos=b.checked_add(off).ok_or(131u32)?;if !(0..=CAP as i64).contains(&pos){return Err(131);}self.handles[i].pos=pos as usize;Ok(pos as u32)}
    pub fn truncate(&mut self,h:u64)->Result<(),u32>{let i=self.index(h)?;let a=self.handles[i];if !a.write{return Err(5);}if a.pos.saturating_sub(self.files[a.file].len)>self.free_bytes()as usize{return Err(112);}let f=&mut self.files[a.file];
        if a.pos>f.len{self.data[a.file][f.len..a.pos].fill(0);}else{self.data[a.file][a.pos..f.len].fill(0);}f.len=a.pos;Ok(())}
    pub fn read(&mut self,h:u64,out:&mut[u8])->Result<usize,u32>{let i=self.index(h)?;let a=self.handles[i];if !a.read{return Err(5);}let n=out.len().min(self.files[a.file].len.saturating_sub(a.pos));
        if n!=0{out[..n].copy_from_slice(&self.data[a.file][a.pos..a.pos+n]);self.handles[i].pos+=n;}Ok(n)}
    pub fn write(&mut self,h:u64,src:&[u8])->Result<usize,u32>{let i=self.index(h)?;let a=self.handles[i];if !a.write{return Err(5);}let end=a.pos.checked_add(src.len()).ok_or(112u32)?;
        if end>CAP||end.saturating_sub(self.files[a.file].len)>self.free_bytes()as usize{return Err(112);}let f=&mut self.files[a.file];if a.pos>f.len{self.data[a.file][f.len..a.pos].fill(0);}self.data[a.file][a.pos..end].copy_from_slice(src);f.len=f.len.max(end);self.handles[i].pos=end;Ok(src.len())}
    pub fn close_all(&mut self){self.handles.fill(HANDLE);}
    pub fn info(&self,p:Path)->Option<Info>{if let Some(i)=self.file(p){let f=self.files[i];Some(Info{path:p,size:f.len as u32,attrs:f.attrs,times:f.times})}else if self.dir(p){Some(Info{path:p,size:0,attrs:16,times:[stamp();3]})}else{None}}
    pub fn handle_info(&self,h:u64)->Result<Info,u32>{let i=self.index(h)?;self.info(self.files[self.handles[i].file].path).ok_or(6)}
    pub fn set_times(&mut self,h:u64,t:[Option<u64>;3])->Result<(),u32>{let i=self.index(h)?;let a=self.handles[i];if !a.write{return Err(5);}for (j,v) in t.iter().enumerate(){if let Some(v)=v{self.files[a.file].times[j]=*v;}}Ok(())}
    pub fn set_attrs(&mut self,p:Path,attrs:u32)->Result<(),u32>{let i=self.file(p).ok_or(2u32)?;if attrs&!0x27!=0&&attrs!=128{return Err(87);}self.files[i].attrs=if attrs==0{128}else{attrs};Ok(())}
    pub fn rename(&mut self,src:Path,dst:Path)->Result<(),u32>{let i=self.file(src).ok_or(2u32)?;if self.info(dst).is_some(){return Err(183);}if !self.dir(dst.parent()){return Err(3);}if self.handles.iter().any(|h|h.used&&h.file==i){return Err(32);}self.files[i].path=dst;Ok(())}
    pub fn entry(&self,i:usize)->Option<Info>{if i<FC{let f=self.files[i];if f.used{return self.info(f.path);}}else if i<FC+DC{let d=self.dirs[i-FC];if d.used&&d.path.len!=0{return self.info(d.path);}}None}
    pub fn matching(&self,pattern:&[u8],cursor:&mut usize)->Option<Info>{
        let cut=pattern.iter().rposition(|c|*c==b'\\'||*c==b'/');
        let (prefix,pat)=if let Some(i)=cut{(&pattern[..i],&pattern[i+1..])}else if pattern.len()>=2&&pattern[1]==b':'{(&pattern[..2],&pattern[2..])}else{(&b""[..],pattern)};
        let parent=Path::parse(prefix)?;
        if pat.is_empty()||pat.iter().any(|c|!(32..127).contains(c)||b":\"<>|".contains(c)){return None;}
        while *cursor<FC+DC{let i=*cursor;*cursor+=1;if let Some(e)=self.entry(i){if e.path.parent()==parent&&wildcard(pat,e.path.name()){return Some(e);}}}None
    }
    pub fn contents(&self,p:Path)->Option<&[u8]>{let i=self.file(p)?;Some(&self.data[i][..self.files[i].len])}
    pub fn put(&mut self,p:Path,b:&[u8])->bool{if b.len()>CAP||b.len().saturating_sub(self.file(p).map_or(0,|i|self.files[i].len))>self.free_bytes()as usize{return false;}let Ok((h,_))=self.open(p,0xc0000000,0,2)else{return false;};let ok=self.write(h,b)==Ok(b.len());let _=self.close(h);ok}
    pub fn free_bytes(&self)->u64{QUOTA.saturating_sub(self.files.iter().filter(|f|f.used).map(|f|f.len).sum::<usize>())as u64}
    // Portable bounded snapshot: magic, count, per-record path length, flags, size, times, bytes.
    // Only the private workspace is serialized; real OS notes and downloads never become writable.
    pub fn pack(&self,out:&mut[u8])->Option<usize>{
        if out.len()<12{return None;}out[..8].copy_from_slice(b"H5WFS001");let mut at=12;let mut count=0u32;
        // Directories first, ordered by depth, then files.
        for depth in 0..96{for d in self.dirs.iter().filter(|d|d.used&&d.path.len!=0&&d.path.bytes().iter().filter(|c|**c==b'\\').count()==depth){
            let e=self.info(d.path)?;at=record(out,at,e,&[])?;count+=1;}}
        for f in self.files.iter().filter(|f|f.used){let e=self.info(f.path)?;at=record(out,at,e,self.contents(f.path)?)?;count+=1;}
        out[8..12].copy_from_slice(&count.to_le_bytes());Some(at)
    }
    pub fn unpack(&mut self,b:&[u8])->bool{
        self.reset();let ok=self.unpack_inner(b);if !ok{self.reset();}ok
    }
    fn unpack_inner(&mut self,b:&[u8])->bool{
        if b.len()<12||&b[..8]!=b"H5WFS001"{return false;}let count=u32::from_le_bytes(b[8..12].try_into().unwrap())as usize;if count>FC+DC-1{return false;}let mut at=12usize;
        for _ in 0..count{
            let Some(h)=b.get(at..at+36)else{return false;};let n=u32::from_le_bytes(h[..4].try_into().unwrap())as usize;let attr=u32::from_le_bytes(h[4..8].try_into().unwrap());let len=u32::from_le_bytes(h[8..12].try_into().unwrap())as usize;
            if n==0||n>96||len>CAP||len>self.free_bytes()as usize{return false;}let Some(name)=b.get(at+36..at+36+n)else{return false;};let Some(p)=Path::parse(name)else{return false;};if p.bytes()!=name||self.info(p).is_some(){return false;}
            let Some(data)=b.get(at+36+n..at+36+n+len)else{return false;};
            if attr==16{if len!=0||self.mkdir(p).is_err(){return false;}}else{if !self.put(p,data)||self.set_attrs(p,attr).is_err(){return false;}let i=self.file(p).unwrap();for j in 0..3{self.files[i].times[j]=u64::from_le_bytes(h[12+j*8..20+j*8].try_into().unwrap());}}
            at+=36+n+len;
        }at==b.len()
    }
    pub fn test(&mut self)->bool{
        self.reset();let Some(dir)=Path::parse(b"C:\\lab")else{return false;};let Some(file)=Path::parse(b"lab\\Test.txt")else{return false;};
        if Path::parse(b"../secret").is_some()||Path::parse(b"\\\\server\\share").is_some()||Path::parse(b"X:\\bad").is_some()||self.mkdir(dir).is_err(){return false;}
        let Ok((h,_))=self.open(file,0xc0000000,0,2)else{return false;};
        if self.write(h,b"Hi5 file")!=Ok(8)||self.size(h)!=Ok(8)||self.open(file,0x80000000,1,3)!=Err(32)||self.seek(h,0,0)!=Ok(0){return false;}
        let mut out=[0u8;8];if self.read(h,&mut out)!=Ok(8)||&out!=b"Hi5 file"||self.rmdir(dir)!=Err(145)||self.delete(file)!=Err(32){return false;}
        if self.seek(h,3,0)!=Ok(3)||self.truncate(h).is_err()||self.size(h)!=Ok(3)||self.close(h).is_err()||self.delete(file).is_err()||self.rmdir(dir).is_err(){return false;}
        self.reset();true
    }
}

fn stamp()->u64{crate::security::now().unwrap_or(0).saturating_add(11644473600).saturating_mul(10000000)}
fn record(out:&mut[u8],at:usize,e:Info,data:&[u8])->Option<usize>{
    let n=e.path.len;let end=at.checked_add(36+n+data.len())?;let h=out.get_mut(at..end)?;
    h[..4].copy_from_slice(&(n as u32).to_le_bytes());h[4..8].copy_from_slice(&e.attrs.to_le_bytes());h[8..12].copy_from_slice(&(data.len()as u32).to_le_bytes());for j in 0..3{h[12+j*8..20+j*8].copy_from_slice(&e.times[j].to_le_bytes());}
    h[36..36+n].copy_from_slice(e.path.bytes());h[36+n..].copy_from_slice(data);Some(end)
}
fn wildcard(pat:&[u8],name:&[u8])->bool{
    if pat==b"*.*"{return true;}let(mut p,mut n,mut star,mut back)=(0usize,0usize,None,0usize);
    while n<name.len(){if p<pat.len()&&(pat[p]==b'?'||pat[p].eq_ignore_ascii_case(&name[n])){p+=1;n+=1;}else if p<pat.len()&&pat[p]==b'*'{star=Some(p);p+=1;back=n;}else if let Some(s)=star{back+=1;n=back;p=s+1;}else{return false;}}
    while p<pat.len()&&pat[p]==b'*'{p+=1;}p==pat.len()
}
