//! Native framebuffer GUI. PS/2 keyboard and relative mouse; no host sharing.
use crate::{boot::Framebuffer,display::glyph,platform as p,services::{Files,Tasks},serial};
use core::ptr;
type Color=[u8;3];
const BG:Color=[12,32,51];const PANEL:Color=[30,51,72];const BAR:Color=[20,40,59];const BLUE:Color=[72,157,228];const WHITE:Color=[226,239,249];const MUTED:Color=[154,183,205];const GREEN:Color=[100,218,158];
struct Canvas{fb:Framebuffer,s:usize}
impl Canvas{
    fn pixel(&self,x:usize,y:usize,c:Color){if x>=self.fb.width||y>=self.fb.height{return;}let v=if self.fb.format==0{u32::from_le_bytes([c[0],c[1],c[2],0])}else{u32::from_le_bytes([c[2],c[1],c[0],0])};unsafe{ptr::write_volatile((self.fb.base as *mut u32).add(y*self.fb.stride+x),v);}}
    fn rect(&self,x:usize,y:usize,w:usize,h:usize,c:Color){for yy in y..(y+h).min(self.fb.height){for xx in x..(x+w).min(self.fb.width){self.pixel(xx,yy,c);}}}
    fn text(&self,mut x:usize,y:usize,t:&str,c:Color){for b in t.bytes(){for (dy,bits) in glyph(b.to_ascii_uppercase()).iter().enumerate(){for dx in 0..5{if bits&(1<<(4-dx))!=0{self.rect(x+dx*self.s,y+dy*self.s,self.s,self.s,c);}}}x+=6*self.s;}}
    fn num(&self,x:usize,y:usize,mut n:u64,c:Color){let mut b=[b'0';20];let mut i=20;loop{i-=1;b[i]=b'0'+(n%10)as u8;n/=10;if n==0{break;}}if let Ok(t)=core::str::from_utf8(&b[i..]){self.text(x,y,t,c);}}
}
#[derive(Clone,Copy,PartialEq)]enum App{Home,System,Files,Network}
struct Ui{c:Canvas,app:App,menu:bool,ram:u64,nics:u32,files:Files,tasks:Tasks,mx:usize,my:usize,mouse:bool,saved:[u32;100],saved_valid:bool}
impl Ui{
    fn cursor_hide(&mut self){if !self.saved_valid{return;}for y in 0..10{for x in 0..10{if self.mx+x<self.c.fb.width&&self.my+y<self.c.fb.height{unsafe{ptr::write_volatile((self.c.fb.base as *mut u32).add((self.my+y)*self.c.fb.stride+self.mx+x),self.saved[y*10+x]);}}}}self.saved_valid=false;}
    fn cursor_show(&mut self){if !self.mouse{return;}for y in 0..10{for x in 0..10{if self.mx+x<self.c.fb.width&&self.my+y<self.c.fb.height{self.saved[y*10+x]=unsafe{ptr::read_volatile((self.c.fb.base as *const u32).add((self.my+y)*self.c.fb.stride+self.mx+x))};if x==0||x==y||y==9&&x<5{self.c.pixel(self.mx+x,self.my+y,WHITE);}}}}self.saved_valid=true;}
    fn draw(&mut self){
        self.saved_valid=false;
        let c=&self.c;let w=c.fb.width;let h=c.fb.height;let s=c.s;let bar=24*s;
        c.rect(0,0,w,h,BG);
        c.rect(0,0,w,24*s,BAR);c.text(8*s,8*s,"HI5OS",BLUE);c.text(66*s,8*s,"DEVELOPER PREVIEW 0.0.8",WHITE);
        let x=12*s;let y=36*s;let gap=29*s;
        for (i,t) in ["1 SYSTEM","2 FILES","3 NETWORK"].iter().enumerate(){let yy=y+i*gap;c.rect(x,yy,82*s,22*s,PANEL);c.rect(x+3*s,yy+4*s,8*s,13*s,BLUE);c.text(x+15*s,yy+8*s,t,WHITE);}
        let px=104*s;let py=36*s;let pw=w.saturating_sub(px+10*s);let ph=h.saturating_sub(py+bar+12*s);
        c.rect(px,py,pw,ph,PANEL);c.rect(px,py,pw,20*s,BAR);
        let name=match self.app{App::Home=>"WELCOME",App::System=>"SYSTEM INFORMATION",App::Files=>"RAM FILES",App::Network=>"NETWORK"};
        c.text(px+6*s,py+6*s,name,WHITE);
        if self.app!=App::Home&&pw>20*s{c.rect(px+pw-18*s,py+2*s,16*s,16*s,BLUE);c.text(px+pw-13*s,py+6*s,"X",WHITE);}
        let tx=px+8*s;let ty=py+32*s;let step=13*s;
        match self.app{
            App::Home=>{c.text(tx,ty,"WELCOME TO HI5OS",BLUE);c.text(tx,ty+step,"NATIVE KERNEL DESKTOP",WHITE);c.text(tx,ty+2*step,"SELECT AN APP OR USE 1 2 3",MUTED);c.text(tx,ty+3*step,"M OPENS START MENU",MUTED);c.text(tx,ty+4*step,"BOOT CHECKS PASSED",GREEN);}
            App::System=>{c.text(tx,ty,"OWN PAGE TABLES",GREEN);c.text(tx,ty+step,"CPU EXCEPTIONS AND GUARD",GREEN);c.text(tx,ty+2*step,"RING 3 SYSCALL PROBE",GREEN);c.text(tx,ty+3*step,"MANAGED MIB:",WHITE);c.num(tx+78*s,ty+3*step,self.ram,BLUE);c.text(tx,ty+4*step,"COOPERATIVE TASKS: 3",WHITE);c.text(tx,ty+5*step,"TIMER IRQ ACTIVE",WHITE);}
            App::Files=>{c.text(tx,ty,"VOLATILE RAM WORKSPACE",BLUE);if let Some(b)=self.files.read(){c.text(tx,ty+step,"NOTES.TXT",WHITE);if let Ok(t)=core::str::from_utf8(b){c.text(tx,ty+2*step,t,GREEN);}}else{c.text(tx,ty+step,"NO FILE",MUTED);}c.text(tx,ty+4*step,"N NEW NOTE   D DELETE",WHITE);c.text(tx,ty+5*step,"NOT SAVED AFTER REBOOT",MUTED);}
            App::Network=>{c.text(tx,ty,"PCI NETWORK DEVICES:",WHITE);c.num(tx+126*s,ty,self.nics as u64,BLUE);c.text(tx,ty+step,"HARDWARE DISCOVERY ONLY",MUTED);c.text(tx,ty+2*step,"NO NETWORK DRIVER OR IP",MUTED);c.text(tx,ty+3*step,"OFFLINE TEST MODE",GREEN);}
        }
        c.rect(0,h-bar,w,bar,BAR);c.rect(4*s,h-bar+3*s,56*s,18*s,BLUE);c.text(9*s,h-bar+8*s,"START",WHITE);c.text(69*s,h-bar+8*s,"HI5 DESKTOP",WHITE);
        if self.menu{let my=h.saturating_sub(bar+86*s);c.rect(4*s,my,100*s,86*s,PANEL);c.text(10*s,my+7*s,"HI5 START",BLUE);for(i,t)in ["1 SYSTEM","2 FILES","3 NETWORK","ESC CLOSE"].iter().enumerate(){c.text(10*s,my+(23+i*14)*s,t,WHITE);}}
        self.cursor_show();
    }
    fn select(&mut self,a:App){self.app=a;self.menu=false;serial::line(match a{App::System=>"H5K: GUI_SYSTEM_OPEN",App::Files=>"H5K: GUI_FILES_OPEN",App::Network=>"H5K: GUI_NETWORK_OPEN",App::Home=>"H5K: GUI_HOME"});self.draw();}
    fn key(&mut self,key:u8){match key{
        0x02=>self.select(App::System),0x03=>self.select(App::Files),0x04=>self.select(App::Network),
        0x32=>{self.menu=!self.menu;self.draw();},0x01=>{if self.menu{self.menu=false;self.draw();}else{self.select(App::Home);}},
        0x31 if self.app==App::Files=>{self.files.write(b"NEW HI5OS NOTE");serial::line("H5K: GUI_FILE_CREATED");self.draw();},
        0x20 if self.app==App::Files=>{self.files.remove();serial::line("H5K: GUI_FILE_DELETED");self.draw();},_=>{}
    }}
    fn click(&mut self){let s=self.c.s;let x=self.mx;let y=self.my;let h=self.c.fb.height;let w=self.c.fb.width;if y>=h-24*s&&x<64*s{self.menu=!self.menu;self.draw();return;}if self.menu&&x<104*s{let top=h.saturating_sub(110*s);if y>=top+18*s&&y<top+80*s{match (y-top-18*s)/(14*s){0=>self.select(App::System),1=>self.select(App::Files),2=>self.select(App::Network),_=>{self.menu=false;self.draw();}}return;}}
        if x>=12*s&&x<94*s&&y>=36*s&&y<123*s{match (y-36*s)/(29*s){0=>self.select(App::System),1=>self.select(App::Files),2=>self.select(App::Network),_=>{}}return;}
        if x>=w.saturating_sub(28*s)&&y>=36*s&&y<56*s{self.select(App::Home);}
    }
}
fn wait_write()->bool{for _ in 0..10000{if p::input(0x64)&2==0{return true;}}false}
fn command(b:u8)->bool{if wait_write(){p::out(0x64,b);true}else{false}}
fn data(b:u8)->bool{if wait_write(){p::out(0x60,b);true}else{false}}
fn mouse_cmd(b:u8)->bool{if !command(0xd4)||!data(b){return false;}for _ in 0..100000{let status=p::input(0x64);if status&1!=0{let v=p::input(0x60);if status&0x20!=0{return v==0xfa;}}}false}
fn init_input()->bool{
    for _ in 0..64{if p::input(0x64)&1==0{break;}p::input(0x60);}
    command(0xae);command(0xa8);
    // UEFI Q35 commonly leaves set-1 translation enabled. Explicitly set it.
    if command(0x20){for _ in 0..10000{if p::input(0x64)&1!=0{let cfg=p::input(0x60);if command(0x60){data((cfg|0x40)&!0x33);}break;}}}
    mouse_cmd(0xf6)&&mouse_cmd(0xf4)
}
pub fn run(fb:Framebuffer,ram:u64,nics:u32,files:Files,tasks:Tasks)->!{
    let s=(fb.width/330).min(fb.height/230).clamp(1,3);
    let mut ui=Ui{c:Canvas{fb,s},app:App::Home,menu:false,ram,nics,files,tasks,mx:fb.width/2,my:fb.height/2,mouse:false,saved:[0;100],saved_valid:false};
    ui.mouse=init_input();
    ui.draw();serial::line("H5K: FRAMEBUFFER_OK");serial::line("H5K: DESKTOP_READY");
    p::timer_init();
    let start=p::ticks();
    while p::ticks().wrapping_sub(start)<3{p::wait_tick();}
    serial::line("H5K: TIMER_IRQ_OK");
    serial::line("H5K: KERNEL_READY");
    let mut packet=[0u8;3];let mut pos=0;let mut down=false;let mut ext=false;
    loop{
        match ui.tasks.dispatch(){
            0=>{for _ in 0..32{let st=p::input(0x64);if st&1==0{break;}let b=p::input(0x60);if st&0x20!=0{
                if pos==0&&b&8==0{continue;}packet[pos]=b;pos+=1;if pos==3{pos=0;let flags=packet[0];let dx=packet[1] as i8 as isize;let dy=packet[2] as i8 as isize;ui.cursor_hide();ui.mouse=true;ui.mx=(ui.mx as isize+dx).clamp(0,fb.width as isize-1)as usize;ui.my=(ui.my as isize-dy).clamp(0,fb.height as isize-1)as usize;let click=flags&1!=0&&!down;down=flags&1!=0;if click{ui.click();}else{ui.cursor_show();}}
            }else if b==0xe0{ext=true;}else{if !ext&&b&0x80==0{ui.key(b);}ext=false;}}}
            1=>{core::hint::spin_loop();} // file service slot; UI commands update RAM file
            _=>{p::wait_tick();} // timer-paced idle/housekeeping slot
        }
    }
}
