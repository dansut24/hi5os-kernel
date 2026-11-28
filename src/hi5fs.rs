//! Hi5FS v1: bounded copy-on-write native volume inside the unused data-disk gap.
//! No legacy sector is ever written. One authenticated local workspace, not OS-level ACL isolation.
use alloc::{vec,vec::Vec,string::String};
pub const FIRST:u32=6466;
pub const END:u32=8192;
const META:u32=16;
const BANK:u32=META+1;
const DATA:u32=FIRST+2*BANK;
pub const BLOCKS:usize=(END-DATA)as usize;
pub const MAX_FILE:usize=256*1024;
const COUNT:usize=64;
const RECORD:usize=128;
const MAGIC:&[u8;8]=b"H5FS0001";
pub const ROOT:u32=1;
pub const HOME:u32=5;
pub const DOCUMENTS:u32=7;
type R<T>=Result<T,&'static str>;
pub trait Device{fn read(&mut self,lba:u32,b:&mut[u8;512])->bool;fn write(&mut self,lba:u32,b:&[u8;512])->bool;fn flush(&mut self)->bool;}
fn crc(b:&[u8])->u32{let mut v=0xffffffffu32;for x in b{v^=*x as u32;for _ in 0..8{v=(v>>1)^if v&1!=0{0xedb88320}else{0};}}!v}
fn get(b:&[u8],i:usize)->u32{u32::from_le_bytes(b[i..i+4].try_into().unwrap())}
fn put(b:&mut[u8],i:usize,v:u32){b[i..i+4].copy_from_slice(&v.to_le_bytes());}
fn get64(b:&[u8],i:usize)->u64{u64::from_le_bytes(b[i..i+8].try_into().unwrap())}
fn put64(b:&mut[u8],i:usize,v:u64){b[i..i+8].copy_from_slice(&v.to_le_bytes());}
fn name_ok(n:&str)->bool{!n.is_empty()&&n.len()<=64&&n!="."&&n!=".."&&n.trim()==n&&n.bytes().all(|b|(32..127).contains(&b)&&!b"\\/:*?\"<>|".contains(&b))}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Entry{pub id:u32,pub parent:u32,pub dir:bool,pub owner:u8,pub len:u32,pub created:u64,pub modified:u64,start:u32,sum:u32,name:[u8;64],n:u8}
impl Entry{
    fn new(id:u32,parent:u32,dir:bool,owner:u8,name:&str,now:u64)->Self{let mut b=[0;64];b[..name.len()].copy_from_slice(name.as_bytes());Self{id,parent,dir,owner,len:0,created:now,modified:now,start:0,sum:crc(&[]),name:b,n:name.len()as u8}}
    pub fn name(&self)->&str{core::str::from_utf8(&self.name[..self.n as usize]).unwrap_or("")}
    fn blocks(&self)->usize{(self.len as usize+511)/512}
    fn encode(&self,b:&mut[u8]){put(b,0,self.id);put(b,4,self.parent);b[8]=if self.dir{1}else{2};b[9]=self.owner;b[10]=self.n;put(b,12,self.len);put(b,16,self.start);put(b,20,self.sum);put64(b,24,self.created);put64(b,32,self.modified);b[40..104].copy_from_slice(&self.name);}
    fn decode(b:&[u8])->R<Self>{let id=get(b,0);let parent=get(b,4);let kind=b[8];let owner=b[9];let n=b[10];if id==0||!matches!(kind,1|2)||owner>1||n>64||b[11]!=0||b[104..].iter().any(|v|*v!=0){return Err("Invalid entry");}let name:[u8;64]=b[40..104].try_into().unwrap();let s=core::str::from_utf8(&name[..n as usize]).map_err(|_|"Invalid name")?;if name[n as usize..].iter().any(|v|*v!=0)||if id==ROOT{n!=0}else{!name_ok(s)}{return Err("Invalid name");}let e=Self{id,parent,dir:kind==1,owner,len:get(b,12),start:get(b,16),sum:get(b,20),created:get64(b,24),modified:get64(b,32),name,n};if e.len as usize>MAX_FILE||(e.dir&&e.len!=0)||(e.len==0&&(e.start!=0||e.sum!=crc(&[])))||e.blocks()>BLOCKS||(e.blocks()>0&&(e.start as usize>=BLOCKS||e.start as usize+e.blocks()>BLOCKS)){return Err("Invalid extent");}Ok(e)}
}
#[derive(Clone,Debug,PartialEq,Eq)]struct Catalog{gen:u32,next:u32,entries:Vec<Entry>}
impl Catalog{
    fn find(&self,id:u32)->Option<&Entry>{self.entries.iter().find(|e|e.id==id)}
    fn child(&self,p:u32,n:&str)->Option<&Entry>{self.entries.iter().find(|e|e.parent==p&&e.name().eq_ignore_ascii_case(n))}
    fn validate(&self)->R<()>{
        if self.gen==0||self.next<14||self.entries.len()>COUNT{return Err("Bad catalogue");}
        let r=self.find(ROOT).ok_or("Root missing")?;if r.parent!=0||!r.dir||r.owner!=0||r.n!=0{return Err("Bad root");}
        let home=self.find(HOME).ok_or("Home missing")?;if !home.dir||home.owner!=0||home.parent!=4{return Err("Bad home");}
        for (id,parent,name) in [(2,1,"System"),(3,1,"Applications"),(4,1,"Users"),(6,5,"Desktop"),(7,5,"Documents"),(8,5,"Downloads"),(9,5,"Pictures"),(10,5,"Music"),(11,5,"Videos"),(12,1,"ProgramData"),(13,1,"Recovery"),(14,1,"Temp")]{
            let e=self.find(id).ok_or("System directory missing")?;if e.parent!=parent||!e.dir||e.owner!=0||e.name()!=name{return Err("System directory invalid");}
        }
        let mut used=[false;BLOCKS];
        for (i,e)in self.entries.iter().enumerate(){
            if e.id>=self.next||(e.id>14&&e.owner!=1)||self.entries[..i].iter().any(|v|v.id==e.id||(v.parent==e.parent&&v.name().eq_ignore_ascii_case(e.name()))){return Err("Duplicate entry");}
            if e.id!=ROOT{if !self.find(e.parent).is_some_and(|v|v.dir){return Err("Missing parent");}let mut p=e.parent;let mut writable=false;for _ in 0..COUNT{if p==HOME||matches!(p,6..=11){writable=true;}if p==ROOT{break;}p=self.find(p).ok_or("Missing ancestor")?.parent;}if p!=ROOT||e.owner==1&&!writable{return Err("Directory loop or protected parent");}}
            for j in e.start as usize..e.start as usize+e.blocks(){if used[j]{return Err("Overlapping extents");}used[j]=true;}
        }Ok(())
    }
    fn bytes(&self)->Vec<u8>{let mut b=vec![0;META as usize*512];for(i,e)in self.entries.iter().enumerate(){e.encode(&mut b[i*RECORD..(i+1)*RECORD]);}b}
    fn from_bytes(gen:u32,next:u32,count:usize,b:&[u8])->R<Self>{if count>COUNT||b.len()!=COUNT*RECORD||b[count*RECORD..].iter().any(|x|*x!=0){return Err("Bad catalogue length");}let mut entries=Vec::new();for i in 0..count{entries.push(Entry::decode(&b[i*RECORD..(i+1)*RECORD])?);}let c=Self{gen,next,entries};c.validate()?;Ok(c)}
    fn writable_parent(&self,id:u32)->bool{self.find(id).is_some_and(|e|e.dir&&(e.owner==1||id==HOME||matches!(id,6..=11)))}
}
fn read_data(d:&mut impl Device,e:&Entry)->R<Vec<u8>>{if e.dir{return Err("Directory is not a file");}let mut v=vec![0;e.len as usize];for(i,part)in v.chunks_mut(512).enumerate(){let mut b=[0;512];if !d.read(DATA+e.start+i as u32,&mut b){return Err("File read failed");}part.copy_from_slice(&b[..part.len()]);}if crc(&v)!=e.sum{return Err("File checksum failed");}Ok(v)}
fn bank(d:&mut impl Device,base:u32)->R<Option<Catalog>>{let mut h=[0;512];if !d.read(base,&mut h){return Err("Metadata read failed");}if &h[..8]!=MAGIC||get(&h,508)!=crc(&h[..508])||get(&h,8)!=1||get(&h,28)!=DATA||get(&h,32)!=END{return Ok(None);}
    let mut b=vec![0;META as usize*512];for(i,c)in b.chunks_mut(512).enumerate(){let mut q=[0;512];if !d.read(base+1+i as u32,&mut q){return Err("Metadata read failed");}c.copy_from_slice(&q);}
    if crc(&b)!=get(&h,24){return Ok(None);}let Ok(c)=Catalog::from_bytes(get(&h,12),get(&h,16),get(&h,20)as usize,&b)else{return Ok(None);};
    for e in c.entries.iter().filter(|e|!e.dir){if read_data(d,e).is_err(){return Ok(None);}}Ok(Some(c))
}
fn write_bank(d:&mut impl Device,base:u32,c:&Catalog)->R<()>{
    c.validate()?;let b=c.bytes();for(i,part)in b.chunks_exact(512).enumerate(){let q:&[u8;512]=part.try_into().unwrap();if !d.write(base+1+i as u32,q){return Err("Metadata write failed");}}
    if !d.flush(){return Err("Metadata flush failed");}let mut h=[0;512];h[..8].copy_from_slice(MAGIC);put(&mut h,8,1);put(&mut h,12,c.gen);put(&mut h,16,c.next);put(&mut h,20,c.entries.len()as u32);put(&mut h,24,crc(&b));put(&mut h,28,DATA);put(&mut h,32,END);let sum=crc(&h[..508]);put(&mut h,508,sum);
    if !d.write(base,&h)||!d.flush(){return Err("Commit failed");}
    if bank(d,base)?.as_ref()!=Some(c){return Err("Commit readback failed");}Ok(())
}
pub struct Fs{cat:Catalog,old:Option<Catalog>,base:u32,pub recovered:bool,pub readonly:bool}
impl Fs{
    pub fn mount(d:&mut impl Device)->R<Option<Self>>{
        let a=bank(d,FIRST)?;let b=bank(d,FIRST+BANK)?;
        let (cat,old,base)=match(a,b){(Some(a),Some(b))=>{if a.gen==b.gen&&a!=b{return Err("Conflicting catalogue generations");}if a.gen>=b.gen{(a,Some(b),FIRST)}else{(b,Some(a),FIRST+BANK)}},(Some(a),None)=>(a,None,FIRST),(None,Some(b))=>(b,None,FIRST+BANK),(None,None)=>{let mut h=[0;512];for lba in FIRST..END{if !d.read(lba,&mut h){return Err("Volume scan failed");}if h.iter().any(|x|*x!=0){return Err("Unrecognised or damaged Hi5FS - no writes");}}return Ok(None);}};
        let mut h=[0;512];let other=if base==FIRST{FIRST+BANK}else{FIRST};let recovered=old.is_none()&&d.read(other,&mut h)&&h.iter().any(|b|*b!=0);
        Ok(Some(Self{cat,old,base,recovered,readonly:false}))
    }
    pub fn format(d:&mut impl Device,username:&str,now:u64)->R<Self>{
        if Self::mount(d)?.is_some(){return Err("Volume already exists");}
        let home=if name_ok(username){username}else{"Local"};let mut entries=Vec::new();
        for(id,p,n)in [(1,0,""),(2,1,"System"),(3,1,"Applications"),(4,1,"Users"),(5,4,home),(6,5,"Desktop"),(7,5,"Documents"),(8,5,"Downloads"),(9,5,"Pictures"),(10,5,"Music"),(11,5,"Videos"),(12,1,"ProgramData"),(13,1,"Recovery"),(14,1,"Temp")]{entries.push(Entry::new(id,p,true,0,n,now));}
        let cat=Catalog{gen:1,next:15,entries};write_bank(d,FIRST,&cat)?;Ok(Self{cat,old:None,base:FIRST,recovered:false,readonly:false})
    }
    pub fn entries(&self)->&[Entry]{&self.cat.entries}
    pub fn entry(&self,id:u32)->Option<&Entry>{self.cat.find(id)}
    pub fn child(&self,p:u32,n:&str)->Option<u32>{self.cat.child(p,n).map(|e|e.id)}
    pub fn resolve(&self,cwd:u32,path:&str)->Option<u32>{let mut p=if path.starts_with('/'){ROOT}else{cwd};for n in path.split('/').filter(|n|!n.is_empty()){if n=="."{continue;}if n==".."{p=self.entry(p)?.parent.max(ROOT);}else{p=self.child(p,n)?;}}Some(p)}
    pub fn split_path(&self,cwd:u32,path:&str)->R<(u32,String)>{let (p,n)=match path.rsplit_once('/') {Some((p,n))=>(self.resolve(cwd,if p.is_empty(){"/"}else{p}).ok_or("Parent not found")?,n),None=>(cwd,path)};if !name_ok(n){return Err("Invalid name (1-64 ASCII characters)");}Ok((p,String::from(n)))}
    pub fn path(&self,id:u32)->String{let mut names=Vec::new();let mut p=id;for _ in 0..COUNT{if p==ROOT{break;}let Some(e)=self.entry(p)else{return String::from("?");};names.push(e.name());p=e.parent;}let mut s=String::new();for n in names.iter().rev(){s.push('/');s.push_str(n);}if s.is_empty(){s.push('/');}s}
    pub fn generation(&self)->u32{self.cat.gen}
    pub fn used_blocks(&self)->usize{self.cat.entries.iter().map(|e|e.blocks()).sum()}
    fn alloc(&self,n:usize)->R<u32>{if n==0{return Ok(0);}let mut used=[false;BLOCKS];for c in core::iter::once(&self.cat).chain(self.old.iter()){for e in &c.entries{for i in e.start as usize..e.start as usize+e.blocks(){used[i]=true;}}}let mut run=0;for(i,v)in used.iter().enumerate(){if *v{run=0;}else{run+=1;if run==n{return Ok((i+1-n)as u32);}}}Err("No contiguous space (previous generation retained)")}
    fn commit(&mut self,d:&mut impl Device,mut c:Catalog)->R<()>{if self.readonly{return Err("Volume read-only; reopen");}c.gen=self.cat.gen.checked_add(1).ok_or("Generation exhausted")?;c.validate()?;let base=if self.base==FIRST{FIRST+BANK}else{FIRST};if let Err(e)=write_bank(d,base,&c){self.readonly=true;return Err(e);}self.old=Some(core::mem::replace(&mut self.cat,c));self.base=base;self.recovered=false;Ok(())}
    pub fn read(&self,d:&mut impl Device,id:u32)->R<Vec<u8>>{read_data(d,self.entry(id).ok_or("File missing")?)}
    pub fn mkdir(&mut self,d:&mut impl Device,parent:u32,name:&str,now:u64)->R<u32>{self.add(d,parent,name,true,&[],now)}
    pub fn create(&mut self,d:&mut impl Device,parent:u32,name:&str,data:&[u8],now:u64)->R<u32>{self.add(d,parent,name,false,data,now)}
    fn add(&mut self,d:&mut impl Device,parent:u32,name:&str,dir:bool,data:&[u8],now:u64)->R<u32>{if !self.cat.writable_parent(parent){return Err("Protected directory");}if !name_ok(name)||self.child(parent,name).is_some(){return Err("Invalid or duplicate name");}if self.cat.entries.len()>=COUNT{return Err("Catalogue full (64 entries)");}let mut c=self.cat.clone();let id=c.next;c.next=c.next.checked_add(1).ok_or("ID exhausted")?;let mut e=Entry::new(id,parent,dir,1,name,now);if !dir{self.payload(d,&mut e,data)?;}c.entries.push(e);self.commit(d,c)?;Ok(id)}
    fn payload(&mut self,d:&mut impl Device,e:&mut Entry,data:&[u8])->R<()>{if self.readonly{return Err("Volume read-only; reopen");}if data.len()>MAX_FILE{return Err("File exceeds 256 KiB");}let start=self.alloc((data.len()+511)/512)?;for(i,p)in data.chunks(512).enumerate(){let mut b=[0;512];b[..p.len()].copy_from_slice(p);if !d.write(DATA+start+i as u32,&b){self.readonly=true;return Err("Data write failed");}}
        if !d.flush(){self.readonly=true;return Err("Data flush failed");}e.start=start;e.len=data.len()as u32;e.sum=crc(data);if read_data(d,e).as_deref()!=Ok(data){self.readonly=true;return Err("Data readback failed");}Ok(())}
    pub fn write(&mut self,d:&mut impl Device,id:u32,data:&[u8],now:u64)->R<()>{let mut c=self.cat.clone();let e=c.entries.iter_mut().find(|e|e.id==id).ok_or("File missing")?;if e.dir||e.owner!=1{return Err("Protected or not a file");}self.payload(d,e,data)?;e.modified=now;self.commit(d,c)}
    pub fn copy(&mut self,d:&mut impl Device,id:u32,parent:u32,name:&str,now:u64)->R<u32>{let b=self.read(d,id)?;self.create(d,parent,name,&b,now)}
    pub fn relocate(&mut self,d:&mut impl Device,id:u32,parent:u32,name:&str,now:u64)->R<()>{if !self.cat.writable_parent(parent)||!name_ok(name)||self.child(parent,name).is_some_and(|v|v!=id){return Err("Protected parent or duplicate name");}let mut c=self.cat.clone();let e=c.entries.iter_mut().find(|e|e.id==id).ok_or("Entry missing")?;if e.owner!=1{return Err("Protected entry");}let mut n=[0;64];n[..name.len()].copy_from_slice(name.as_bytes());e.parent=parent;e.name=n;e.n=name.len()as u8;e.modified=now;self.commit(d,c)}
    pub fn remove(&mut self,d:&mut impl Device,id:u32)->R<()>{if !self.entry(id).is_some_and(|e|e.owner==1){return Err("Protected or missing entry");}if self.cat.entries.iter().any(|e|e.parent==id){return Err("Directory not empty");}let mut c=self.cat.clone();c.entries.retain(|e|e.id!=id);self.commit(d,c)}
    pub fn check(&self,d:&mut impl Device)->R<()>{self.cat.validate()?;for e in self.entries().iter().filter(|e|!e.dir){self.read(d,e.id)?;}Ok(())}
}
pub struct MemoryDisk{data:Vec<u8>,pub writes:usize,pub fail_after:Option<usize>}
impl MemoryDisk{pub fn new()->Self{Self{data:vec![0;(END-FIRST)as usize*512],writes:0,fail_after:None}}pub fn corrupt(&mut self,lba:u32){self.data[(lba-FIRST)as usize*512]^=1;}}
impl Device for MemoryDisk{fn read(&mut self,lba:u32,b:&mut[u8;512])->bool{if !(FIRST..END).contains(&lba){return false;}let i=(lba-FIRST)as usize*512;b.copy_from_slice(&self.data[i..i+512]);true}fn write(&mut self,lba:u32,b:&[u8;512])->bool{if !(FIRST..END).contains(&lba)||self.fail_after.is_some_and(|n|self.writes>=n){return false;}self.writes+=1;let i=(lba-FIRST)as usize*512;self.data[i..i+512].copy_from_slice(b);true}fn flush(&mut self)->bool{true}}
pub fn self_test()->bool{
    let test=||->R<()>{
        let mut d=MemoryDisk::new();let mut f=Fs::format(&mut d,"tester",1)?;
        let p=f.mkdir(&mut d,DOCUMENTS,"Work",2)?;if f.mkdir(&mut d,DOCUMENTS,"work",2).is_ok()||f.create(&mut d,p,"bad/name",b"",2).is_ok()||f.create(&mut d,p,"large",&vec![0;MAX_FILE+1],2).is_ok(){return Err("Invalid create");}let data=vec![0x5a;8193];let id=f.create(&mut d,p,"sample.bin",&data,3)?;
        let empty=f.create(&mut d,p,"empty.txt",&[],3)?;if !f.read(&mut d,empty)?.is_empty(){return Err("Empty file");}f.remove(&mut d,empty)?;let q=f.copy(&mut d,id,DOCUMENTS,"copy.bin",4)?;f.relocate(&mut d,q,p,"renamed.bin",5)?;
        if f.resolve(ROOT,"/Users/tester/Documents/Work/renamed.bin")!=Some(q)||f.read(&mut d,id)?!=data||f.remove(&mut d,p).is_ok()||f.relocate(&mut d,p,p,"loop",6).is_ok()||f.mkdir(&mut d,ROOT,"bad",6).is_ok(){return Err("Operations");}
        f.remove(&mut d,q)?;f.write(&mut d,id,b"current",7)?;let gen=f.generation();let active=f.base;let mut backup=MemoryDisk{data:d.data.clone(),writes:0,fail_after:None};backup.corrupt(active);let older=Fs::mount(&mut backup)?.ok_or("Missing previous generation")?;if older.generation()!=gen-1||!older.recovered||older.read(&mut backup,id)?!=data{return Err("Corruption fallback");}drop(older);drop(backup);drop(f);
        let mut f=Fs::mount(&mut d)?.ok_or("Missing mount")?;if f.generation()!=gen||f.read(&mut d,id)?!=b"current"{return Err("Mount");}
        d.fail_after=Some(d.writes+2);if f.write(&mut d,id,b"torn",8).is_ok()||!f.readonly{return Err("Torn write");}d.fail_after=None;drop(f);
        let f=Fs::mount(&mut d)?.ok_or("Missing recovery")?;if f.read(&mut d,id)?!=b"current"{return Err("Rollback");}drop(f);
        d.corrupt(active);let other=if active==FIRST{FIRST+BANK}else{FIRST};if let Ok(Some(older))=Fs::mount(&mut d){if older.generation()>=gen||!older.recovered{return Err("Fallback generation");}}d.corrupt(other);if Fs::mount(&mut d).is_ok(){return Err("Both generations corrupt should fail closed");}Ok(())
    };test().is_ok()
}
