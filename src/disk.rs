//! VM-only AHCI data store. Never writes an unrecognised disk.
use crate::{platform as p,serial};
use core::ptr;
const HEADER:&[u8]=b"HI5OS-DATA-V1-7D1F6C0284A9E3B5";
const RECORD:&[u8;8]=b"H5NOTE01";
pub const CAP:usize=384;
pub const FILE_COUNT:usize=4;
pub const NAMES:[&str;FILE_COUNT]=["notes.txt","work.txt","personal.txt","office.txt"];
#[repr(C,align(4096))]
struct Dma([u8;4096]);
static mut CL:Dma=Dma([0;4096]);
static mut RF:Dma=Dma([0;4096]);
static mut CT:Dma=Dma([0;4096]);
static mut DB:Dma=Dma([0;4096]);
#[derive(Clone,Copy)]struct Port{base:usize}
fn rd(a:usize)->u32{unsafe{ptr::read_volatile(a as *const u32)}}
fn wr(a:usize,v:u32){unsafe{ptr::write_volatile(a as *mut u32,v)}}
fn wait(a:usize,mask:u32,expected:u32)->bool{for _ in 0..2_000_000{if rd(a)&mask==expected{return true;}core::hint::spin_loop();}false}
fn pci_read(b:u32,off:u32)->u32{p::out32(0xcf8,b|off);p::input32(0xcfc)}
fn pci_write(b:u32,off:u32,v:u32){p::out32(0xcf8,b|off);p::out32(0xcfc,v);}
fn header_ok(b:&[u8;512])->bool{&b[..HEADER.len()]==HEADER&&b[HEADER.len()..].iter().all(|v|*v==0)}
impl Port{
    fn stop(self)->bool{let a=self.base+0x18;wr(a,rd(a)&!1);if !wait(a,1<<15,0){return false;}wr(a,rd(a)&!(1<<4));wait(a,1<<14,0)}
    fn setup(self)->bool{
        let a=self.base;
        if rd(a+0x38)!=0||rd(a+0x34)!=0||!self.stop(){return false;}
        unsafe{
            let cl=ptr::addr_of_mut!(CL.0).cast::<u8>();let rf=ptr::addr_of_mut!(RF.0).cast::<u8>();
            if cl as usize>=0xffff_f000||rf as usize>=0xffff_f000{return false;}
            ptr::write_bytes(cl,0,4096);ptr::write_bytes(rf,0,4096);
            wr(a,cl as u32);wr(a+4,0);wr(a+8,rf as u32);wr(a+12,0);
        }
        wr(a+0x14,0);wr(a+0x10,u32::MAX);wr(a+0x30,u32::MAX);
        wr(a+0x18,rd(a+0x18)|(1<<4));wr(a+0x18,rd(a+0x18)|1);true
    }
    fn transfer(self,cmd:u8,lba:u32,buf:&mut [u8;512],write:bool)->bool{
        let a=self.base;
        if (lba>4161&&!(8192..=32767).contains(&lba))||!wait(a+0x20,0x88,0)||rd(a+0x38)!=0{serial::metric("H5K: AHCI_NOT_READY_TFD=",rd(a+0x20)as u64);return false;}
        let flush=cmd==0xea;
        unsafe{
            let cl=ptr::addr_of_mut!(CL.0).cast::<u8>();let ct=ptr::addr_of_mut!(CT.0).cast::<u8>();let db=ptr::addr_of_mut!(DB.0).cast::<u8>();
            if ct as usize>=0xffff_f000||db as usize>=0xffff_f000{return false;}
            ptr::write_bytes(cl,0,32);ptr::write_bytes(ct,0,256);
            if write&&!flush{ptr::copy_nonoverlapping(buf.as_ptr(),db,512);}else{ptr::write_bytes(db,0,512);}
            ptr::write_unaligned(cl.cast::<u16>(),5|if write&&!flush{1<<6}else{0});
            ptr::write_unaligned(cl.add(2).cast::<u16>(),if flush{0}else{1});
            ptr::write_unaligned(cl.add(8).cast::<u32>(),ct as u32);
            ct.write(0x27);ct.add(1).write(0x80);ct.add(2).write(cmd);
            ct.add(4).write(lba as u8);ct.add(5).write((lba>>8)as u8);ct.add(6).write((lba>>16)as u8);ct.add(7).write(0x40);ct.add(8).write((lba>>24)as u8);
            if !flush{ct.add(12).write(1);ptr::write_unaligned(ct.add(128).cast::<u32>(),db as u32);ptr::write_unaligned(ct.add(140).cast::<u32>(),511);}
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            wr(a+0x10,u32::MAX);wr(a+0x38,1);
            if !wait(a+0x38,1,0)||rd(a+0x10)&(1<<30)!=0||rd(a+0x20)&1!=0{serial::metric("H5K: AHCI_CMD_ERROR_IS=",rd(a+0x10)as u64);serial::metric("H5K: AHCI_CMD_ERROR_TFD=",rd(a+0x20)as u64);serial::metric("H5K: AHCI_CMD_ERROR_CI=",rd(a+0x38)as u64);return false;}
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            if !write&&!flush{ptr::copy_nonoverlapping(db,buf.as_mut_ptr(),512);}
        }true
    }
    fn read(self,lba:u32,b:&mut [u8;512])->bool{self.transfer(0x25,lba,b,false)}
    fn write(self,lba:u32,b:&mut [u8;512])->bool{self.transfer(0x35,lba,b,true)}
    fn flush(self)->bool{self.transfer(0xea,0,&mut [0;512],false)}
}
fn find()->Option<Port>{
    // Q35 AHCI, bus zero. No arbitrary I/O BARs or physical-drive use.
    for dev in 0..32u32{for fun in 0..8u32{
        let b=0x80000000|(dev<<11)|(fun<<8);
        if pci_read(b,0)&0xffff==0xffff||pci_read(b,8)>>8!=0x010601{continue;}
        serial::metric("H5K: AHCI_PCI_BDF=",b as u64);
        let bar=pci_read(b,0x24);serial::metric("H5K: AHCI_BAR=",bar as u64);
        if bar&7!=0||bar==0||bar==u32::MAX{continue;}
        let base=(bar&!15)as usize;if base<0x8000_0000||base>0xffff_e000{continue;}
        let cmd=pci_read(b,4);pci_write(b,4,(cmd&0xffff)|6);
        wr(base+4,(rd(base+4)|(1<<31))&!2);
        let pi=rd(base+0x0c);serial::metric("H5K: AHCI_PORTS=",pi as u64);
        for i in 0..32usize{if pi&(1u32<<i)==0{continue;}
            let port=Port{base:base+0x100+i*0x80};
            let ssts=rd(port.base+0x28);let sig=rd(port.base+0x24);
            serial::metric("H5K: AHCI_PORT=",i as u64);serial::metric("H5K: AHCI_SSTS=",ssts as u64);serial::metric("H5K: AHCI_SIG_BEFORE=",sig as u64);
            // Signature can remain uninitialised before the receive engine sees a D2H FIS.
            // Require an established physical link, not a pre-populated ATA signature.
            if ssts&15!=3{continue;}
            if sig==0xeb140101||sig==0x96690101||sig==0xc33c0101{continue;}
            if !port.setup(){serial::line("H5K: AHCI_SETUP_FAILED");return None;}
            serial::metric("H5K: AHCI_SIG_AFTER=",rd(port.base+0x24)as u64);
            let mut h=[0;512];
            if port.read(0,&mut h){if header_ok(&h){serial::line("H5K: AHCI_DATA_HEADER_OK");return Some(port);}serial::line("H5K: AHCI_HEADER_NOT_HI5OS");}else{serial::line("H5K: AHCI_HEADER_READ_FAILED");}
            if !port.stop()||rd(port.base+0x38)!=0{return None;}
        }
    }}None
}
fn crc_step(mut v:u32,b:&[u8])->u32{for byte in b{v^=*byte as u32;for _ in 0..8{v=(v>>1)^if v&1!=0{0xedb88320}else{0};}}v}
fn crc(b:&[u8])->u32{!crc_step(0xffff_ffff,b)}
fn put(b:&mut [u8],off:usize,n:u32){b[off..off+4].copy_from_slice(&n.to_le_bytes());}
fn get(b:&[u8],off:usize)->u32{u32::from_le_bytes([b[off],b[off+1],b[off+2],b[off+3]])}
fn decode(b:&[u8;512])->Option<(u32,bool,usize)>{
    if &b[..8]!=RECORD||get(b,508)!=crc(&b[..508]){return None;}
    let gen=get(b,8);let len=get(b,16)as usize;let flag=get(b,12);
    if gen==0||flag>1||len>CAP||(flag==0&&len!=0)||!b[20..20+len].iter().all(|c|*c==10||(32..127).contains(c)){return None;}
    Some((gen,flag==1,len))
}
// V2 download banks: sectors 64..2112 and 2113..4161, inside the original 16 MiB disk.
const BANK_A:u32=64;
const BANK_B:u32=2113;
pub const DL_CAP:usize=1024*1024;
#[derive(Clone,Copy)]pub struct Download{pub len:usize,pub url:[u8;128],pub url_len:usize,gen:u32,sum:u32}
pub struct Store{port:Option<Port>,generation:u32,slot:u32,file:usize}
impl Store{
    pub fn open()->(Self,Option<(bool,[u8;CAP],usize)>){
        let mut s=Self{port:find(),generation:0,slot:2,file:0};
        let Some(p)=s.port else{serial::line("H5K: DATA_DISK_MISSING");return (s,None);};
        let loaded=s.load(0);(s,loaded)
    }
    pub fn load(&mut self,file:usize)->Option<(bool,[u8;CAP],usize)>{
        if file>=FILE_COUNT{return None;}
        self.file=file;self.generation=0;self.slot=2;
        let p=self.port?;
        let offset=(file as u32)*2;
        let mut a=[0;512];let mut b=[0;512];
        if !p.read(offset+1,&mut a)||!p.read(offset+2,&mut b){self.port=None;serial::line("H5K: DATA_DISK_READ_ERROR");return None;}
        let selected=match(decode(&a),decode(&b)){
            (Some(x),Some(y))=>if y.0>x.0{Some((2,y,&b))}else{Some((1,x,&a))},
            (Some(x),None)=>Some((1,x,&a)),(None,Some(y))=>Some((2,y,&b)),_=>None
        };
        if let Some((slot,(gen,present,len),raw))=selected{
            self.generation=gen;self.slot=slot;let mut data=[0;CAP];data[..len].copy_from_slice(&raw[20..20+len]);
            serial::line("H5K: DATA_DISK_LOADED");serial::metric("H5K: DATA_LENGTH=",len as u64);serial::metric("H5K: DATA_CRC=",crc(&data[..len])as u64);
            return Some((present,data,len));
        }
        if a.iter().any(|x|*x!=0)||b.iter().any(|x|*x!=0){self.port=None;serial::line("H5K: DATA_DISK_CORRUPT");}
        else{serial::line("H5K: DATA_DISK_EMPTY");}
        None
    }
    pub fn attached(&self)->bool{self.port.is_some()}
    // Account sectors 12/13, fresh setup marker 14. Never overwrite documents or download banks.
    pub fn setup_requested(&self)->bool{let Some(p)=self.port else{return false;};let mut b=[0;512];p.read(14,&mut b)&&&b[..8]==b"H5SETUP1"&&get(&b,508)==crc(&b[..508])}
    fn account_record(b:&[u8;512])->Option<crate::account::Account>{if &b[..8]!=b"H5ACC001"||get(b,508)!=crc(&b[..508])||get(b,8)!=crate::account::ROUNDS{return None;}let len=b[12];let host_len=b[13];if len>32||host_len>32||b[14]>1||b[15]>1||!crate::account::valid_name(&b[16..16+len as usize])||!crate::account::valid_name(&b[48..48+host_len as usize]){return None;}let salt=b[80..96].try_into().ok()?;if salt==[0;16]{return None;}Some(crate::account::Account{name:b[16..48].try_into().ok()?,len,host:b[48..80].try_into().ok()?,host_len,region:b[14],dark:b[15]!=0,salt,verifier:b[96..128].try_into().ok()?})}
    // Err means unreadable/corrupted account state: lock, never silently reset.
    pub fn account(&self)->Result<Option<crate::account::Account>,()>{let Some(p)=self.port else{return Ok(None);};let mut a=[0;512];let mut b=[0;512];if !p.read(12,&mut a)||!p.read(13,&mut b){return Err(());}match(Self::account_record(&a),Self::account_record(&b)){(Some(x),Some(y))if x!=y=>Err(()),(Some(x),_)|(_,Some(x))=>Ok(Some(x)),_ if a.iter().all(|v|*v==0)&&b.iter().all(|v|*v==0)=>Ok(None),_=>Err(())}}
    pub fn create_account(&self,a:&crate::account::Account)->bool{let Some(p)=self.port else{return false;};if self.account()!=Ok(None){return false;}let mut b=[0;512];if !p.read(0,&mut b)||!header_ok(&b){return false;}b.fill(0);b[..8].copy_from_slice(b"H5ACC001");put(&mut b,8,crate::account::ROUNDS);b[12]=a.len;b[13]=a.host_len;b[14]=a.region;b[15]=a.dark as u8;b[16..48].copy_from_slice(&a.name);b[48..80].copy_from_slice(&a.host);b[80..96].copy_from_slice(&a.salt);b[96..128].copy_from_slice(&a.verifier);let sum=crc(&b[..508]);put(&mut b,508,sum);if !p.write(12,&mut b)||!p.flush(){return false;}let mut check=[0;512];if !p.read(12,&mut check)||check!=b{return false;}let _=p.write(13,&mut b)&&p.flush();self.account()==Ok(Some(*a))}

