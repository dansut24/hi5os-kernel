//! Hi5Desktop v0.0.9 — native software GUI; single foreground window.
use crate::{boot::Framebuffer,ui::{Canvas,Color},platform as p,services::{Files,Tasks},serial};
use core::ptr;
const BLUE:Color=[34,112,195];const WHITE:Color=[255,255,255];const GOLD:Color=[234,178,66];
#[derive(Clone,Copy)]struct Palette{wall:Color,wall2:Color,bar:Color,panel:Color,sub:Color,ink:Color,muted:Color,edge:Color,green:Color}
fn palette(dark:bool)->Palette{if dark{Palette{wall:[19,39,61],wall2:[32,65,92],bar:[27,42,58],panel:[34,50,67],sub:[45,63,81],ink:[231,240,247],muted:[169,189,207],edge:[58,80,102],green:[113,221,166]}}else{Palette{wall:[211,229,244],wall2:[175,205,232],bar:[244,248,252],panel:[255,255,255],sub:[238,244,249],ink:[27,43,61],muted:[83,105,127],edge:[204,218,230],green:[23,128,83]}}}
#[derive(Clone,Copy,PartialEq)]enum App{System,Files,Network,Terminal}
impl App{fn title(self)->&'static str{match self{Self::System=>"Settings",Self::Files=>"File Explorer",Self::Network=>"Network",Self::Terminal=>"Terminal"}}}
const APPS:[App;4]=[App::System,App::Files,App::Network,App::Terminal];
#[derive(Clone,Copy)]struct Rect{x:usize,y:usize,w:usize,h:usize}
impl Rect{fn hit(self,x:usize,y:usize)->bool{x>=self.x&&x<self.x+self.w&&y>=self.y&&y<self.y+self.h}}
struct Terminal{buf:[u8;48],len:usize,last:[u8;48],last_len:usize,result:u8,shift:bool}
impl Terminal{fn new()->Self{Self{buf:[0;48],len:0,last:[0;48],last_len:0,result:0,shift:false}}}
struct Ui{c:Canvas,app:Option<App>,minimized:bool,menu:bool,dark:bool,ram:u64,nics:u32,files:Files,tasks:Tasks,mx:usize,my:usize,mouse:bool,saved:[u32;256],saved_valid:bool,win:Rect,restore:Rect,maximized:bool,drag:Option<(usize,usize)>,term:Terminal}
impl Ui{
    fn cursor_hide(&mut self){if !self.saved_valid{return;}for y in 0..16{for x in 0..16{if self.mx+x<self.c.fb.width&&self.my+y<self.c.fb.height{unsafe{ptr::write_volatile((self.c.fb.base as *mut u32).add((self.my+y)*self.c.fb.stride+self.mx+x),self.saved[y*16+x]);}}}}self.saved_valid=false;}
    fn cursor_show(&mut self){if !self.mouse{return;}for y in 0..16{for x in 0..16{if self.mx+x<self.c.fb.width&&self.my+y<self.c.fb.height{self.saved[y*16+x]=unsafe{ptr::read_volatile((self.c.fb.base as *const u32).add((self.my+y)*self.c.fb.stride+self.mx+x))};if x<=y/2&&y<14{self.c.front_pixel(self.mx+x,self.my+y,if x==0||x==y/2||y==13{[22,32,43]}else{WHITE});}}}}self.saved_valid=true;}
    fn bar_y(&self)->usize{self.c.fb.height.saturating_sub(52)}
    fn dock_x(&self)->usize{self.c.fb.width.saturating_sub(310)/2}
    fn menu_rect(&self)->Rect{Rect{x:8,y:self.bar_y().saturating_sub(278),w:270,h:272}}
    fn button(&self,r:Rect,t:&str,fill:Color,ink:Color){self.c.round(r.x,r.y,r.w,r.h,6,fill);self.c.text(r.x+9,r.y+5,t,ink);}
    fn icon(&self,a:App,x:usize,y:usize){let c=&self.c;let pal=palette(self.dark);
        match a{
            App::System=>{c.round(x,y,34,26,5,BLUE);c.round(x+4,y+4,26,16,2,[211,232,251]);c.rect(x+15,y+26,4,5,pal.ink);c.rect(x+8,y+31,18,2,pal.ink);}
            App::Files=>{c.round(x,y+7,34,25,4,GOLD);c.round(x+2,y+3,15,10,3,GOLD);c.rect(x+3,y+13,28,3,[255,221,142]);}
            App::Network=>{c.round(x,y,34,34,9,BLUE);c.rect(x+5,y+16,24,2,WHITE);c.rect(x+16,y+5,2,24,WHITE);c.round(x+8,y+8,18,18,5,[109,177,231]);c.rect(x+16,y+5,2,24,WHITE);}
            App::Terminal=>{c.round(x,y,34,30,5,[30,44,58]);c.text(x+5,y+4,">_",WHITE);}
        }
    }
    fn draw(&mut self){
        self.saved_valid=false;
        let c=&self.c;let w=c.fb.width;let h=c.fb.height;let q=palette(self.dark);let by=self.bar_y();
        // Quiet, low-cost gradient and a native desktop with standard controls.
        for y in 0..by{let mix=|i:usize|((q.wall[i]as usize*(by-y)+q.wall2[i]as usize*y)/by.max(1))as u8;c.rect(0,y,w,1,[mix(0),mix(1),mix(2)]);}
        c.title(w.saturating_sub(145),20,"Hi5OS",q.ink);
        c.text(w.saturating_sub(202),54,"Developer preview 0.0.9",q.muted);
        for(i,a)in APPS.iter().enumerate(){let y=18+i*76;self.icon(*a,31,y);let label=match a{App::System=>"This PC",App::Files=>"Files",App::Network=>"Network",App::Terminal=>"Terminal"};c.text(18,y+35,label,q.ink);}
        if let Some(a)=self.app{if !self.minimized{self.draw_window(a,q);}}
        c.rect(0,by,w,h-by,q.bar);c.rect(0,by,w,1,q.edge);
        self.button(Rect{x:8,y:by+8,w:78,h:36},"Start",BLUE,WHITE);
        let dock=self.dock_x();
        for(i,a)in APPS.iter().enumerate(){let x=dock+i*50;if self.app==Some(*a){c.round(x,by+5,46,42,7,q.sub);c.rect(x+14,by+47,18,3,BLUE);}self.icon(*a,x+6,by+10);}
        c.text(w.saturating_sub(100),by+6,"Offline",q.muted);c.text(w.saturating_sub(100),by+25,"Hi5Kernel",q.ink);
        if self.menu{let r=self.menu_rect();c.round(r.x+3,r.y+4,r.w,r.h,12,q.edge);c.round(r.x,r.y,r.w,r.h,12,q.panel);c.title(r.x+16,r.y+10,"Start",q.ink);
            for(i,a)in APPS.iter().enumerate(){let y=r.y+49+i*40;self.icon(*a,r.x+14,y);c.text(r.x+60,y+6,a.title(),q.ink);}
            self.button(Rect{x:r.x+12,y:r.y+218,w:r.w-24,h:38},if self.dark{"Light appearance (T)"}else{"Dark appearance (T)"},q.sub,q.ink);
        }
        c.present();self.cursor_show();
    }
    fn draw_window(&self,a:App,q:Palette){
        let c=&self.c;let r=self.win;
        c.round(r.x+4,r.y+6,r.w,r.h,11,q.edge);c.round(r.x,r.y,r.w,r.h,10,q.panel);
        c.round(r.x,r.y,r.w,42,10,q.bar);c.rect(r.x,r.y+35,r.w,8,q.bar);c.rect(r.x,r.y+42,r.w,1,q.edge);
        c.text(r.x+14,r.y+11,a.title(),q.ink);
        self.button(Rect{x:r.x+r.w-108,y:r.y+5,w:30,h:30},"-",q.sub,q.ink);
        self.button(Rect{x:r.x+r.w-73,y:r.y+5,w:30,h:30},"+",q.sub,q.ink);
        self.button(Rect{x:r.x+r.w-38,y:r.y+5,w:30,h:30},"x",q.sub,q.ink);
        let x=r.x+22;let y=r.y+58;let right=r.x+r.w-14;
        match a{
            App::System=>{
                c.title(x,y,"About this PC",q.ink);c.round(x,y+42,r.w-44,76,9,q.sub);
                c.text(x+14,y+50,"Hi5OS Developer Preview",q.ink);c.text(x+14,y+77,"Independent x86-64 kernel",q.muted);
                c.text(x,y+135,"Managed memory (MiB)",q.muted);c.num(x+200,y+135,self.ram,q.ink);
                c.text(x,y+163,"Kernel",q.muted);c.text(x+200,y+163,"Hi5Kernel 0.0.9",q.ink);
                c.text(x,y+191,"Paging and exception checks passed",q.green);
                self.button(Rect{x,y:y+223,w:180,h:34},if self.dark{"Use light theme"}else{"Use dark theme"},q.sub,q.ink);
            }
            App::Files=>{
                c.title(x,y,"RAM workspace",q.ink);
                self.button(Rect{x,y:y+43,w:106,h:34},"New note",BLUE,WHITE);
                self.button(Rect{x:x+116,y:y+43,w:90,h:34},"Delete",q.sub,q.ink);
                c.round(x,y+90,r.w-44,104,8,q.sub);
                if let Some(b)=self.files.read(){self.icon(App::Files,x+12,y+102);c.text(x+60,y+103,"notes.txt",q.ink);if let Ok(t)=core::str::from_utf8(b){c.text_limit(x+60,y+132,t,q.muted,false,right-6);}}else{c.text(x+15,y+110,"This workspace is empty",q.muted);}
                c.text_limit(x,y+211,"Volatile storage - not saved after reboot.",q.muted,false,right);
                c.text(x,y+239,"N: New note   D: Delete",q.muted);
            }
            App::Network=>{
                c.title(x,y,"Network status",q.ink);c.round(x,y+44,r.w-44,91,8,q.sub);c.text(x+16,y+55,"Offline",q.ink);
                c.text(x+16,y+83,"Detected PCI network devices:",q.muted);c.num(x+260,y+83,self.nics,q.ink);
                c.text_limit(x,y+155,"Hardware discovery only. No network driver or IP.",q.muted,false,right);
            }
            App::Terminal=>{
                c.rect(r.x+8,r.y+49,r.w-16,r.h-57,[22,32,43]);let fg=[220,234,247];let green=[112,220,164];
                c.text_limit(x,y,"Hi5Shell - native diagnostic console",fg,false,right);
                c.text_limit(x,y+29,"Type help for available commands.",fg,false,right);
                if self.term.last_len>0{c.text(x,y+63,">",green);if let Ok(t)=core::str::from_utf8(&self.term.last[..self.term.last_len]){c.text_limit(x+16,y+63,t,fg,false,right);}}
                match self.term.result{
                    1=>{c.text_limit(x,y+91,"help  mem  ver  ls  clear  theme",fg,false,right);}
                    2=>{c.text(x,y+91,"Managed MiB:",fg);c.num(x+126,y+91,self.ram,green);}
                    3=>c.text(x,y+91,"Hi5OS 0.0.9",fg),
                    4=>c.text(x,y+91,if self.files.present{"notes.txt (RAM)"}else{"Empty RAM workspace"},fg),
                    5=>c.text(x,y+91,"Appearance changed",fg),
                    6=>c.text(x,y+91,"Unknown command",fg),_=>{}
                }
                c.text(x,y+133,">",green);
                if let Ok(t)=core::str::from_utf8(&self.term.buf[..self.term.len]){c.text_limit(x+16,y+133,t,fg,false,right);}
                c.text(x,y+174,"Esc: close terminal",fg);
            }
        }
    }
    fn select(&mut self,a:App){self.app=Some(a);self.minimized=false;self.menu=false;serial::line(match a{App::System=>"H5K: GUI_SYSTEM_OPEN",App::Files=>"H5K: GUI_FILES_OPEN",App::Network=>"H5K: GUI_NETWORK_OPEN",App::Terminal=>"H5K: GUI_TERMINAL_OPEN"});self.draw();}
    fn theme(&mut self){self.dark=!self.dark;serial::line(if self.dark{"H5K: GUI_THEME_DARK"}else{"H5K: GUI_THEME_LIGHT"});self.draw();}
    fn new_file(&mut self){self.files.write(b"New Hi5OS note");serial::line("H5K: GUI_FILE_CREATED");self.draw();}
    fn delete_file(&mut self){self.files.remove();serial::line("H5K: GUI_FILE_DELETED");self.draw();}
    fn close(&mut self){self.app=None;self.minimized=false;self.menu=false;self.drag=None;serial::line("H5K: GUI_HOME");self.draw();}
    fn maximize(&mut self){if self.maximized{self.win=self.restore;}else{self.restore=self.win;self.win=Rect{x:6,y:6,w:self.c.fb.width.saturating_sub(12),h:self.bar_y().saturating_sub(12)};}self.maximized=!self.maximized;serial::line(if self.maximized{"H5K: GUI_MAXIMIZED"}else{"H5K: GUI_RESTORED"});self.draw();}
    fn terminal_key(&mut self,key:u8){
        match key{
            0x1c=>{
                let cmd=&self.term.buf[..self.term.len];let result=match cmd{b"help"=>1,b"mem"=>2,b"ver"=>3,b"ls"=>4,b"theme"=>5,b"clear"=>0,_=>6};
                self.term.last=self.term.buf;self.term.last_len=if result==0{0}else{self.term.len};self.term.len=0;self.term.result=result;
                serial::line(match result{1=>"H5K: SHELL_HELP_OK",2=>"H5K: SHELL_MEM_OK",3=>"H5K: SHELL_VER_OK",4=>"H5K: SHELL_LS_OK",5=>"H5K: SHELL_THEME_OK",0=>"H5K: SHELL_CLEAR_OK",_=>"H5K: SHELL_UNKNOWN"});
                if result==5{self.theme();}else{self.draw();}
            }
            0x0e=>{self.term.len=self.term.len.saturating_sub(1);self.draw();}
            _=>{if let Some(b)=ascii(key,self.term.shift){if self.term.len<self.term.buf.len(){self.term.buf[self.term.len]=b;self.term.len+=1;self.draw();}}}
        }
    }
    fn key(&mut self,key:u8){
        match key{0x2a|0x36=>{self.term.shift=true;return;},0xaa|0xb6=>{self.term.shift=false;return;},_=>{}}
        if key&0x80!=0{return;}
        if key==0x01{if self.menu{self.menu=false;self.draw();}else{self.close();}return;}
        if key>=0x3b&&key<=0x3e{self.select(APPS[(key-0x3b)as usize]);return;}
        if self.app==Some(App::Terminal)&&!self.menu&&!self.minimized{self.terminal_key(key);return;}
        match key{
            0x02..=0x05=>self.select(APPS[(key-2)as usize]),0x32=>{self.menu=!self.menu;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();},
            0x14=>self.theme(),0x2d if self.app.is_some()=>self.maximize(),
            0x31 if self.app==Some(App::Files)=>self.new_file(),0x20 if self.app==Some(App::Files)=>self.delete_file(),_=>{}
        }
    }
    fn click(&mut self){
        let x=self.mx;let y=self.my;let by=self.bar_y();let dock=self.dock_x();
        if y>=by{if x<92{self.menu=!self.menu;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();return;}if x>=dock&&x<dock+200{let a=APPS[(x-dock)/50];if self.app==Some(a)&&!self.minimized{self.minimized=true;self.draw();}else{self.select(a);}return;}}
        if self.menu{let m=self.menu_rect();if m.hit(x,y){if y>=m.y+49&&y<m.y+209{self.select(APPS[(y-m.y-49)/40]);}else if y>=m.y+218{self.menu=false;self.theme();}return;}self.menu=false;self.draw();}
        if let Some(a)=self.app{let r=self.win;if !self.minimized&&r.hit(x,y){
            if y<r.y+42{if x>=r.x+r.w-38{self.close();}else if x>=r.x+r.w-73{self.maximize();}else if x>=r.x+r.w-108{self.minimized=true;self.draw();}else if !self.maximized{self.drag=Some((x-r.x,y-r.y));}return;}
            let bx=r.x+22;let yy=r.y+58;
            if a==App::Files&&y>=yy+43&&y<yy+77{if x>=bx&&x<bx+106{self.new_file();}else if x>=bx+116&&x<bx+206{self.delete_file();}}
            if a==App::System&&x>=bx&&x<bx+180&&y>=yy+223&&y<yy+257{self.theme();}return;
        }}
        if x<100&&y>=18&&y<322{let i=(y-18)/76;if i<4{self.select(APPS[i]);}}
    }
    fn move_pointer(&mut self,dx:isize,dy:isize,pressed:bool,was:bool){
        self.cursor_hide();self.mouse=true;self.mx=(self.mx as isize+dx).clamp(0,self.c.fb.width as isize-1)as usize;self.my=(self.my as isize-dy).clamp(0,self.c.fb.height as isize-1)as usize;
        if pressed&&!was{self.click();}
        if !pressed&&was{if let Some((ox,oy))=self.drag.take(){self.win.x=self.mx.saturating_sub(ox).min(self.c.fb.width.saturating_sub(self.win.w));self.win.y=self.my.saturating_sub(oy).min(self.bar_y().saturating_sub(self.win.h));serial::line("H5K: GUI_WINDOW_MOVED");self.draw();return;}}
        if !self.saved_valid{self.cursor_show();}
    }
}
fn ascii(key:u8,shift:bool)->Option<u8>{
    let mut b=match key{0x02..=0x0b=>b"1234567890"[(key-2)as usize],0x10..=0x19=>b"qwertyuiop"[(key-0x10)as usize],0x1e..=0x26=>b"asdfghjkl"[(key-0x1e)as usize],0x2c..=0x32=>b"zxcvbnm"[(key-0x2c)as usize],0x39=>b' ',0x0c=>b'-',0x0d=>b'=',0x33=>b',',0x34=>b'.',0x35=>b'/',0x27=>b';',0x28=>b'\'',_=>return None};
    if shift{b=match b{b'a'..=b'z'=>b.to_ascii_uppercase(),b'1'=>b'!',b'2'=>b'"',b'3'=>b'#',b'4'=>b'$',b'5'=>b'%',b'6'=>b'^',b'7'=>b'&',b'8'=>b'*',b'9'=>b'(',b'0'=>b')',b'-'=>b'_',b'='=>b'+',b','=>b'<',b'.'=>b'>',b'/'=>b'?',b';'=>b':',b'\''=>b'@',_=>b};}Some(b)
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
    if fb.width<640||fb.height<480{serial::line("H5K: FAIL DESKTOP_RESOLUTION");crate::kernel::halt();}
    let w=560.min(fb.width.saturating_sub(108));let h=365.min(fb.height.saturating_sub(66));let win=Rect{x:(fb.width-w)/2+20,y:(fb.height-52-h)/2,w,h};
    let mut ui=Ui{c:Canvas::new(fb),app:None,minimized:false,menu:false,dark:false,ram,nics,files,tasks,mx:fb.width/2,my:fb.height/2,mouse:false,saved:[0;256],saved_valid:false,win,restore:win,maximized:false,drag:None,term:Terminal::new()};
    ui.mouse=init_input();serial::line(if ui.mouse{"H5K: PS2_MOUSE_ACK"}else{"H5K: PS2_MOUSE_UNAVAILABLE"});
    serial::line(if ui.c.buffered{"H5K: DOUBLE_BUFFER_OK"}else{"H5K: DIRECT_RENDER_FALLBACK"});
    ui.draw();serial::line("H5K: FRAMEBUFFER_OK");serial::line("H5K: DESKTOP_READY");
    p::timer_init();let start=p::ticks();while p::ticks().wrapping_sub(start)<3{p::wait_tick();}
    serial::line("H5K: TIMER_IRQ_OK");serial::line("H5K: KERNEL_READY");
    let mut packet=[0u8;3];let mut pos=0;let mut down=false;let mut ext=false;
    loop{match ui.tasks.dispatch(){
        0=>{for _ in 0..32{let st=p::input(0x64);if st&1==0{break;}let b=p::input(0x60);if st&0x20!=0{
            if pos==0&&b&8==0{continue;}packet[pos]=b;pos+=1;if pos==3{pos=0;let flags=packet[0];if flags&0xc0!=0{continue;}let dx=packet[1]as isize-if flags&0x10!=0{256}else{0};let dy=packet[2]as isize-if flags&0x20!=0{256}else{0};let pressed=flags&1!=0;ui.move_pointer(dx,dy,pressed,down);down=pressed;}
        }else if b==0xe0{ext=true;}else{if !ext{ui.key(b);}ext=false;}}}
        1=>core::hint::spin_loop(),_=>p::wait_tick(),
    }}
}
