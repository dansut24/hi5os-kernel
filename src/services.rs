//! Explicit cooperative tasks, volatile file service, PCI network discovery.
use crate::{platform,serial};
pub struct Files { pub data:[u8;64], pub len:usize, pub present:bool }
impl Files {
    pub fn new()->Self{Self{data:[0;64],len:0,present:false}}
    pub fn write(&mut self,data:&[u8])->bool{
        if data.len()>self.data.len(){return false;}
        self.data.fill(0);self.data[..data.len()].copy_from_slice(data);self.len=data.len();self.present=true;true
    }
    pub fn read(&self)->Option<&[u8]>{if self.present{Some(&self.data[..self.len])}else{None}}
    pub fn remove(&mut self){self.data.fill(0);self.len=0;self.present=false;}
    pub fn self_test(&mut self)->bool{
        if !self.write(b"HI5OS RAM FILE")||self.read()!=Some(&b"HI5OS RAM FILE"[..]){return false;}
        self.remove();self.read().is_none()&&self.write(b"WELCOME TO HI5OS")
    }
}
pub struct Tasks {pub runs:[u64;3], next:usize}
impl Tasks {
    pub fn new()->Self{Self{runs:[0;3],next:0}}
    pub fn dispatch(&mut self)->usize{let n=self.next;self.next=(n+1)%3;self.runs[n]=self.runs[n].wrapping_add(1);n}
    pub fn self_test(&mut self)->bool{for _ in 0..9{self.dispatch();}self.runs==[3;3]}
}
pub fn pci_network()->u32{
    // Enumerate bus zero, conventional multifunction PCI devices.
    let mut count=0;
    for dev in 0..32u32{for fun in 0..8u32{
        let base=0x80000000|(dev<<11)|(fun<<8);
        platform::out32(0xcf8,base);
        if platform::input32(0xcfc)&0xffff==0xffff{if fun==0{break;}else{continue;}}
        platform::out32(0xcf8,base|8);
        if platform::input32(0xcfc)>>24==2{count+=1;}
        if fun==0{platform::out32(0xcf8,base|12);if platform::input32(0xcfc)&0x00800000==0{break;}}
    }}
    count
}
pub fn network_probe()->u32{let count=pci_network();serial::metric("H5K: PCI_NETWORK_DEVICES=",count as u64);serial::line("H5K: PCI_SCAN_OK");count}