    // Previously unused sectors 10 and 11; separate from notes, package and download banks.
    // Sectors 15/16: independent browser preferences, alternating CRC records.
    fn browser_record(p:Port,lba:u32)->Option<(u32,u8)>{let mut b=[0;512];if !p.read(lba,&mut b)||&b[..8]!=b"H5BRW001"||get(&b,508)!=crc(&b[..508])||b[12]>3{return None;}let g=get(&b,8);(g>0).then_some((g,b[12]))}
    pub fn browser_settings(&self)->u8{let Some(p)=self.port else{return 0;};[Self::browser_record(p,15),Self::browser_record(p,16)].into_iter().flatten().max_by_key(|r|r.0).map_or(0,|r|r.1)}
    pub fn save_browser_settings(&self,flags:u8)->bool{let Some(p)=self.port else{return false;};if flags>3{return false;}let mut h=[0;512];if !p.read(0,&mut h)||!header_ok(&h){return false;}let a=Self::browser_record(p,15).map_or(0,|v|v.0);let b=Self::browser_record(p,16).map_or(0,|v|v.0);let Some(g)=a.max(b).checked_add(1)else{return false;};let lba=if a<=b{15}else{16};h.fill(0);h[..8].copy_from_slice(b"H5BRW001");put(&mut h,8,g);h[12]=flags;let sum=crc(&h[..508]);put(&mut h,508,sum);p.write(lba,&mut h)&&p.flush()&&Self::browser_record(p,lba)==Some((g,flags))}
    fn display_record(p:Port,lba:u32)->Option<(u32,usize,usize)>{let mut b=[0;512];if !p.read(lba,&mut b)||&b[..8]!=b"H5DSP001"||get(&b,508)!=crc(&b[..508]){return None;}let g=get(&b,8);let m=get(&b,12)as usize;let s=get(&b,16)as usize;if g==0||m>=crate::display::MODES.len()||!matches!(s,100|125|150|175|200){return None;}Some((g,m,s))}
    pub fn display_settings(&self)->Option<(usize,usize)>{let p=self.port?;[Self::display_record(p,10),Self::display_record(p,11)].into_iter().flatten().max_by_key(|v|v.0).map(|v|(v.1,v.2))}
    pub fn save_display(&self,mode:usize,scale:usize)->bool{let Some(p)=self.port else{return false;};if mode>=crate::display::MODES.len()||!matches!(scale,100|125|150|175|200){return false;}let mut h=[0;512];if !p.read(0,&mut h)||!header_ok(&h){return false;}let a=Self::display_record(p,10).map_or(0,|v|v.0);let b=Self::display_record(p,11).map_or(0,|v|v.0);let Some(g)=a.max(b).checked_add(1)else{return false;};let lba=if a<=b{10}else{11};h.fill(0);h[..8].copy_from_slice(b"H5DSP001");put(&mut h,8,g);put(&mut h,12,mode as u32);put(&mut h,16,scale as u32);let sum=crc(&h[..508]);put(&mut h,508,sum);p.write(lba,&mut h)&&p.flush()&&Self::display_record(p,lba)==Some((g,mode,scale))}

