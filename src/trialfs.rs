//! Volatile, single-process private file namespace for the pinned Windows trial.
//! No access to host, real disk, saved downloads, other apps, devices or network shares.
//! Eight 64 KiB file slots, eight directory slots (including root), sixteen handles.
const FC:usize=8;const DC:usize=8;const HC:usize=16;const CAP:usize=65536;
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
    fn parent(self)->Self{let mut p=Self::empty();if let Some(n)=self.bytes[..self.len].iter().rposition(|c|*c==b'\\'){p.bytes[..n].copy_from_slice(&self.bytes[..n]);p.len=n;}p}
    fn child(self,parent:Self)->bool{if parent.len==0{return self.len!=0;}self.len>parent.len&&self.bytes[..parent.len]==parent.bytes[..parent.len]&&self.bytes[parent.len]==b'\\'}
}
#[derive(Clone,Copy)]struct File{path:Path,used:bool,len:usize}
const FILE:File=File{path:Path::empty(),used:false,len:0};
#[derive(Clone,Copy)]struct Dir{path:Path,used:bool}
const DIR:Dir=Dir{path:Path::empty(),used:false};
#[derive(Clone,Copy)]struct Handle{file:usize,pos:usize,read:bool,write:bool,used:bool,share:u32}
const HANDLE:Handle=Handle{file:0,pos:0,read:false,write:false,used:false,share:0};
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
    pub fn attrs(&self,p:Path)->Result<u32,u32>{if self.dir(p){Ok(16)}else if self.file(p).is_some(){Ok(128)}else{Err(2)}}
    pub fn open(&mut self,p:Path,access:u32,share:u32,mode:u32)->Result<(u64,u32),u32>{
        if p.len==0||self.dir(p){return Err(5);}if !self.dir(p.parent()){return Err(3);}
        if access&!0xc0000000!=0||share&!3!=0||!(1..=5).contains(&mode){return Err(87);}
        let read=access&0x80000000!=0;let write=access&0x40000000!=0;
        let h=self.handles.iter().position(|h|!h.used).ok_or(4u32)?;
        let exists=self.file(p);
        if mode==1&&exists.is_some(){return Err(80);}if matches!(mode,3|5)&&exists.is_none(){return Err(2);}
        if matches!(mode,2|5)&&!write{return Err(5);}
        if let Some(i)=exists{for other in self.handles.iter().filter(|h|h.used&&h.file==i){
            if read&&other.share&1==0||write&&other.share&2==0||other.read&&share&1==0||other.write&&share&2==0{return Err(32);}
        }}
        let i=if let Some(i)=exists{i}else{let i=self.files.iter().position(|f|!f.used).ok_or(112u32)?;
            self.files[i]=File{path:p,used:true,len:0};self.data[i].fill(0);i};
        if matches!(mode,2|5){self.files[i].len=0;self.data[i].fill(0);}
        self.handles[h]=Handle{file:i,pos:0,read,write,used:true,share};
        Ok((0x100+h as u64,if exists.is_some()&&matches!(mode,2|4){183}else{0}))
    }
    fn index(&self,h:u64)->Result<usize,u32>{let i=h.checked_sub(0x100).ok_or(6u32)? as usize;if i>=HC||!self.handles[i].used{Err(6)}else{Ok(i)}}
    pub fn close(&mut self,h:u64)->Result<(),u32>{let i=self.index(h)?;self.handles[i]=HANDLE;Ok(())}
    pub fn size(&self,h:u64)->Result<u32,u32>{let i=self.index(h)?;Ok(self.files[self.handles[i].file].len as u32)}
    pub fn seek(&mut self,h:u64,off:i64,origin:u32)->Result<u32,u32>{let i=self.index(h)?;let a=self.handles[i];let b=match origin{0=>0,1=>a.pos as i64,2=>self.files[a.file].len as i64,_=>return Err(87)};
        let pos=b.checked_add(off).ok_or(131u32)?;if !(0..=CAP as i64).contains(&pos){return Err(131);}self.handles[i].pos=pos as usize;Ok(pos as u32)}
    pub fn truncate(&mut self,h:u64)->Result<(),u32>{let i=self.index(h)?;let a=self.handles[i];if !a.write{return Err(5);}let f=&mut self.files[a.file];
        if a.pos>f.len{self.data[a.file][f.len..a.pos].fill(0);}else{self.data[a.file][a.pos..f.len].fill(0);}f.len=a.pos;Ok(())}
    pub fn read(&mut self,h:u64,out:&mut[u8])->Result<usize,u32>{let i=self.index(h)?;let a=self.handles[i];if !a.read{return Err(5);}let n=out.len().min(self.files[a.file].len.saturating_sub(a.pos));
        if n!=0{out[..n].copy_from_slice(&self.data[a.file][a.pos..a.pos+n]);self.handles[i].pos+=n;}Ok(n)}
    pub fn write(&mut self,h:u64,src:&[u8])->Result<usize,u32>{let i=self.index(h)?;let a=self.handles[i];if !a.write{return Err(5);}let end=a.pos.checked_add(src.len()).ok_or(112u32)?;
        if end>CAP{return Err(112);}let f=&mut self.files[a.file];if a.pos>f.len{self.data[a.file][f.len..a.pos].fill(0);}self.data[a.file][a.pos..end].copy_from_slice(src);f.len=f.len.max(end);self.handles[i].pos=end;Ok(src.len())}
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
