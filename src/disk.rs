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
        if lba>48||!wait(a+0x20,0x88,0)||rd(a+0x38)!=0{serial::metric("H5K: AHCI_NOT_READY_TFD=",rd(a+0x20)as u64);return false;}
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
fn crc(b:&[u8])->u32{let mut v=0xffff_ffffu32;for byte in b{v^=*byte as u32;for _ in 0..8{v=(v>>1)^if v&1!=0{0xedb88320}else{0};}}!v}
fn put(b:&mut [u8],off:usize,n:u32){b[off..off+4].copy_from_slice(&n.to_le_bytes());}
fn get(b:&[u8],off:usize)->u32{u32::from_le_bytes([b[off],b[off+1],b[off+2],b[off+3]])}
fn decode(b:&[u8;512])->Option<(u32,bool,usize)>{
    if &b[..8]!=RECORD||get(b,508)!=crc(&b[..508]){return None;}
    let gen=get(b,8);let len=get(b,16)as usize;let flag=get(b,12);
    if gen==0||flag>1||len>CAP||(flag==0&&len!=0)||!b[20..20+len].iter().all(|c|*c==10||(32..127).contains(c)){return None;}
    Some((gen,flag==1,len))
}
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
    // Bank headers at 16 and 32; each bank has eight 512-byte payload sectors.
    // Never touches document slots 1..8. CRC checks detect incomplete writes.
    fn download_bank(p:Port,base:u32,out:&mut[u8;4096])->Option<(u32,usize)>{
        let mut h=[0;512];if !p.read(base,&mut h)||&h[..8]!=b"H5DL0001"||get(&h,508)!=crc(&h[..508]){return None;}
        let gen=get(&h,8);let len=get(&h,12)as usize;if gen==0||len>4096{return None;}
        for i in 0..8{let mut b=[0;512];if !p.read(base+1+i as u32,&mut b){return None;}out[i*512..(i+1)*512].copy_from_slice(&b);}
        if crc(&out[..len])!=get(&h,16){return None;}Some((gen,len))
    }
    pub fn load_download(&mut self,out:&mut[u8;4096])->Option<usize>{
        let p=self.port?;let a=Self::download_bank(p,16,out);let b=Self::download_bank(p,32,out);
        let base=match(a,b){(Some(x),Some(y))=>if x.0>=y.0{16}else{32},(Some(_),None)=>16,(None,Some(_))=>32,_=>return None};
        let (_,len)=Self::download_bank(p,base,out)?;serial::metric("H5K: DOWNLOAD_LOADED_CRC=",crc(&out[..len])as u64);Some(len)
    }
    pub fn save_download(&mut self,data:&[u8])->bool{
        let Some(p)=self.port else{return false;};if data.len()>4096{return false;}
        let mut h=[0;512];if !p.read(0,&mut h)||!header_ok(&h){self.port=None;return false;}
        let mut scratch=[0;4096];let a=Self::download_bank(p,16,&mut scratch);let b=Self::download_bank(p,32,&mut scratch);
        let (base,gen)=match(a,b){(Some(x),Some(y))=>if x.0>=y.0{(32,x.0)}else{(16,y.0)},(Some(x),None)=>(32,x.0),(None,Some(y))=>(16,y.0),_=>(16,0)};
        let Some(gen)=gen.checked_add(1)else{return false;};
        for i in 0..8{let mut block=[0;512];let start=i*512;if start<data.len(){let end=(start+512).min(data.len());block[..end-start].copy_from_slice(&data[start..end]);}if !p.write(base+1+i as u32,&mut block){self.port=None;return false;}}
        if !p.flush(){self.port=None;return false;}
        h.fill(0);h[..8].copy_from_slice(b"H5DL0001");put(&mut h,8,gen);put(&mut h,12,data.len()as u32);put(&mut h,16,crc(data));let c=crc(&h[..508]);put(&mut h,508,c);
        if !p.write(base,&mut h)||!p.flush()||Self::download_bank(p,base,&mut scratch)!=Some((gen,data.len()))||&scratch[..data.len()]!=data{self.port=None;return false;}
        serial::metric("H5K: DOWNLOAD_SAVED_CRC=",crc(data)as u64);serial::line("H5K: DOWNLOAD_SAVED");true
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