    // Bank headers at 16 and 32; each bank has eight 512-byte payload sectors.
    // Never touches document slots 1..8. CRC checks detect incomplete writes.
    fn download_bank(p:Port,base:u32,out:&mut[u8;4096])->Option<(u32,usize)>{
        let mut h=[0;512];if !p.read(base,&mut h)||&h[..8]!=b"H5DL0001"||get(&h,508)!=crc(&h[..508]){return None;}
        let gen=get(&h,8);let len=get(&h,12)as usize;if gen==0||len>4096{return None;}
        for i in 0..8{let mut b=[0;512];if !p.read(base+1+i as u32,&mut b){return None;}out[i*512..(i+1)*512].copy_from_slice(&b);}
        if crc(&out[..len])!=get(&h,16){return None;}Some((gen,len))
    }
    fn v2_header(p:Port,base:u32)->Option<Download>{
        let mut h=[0;512];if !p.read(base,&mut h)||&h[..8]!=b"H5DL0002"||get(&h,508)!=crc(&h[..508]){return None;}
        let gen=get(&h,8);let len=get(&h,12)as usize;let sum=get(&h,16);let url_len=get(&h,20)as usize;
        if gen==0||len>DL_CAP||url_len>128||!h[24..24+url_len].iter().all(|b|(33..127).contains(b)){return None;}
        let mut url=[0;128];url[..url_len].copy_from_slice(&h[24..24+url_len]);
        Some(Download{len,url,url_len,gen,sum})
    }
    fn v2_read(p:Port,base:u32,d:Download,mut out:Option<&mut[u8]>,compare:Option<&[u8]>)->bool{
        if out.as_ref().is_some_and(|b|b.len()<d.len)||compare.is_some_and(|b|b.len()!=d.len){return false;}
        let mut state=0xffff_ffff;let mut pos=0;
        while pos<d.len{let mut block=[0;512];if !p.read(base+1+(pos/512)as u32,&mut block){return false;}
            let n=(d.len-pos).min(512);state=crc_step(state,&block[..n]);
            if let Some(ref mut dst)=out{dst[pos..pos+n].copy_from_slice(&block[..n]);}
            if let Some(src)=compare{if src[pos..pos+n]!=block[..n]{return false;}}pos+=n;
        }!state==d.sum
    }
    pub fn load_download(&mut self,out:&mut[u8])->Option<Download>{
        let p=self.port?;let a=Self::v2_header(p,BANK_A);let b=Self::v2_header(p,BANK_B);
        let order=if b.map_or(0,|v|v.gen)>a.map_or(0,|v|v.gen){[(BANK_B,b),(BANK_A,a)]}else{[(BANK_A,a),(BANK_B,b)]};
        for (base,d) in order{if let Some(d)=d{if Self::v2_read(p,base,d,Some(out),None){
            serial::metric("H5K: DOWNLOAD_LOADED_CRC=",d.sum as u64);serial::metric("H5K: DOWNLOAD_URL_CRC=",crc(&d.url[..d.url_len])as u64);return Some(d);
        }}}
        // Old v0.1.2-4 downloads remain readable, but never invent their original URL.
        let old:&mut[u8;4096]=out.get_mut(..4096)?.try_into().ok()?;
        let a=Self::download_bank(p,16,old);let b=Self::download_bank(p,32,old);
        let base=match(a,b){(Some(x),Some(y))=>if x.0>=y.0{16}else{32},(Some(_),None)=>16,(None,Some(_))=>32,_=>return None};
        let (_,len)=Self::download_bank(p,base,old)?;let sum=crc(&old[..len]);
        serial::metric("H5K: DOWNLOAD_LOADED_CRC=",sum as u64);serial::line("H5K: DOWNLOAD_LEGACY_NO_URL");Some(Download{len,url:[0;128],url_len:0,gen:0,sum})
    }
    pub fn load_previous_download(&mut self,out:&mut[u8])->Option<Download>{let p=self.port?;let a=Self::v2_header(p,BANK_A)?;let b=Self::v2_header(p,BANK_B)?;if a.gen==b.gen{return None;}let(base,d)=if a.gen<b.gen{(BANK_A,a)}else{(BANK_B,b)};if !Self::v2_read(p,base,d,None,None)||!Self::v2_read(p,base,d,Some(out),None){return None;}Some(d)}
    pub fn save_download(&mut self,data:&[u8],url:&[u8])->bool{
        let Some(p)=self.port else{return false;};
        if data.len()>DL_CAP||url.len()>128||!url.iter().all(|b|(33..127).contains(b)){return false;}
        let mut h=[0;512];if !p.read(0,&mut h)||!header_ok(&h){self.port=None;return false;}
        let a=Self::v2_header(p,BANK_A).filter(|d|Self::v2_read(p,BANK_A,*d,None,None));
        let b=Self::v2_header(p,BANK_B).filter(|d|Self::v2_read(p,BANK_B,*d,None,None));
        let (base,gen)=match(a,b){(Some(x),Some(y))=>if x.gen>=y.gen{(BANK_B,x.gen)}else{(BANK_A,y.gen)},(Some(x),None)=>(BANK_B,x.gen),(None,Some(y))=>(BANK_A,y.gen),_=>(BANK_A,0)};
        let Some(gen)=gen.checked_add(1)else{return false;};
        for (i,part) in data.chunks(512).enumerate(){let mut block=[0;512];block[..part.len()].copy_from_slice(part);
            if !p.write(base+1+i as u32,&mut block){self.port=None;return false;}}
        if !p.flush(){self.port=None;return false;}
        let sum=crc(data);h.fill(0);h[..8].copy_from_slice(b"H5DL0002");put(&mut h,8,gen);put(&mut h,12,data.len()as u32);put(&mut h,16,sum);put(&mut h,20,url.len()as u32);h[24..24+url.len()].copy_from_slice(url);let c=crc(&h[..508]);put(&mut h,508,c);
        if !p.write(base,&mut h)||!p.flush(){self.port=None;return false;}
        let Some(d)=Self::v2_header(p,base)else{self.port=None;return false;};
        if d.gen!=gen||d.len!=data.len()||d.sum!=sum||&d.url[..d.url_len]!=url||!Self::v2_read(p,base,d,None,Some(data)){self.port=None;return false;}
        serial::metric("H5K: DOWNLOAD_SAVED_CRC=",sum as u64);serial::metric("H5K: DOWNLOAD_URL_CRC=",crc(url)as u64);serial::line("H5K: DOWNLOAD_SAVED");true
    }
    // Separate application bank, never overlaps the legacy notes/download sectors.
    // Single pinned package; immutable contents and idempotent reinstallation.
    fn app_header(p:Port,base:u32)->Option<Download>{
        let mut h=[0;512];if !p.read(base,&mut h)||&h[..8]!=b"H5APP001"||get(&h,508)!=crc(&h[..508]){return None;}
        let gen=get(&h,8);let len=get(&h,12)as usize;if gen==0||len!=crate::trial::PACKAGE_SIZE{return None;}
        Some(Download{len,gen,sum:get(&h,16),url:[0;128],url_len:0})
    }
    // Legacy 1 MiB banks retained for migration; new 3 MiB banks in unused sectors.
    fn work_header(p:Port,base:u32)->Option<Download>{
        let mut h=[0;512];if !p.read(base,&mut h)||&h[..8]!=b"H5WORK01"||get(&h,508)!=crc(&h[..508]){return None;}
        let gen=get(&h,8);let len=get(&h,12)as usize;let max=if base==16384||base==18432{1048576-512}else{3145728-512};if gen==0||len<12||len>crate::trialfs::SNAP_CAP||len>max{return None;}
        Some(Download{len,gen,sum:get(&h,16),url:[0;128],url_len:0})
    }
    pub fn load_workspace(&mut self,out:&mut[u8])->Option<usize>{
        let p=self.port?;let mut order=[(16384,Self::work_header(p,16384)),(18432,Self::work_header(p,18432)),(20480,Self::work_header(p,20480)),(26624,Self::work_header(p,26624))];
        order.sort_unstable_by_key(|v|core::cmp::Reverse(v.1.map_or(0,|x|x.gen)));
        for(base,v)in order{if let Some(v)=v{if out.len()>=v.len&&Self::v2_read(p,base,v,Some(out),None){serial::line("H5K: WORKSPACE_LOADED");return Some(v.len);}}}None
    }
    pub fn save_workspace(&mut self,data:&[u8])->bool{
        let Some(p)=self.port else{return false;};if data.len()<12||data.len()>crate::trialfs::SNAP_CAP{return false;}
        let mut h=[0;512];if !p.read(0,&mut h)||!header_ok(&h){return false;}
        let mut gen=0;let mut latest=0;
        for base in [16384,18432,20480,26624]{if let Some(v)=Self::work_header(p,base){if v.gen>gen&&Self::v2_read(p,base,v,None,None){gen=v.gen;latest=base;}}}
        let base=if latest==20480{26624}else{20480};
        let Some(gen)=gen.checked_add(1)else{return false;};
        for(i,part)in data.chunks(512).enumerate(){let mut block=[0;512];block[..part.len()].copy_from_slice(part);if !p.write(base+1+i as u32,&mut block){return false;}}
        if !p.flush(){return false;}h.fill(0);h[..8].copy_from_slice(b"H5WORK01");put(&mut h,8,gen);put(&mut h,12,data.len()as u32);put(&mut h,16,crc(data));let sum=crc(&h[..508]);put(&mut h,508,sum);
        if !p.write(base,&mut h)||!p.flush(){return false;}let Some(v)=Self::work_header(p,base)else{return false;};if v.gen!=gen||!Self::v2_read(p,base,v,None,Some(data)){return false;}
        serial::line("H5K: WORKSPACE_SAVED");true
    }
    fn package_enabled(p:Port)->bool{
        let mut h=[0;512];if !p.read(9,&mut h){return false;}
        h.iter().all(|v|*v==0)||&h[..8]==b"H5PKG001"&&h[8]==1&&get(&h,508)==crc(&h[..508])
    }
    fn package_state(p:Port,enabled:bool)->bool{
        let mut h=[0;512];if !p.read(0,&mut h)||!header_ok(&h){return false;}h.fill(0);
        h[..8].copy_from_slice(b"H5PKG001");h[8]=enabled as u8;let sum=crc(&h[..508]);put(&mut h,508,sum);
        if !p.write(9,&mut h)||!p.flush(){return false;}let mut check=[0;512];p.read(9,&mut check)&&check==h
    }
    pub fn uninstall_7zip(&mut self)->bool{
        let Some(p)=self.port else{return false;};if !Self::package_state(p,false){return false;}
        serial::line("H5K: APP_7ZIP_UNINSTALLED");true
    }
    pub fn install_7zip(&mut self,data:&[u8])->bool{
        let Some(p)=self.port else{return false;};if !crate::trial::verified(data){return false;}
        let mut h=[0;512];if !p.read(0,&mut h)||!header_ok(&h){return false;}
        let a=Self::app_header(p,8192);let b=Self::app_header(p,12288);
        // An exact existing valid copy need not be rewritten.
        for (base,v) in [(8192,a),(12288,b)]{if let Some(v)=v{if Self::v2_read(p,base,v,None,Some(data)){if !Self::package_state(p,true){return false;}serial::line("H5K: APP_7ZIP_INSTALLED");return true;}}}
        let (base,gen)=if a.map_or(0,|x|x.gen)>b.map_or(0,|x|x.gen){(12288,a.unwrap().gen)}else{(8192,b.map_or(0,|x|x.gen))};
        let Some(gen)=gen.checked_add(1)else{return false;};
        for (i,part) in data.chunks(512).enumerate(){let mut block=[0;512];block[..part.len()].copy_from_slice(part);if !p.write(base+1+i as u32,&mut block){return false;}}
        if !p.flush(){return false;}
        h.fill(0);h[..8].copy_from_slice(b"H5APP001");put(&mut h,8,gen);put(&mut h,12,data.len()as u32);put(&mut h,16,crc(data));let sum=crc(&h[..508]);put(&mut h,508,sum);
        if !p.write(base,&mut h)||!p.flush(){return false;}
        let Some(v)=Self::app_header(p,base)else{return false;};if v.gen!=gen||!Self::v2_read(p,base,v,None,Some(data)){return false;}
        if !Self::package_state(p,true){return false;}serial::line("H5K: APP_7ZIP_INSTALLED");true
    }
    pub fn load_7zip(&mut self,out:&mut[u8])->Option<usize>{
        let p=self.port?;if !Self::package_enabled(p){return None;}let a=Self::app_header(p,8192);let b=Self::app_header(p,12288);
        let order=if b.map_or(0,|x|x.gen)>a.map_or(0,|x|x.gen){[(12288,b),(8192,a)]}else{[(8192,a),(12288,b)]};
        for(base,v)in order{if let Some(v)=v{if Self::v2_read(p,base,v,Some(out),None)&&crate::trial::verified(&out[..v.len]){serial::line("H5K: APP_7ZIP_LOADED");return Some(v.len);}}}None
    }
    pub fn save(&mut self,present:bool,data:&[u8])->bool{
        let Some(p)=self.port else{return false;};
        if data.len()>CAP||(!present&&!data.is_empty())||self.generation==u32::MAX{return false;}
        let mut header=[0;512];if !p.read(0,&mut header)||!header_ok(&header){self.port=None;return false;}
        let mut record=[0;512];record[..8].copy_from_slice(RECORD);let gen=self.generation+1;
        put(&mut record,8,gen);put(&mut record,12,present as u32);put(&mut record,16,data.len()as u32);record[20..20+data.len()].copy_from_slice(data);
        let sum=crc(&record[..508]);put(&mut record,508,sum);
        let slot=3-self.slot;let lba=slot+2*self.file as u32;let mut check=[0;512];
        if !p.write(lba,&mut record)||!p.flush()||!p.read(lba,&mut check)||check!=record{self.port=None;return false;}
        self.generation=gen;self.slot=slot;serial::metric("H5K: DATA_SAVED_CRC=",crc(data)as u64);true
    }
}
