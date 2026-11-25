//! Hi5Desktop v0.2.3 — native software GUI; single foreground window.
use crate::{boot::Framebuffer,ui::{Canvas,Color},platform as p,services::{Files,Tasks},serial,disk::{Store,CAP,FILE_COUNT,NAMES}};
use core::ptr;
use crate::net::{Nic,MAX_RESPONSE};
const BLUE:Color=[34,112,195];const WHITE:Color=[255,255,255];const GOLD:Color=[234,178,66];
#[derive(Clone,Copy)]struct Palette{wall:Color,wall2:Color,bar:Color,panel:Color,sub:Color,ink:Color,muted:Color,edge:Color,green:Color}
fn palette(dark:bool)->Palette{if dark{Palette{wall:[19,39,61],wall2:[32,65,92],bar:[27,42,58],panel:[34,50,67],sub:[45,63,81],ink:[231,240,247],muted:[169,189,207],edge:[58,80,102],green:[113,221,166]}}else{Palette{wall:[211,229,244],wall2:[175,205,232],bar:[244,248,252],panel:[255,255,255],sub:[238,244,249],ink:[27,43,61],muted:[83,105,127],edge:[204,218,230],green:[23,128,83]}}}
#[derive(Clone,Copy,PartialEq)]enum App{System,Files,Network,Terminal,Notepad,Browser,Sheet}
impl App{fn title(self)->&'static str{match self{Self::System=>"Settings",Self::Files=>"File Explorer",Self::Network=>"Network",Self::Terminal=>"Terminal",Self::Notepad=>"Notepad",Self::Browser=>"Hi5Browser",Self::Sheet=>"Hi5Sheet"}}}
const APPS:[App;6]=[App::System,App::Files,App::Network,App::Terminal,App::Notepad,App::Browser];
#[derive(Clone,Copy)]struct Rect{x:usize,y:usize,w:usize,h:usize}
impl Rect{fn hit(self,x:usize,y:usize)->bool{x>=self.x&&x<self.x+self.w&&y>=self.y&&y<self.y+self.h}}
struct Terminal{buf:[u8;48],len:usize,last:[u8;48],last_len:usize,result:u8,shift:bool}
impl Terminal{fn new()->Self{Self{buf:[0;48],len:0,last:[0;48],last_len:0,result:0,shift:false}}}
#[derive(Clone,Copy)]struct FileSlot{data:[u8;CAP],len:usize,present:bool}
const EMPTY_FILE:FileSlot=FileSlot{data:[0;CAP],len:0,present:false};
// The large response buffer is image-owned static memory, not the 64 KiB kernel stack.
static mut WEB_BUFFER:[u8;MAX_RESPONSE]=[0;MAX_RESPONSE];
static mut APP_BUFFER:[u8;crate::trial::PACKAGE_SIZE]=[0;crate::trial::PACKAGE_SIZE];
struct Ui{c:Canvas,app:Option<App>,minimized:bool,menu:bool,dark:bool,ram:u64,nics:u32,files:Files,tasks:Tasks,mx:usize,my:usize,mouse:bool,saved:[u32;256],saved_valid:bool,win:Rect,restore:Rect,maximized:bool,drag:Option<(usize,usize)>,term:Terminal,sheet:crate::sheet::Sheet,sheet_result:i64,sheet_cell:usize,sheet_edit:[u8;48],sheet_len:usize,sheet_msg:&'static str,pe_report:crate::imports::Report,rebase:crate::relocate::Report,store:Store,note:[u8;CAP],note_len:usize,dirty:bool,save_status:u8,ctrl:bool,file:usize,cache:[FileSlot;FILE_COUNT],nic:Option<Nic>,url:[u8;128],url_len:usize,web:&'static mut [u8;MAX_RESPONSE],source:[u8;128],source_len:usize,web_len:usize,web_ok:bool,web_msg:&'static str}
impl Ui{
    fn cursor_hide(&mut self){if !self.saved_valid{return;}for y in 0..16{for x in 0..16{if self.mx+x<self.c.fb.width&&self.my+y<self.c.fb.height{unsafe{ptr::write_volatile((self.c.fb.base as *mut u32).add((self.my+y)*self.c.fb.stride+self.mx+x),self.saved[y*16+x]);}}}}self.saved_valid=false;}
    fn cursor_show(&mut self){if !self.mouse{return;}for y in 0..16{for x in 0..16{if self.mx+x<self.c.fb.width&&self.my+y<self.c.fb.height{self.saved[y*16+x]=unsafe{ptr::read_volatile((self.c.fb.base as *const u32).add((self.my+y)*self.c.fb.stride+self.mx+x))};if x<=y/2&&y<14{self.c.front_pixel(self.mx+x,self.my+y,if x==0||x==y/2||y==13{[22,32,43]}else{WHITE});}}}}self.saved_valid=true;}
    fn bar_y(&self)->usize{self.c.fb.height.saturating_sub(52)}
    fn dock_x(&self)->usize{self.c.fb.width.saturating_sub(300)/2}
    fn menu_rect(&self)->Rect{Rect{x:8,y:self.bar_y().saturating_sub(358),w:270,h:352}}
    fn button(&self,r:Rect,t:&str,fill:Color,ink:Color){self.c.round(r.x,r.y,r.w,r.h,6,fill);self.c.text(r.x+9,r.y+5,t,ink);}
    fn icon(&self,a:App,x:usize,y:usize){let c=&self.c;let pal=palette(self.dark);
        match a{
            App::System=>{c.round(x,y,34,26,5,BLUE);c.round(x+4,y+4,26,16,2,[211,232,251]);c.rect(x+15,y+26,4,5,pal.ink);c.rect(x+8,y+31,18,2,pal.ink);}
            App::Files=>{c.round(x,y+7,34,25,4,GOLD);c.round(x+2,y+3,15,10,3,GOLD);c.rect(x+3,y+13,28,3,[255,221,142]);}
            App::Network=>{c.round(x,y,34,34,9,BLUE);c.rect(x+5,y+16,24,2,WHITE);c.rect(x+16,y+5,2,24,WHITE);c.round(x+8,y+8,18,18,5,[109,177,231]);c.rect(x+16,y+5,2,24,WHITE);}
            App::Terminal=>{c.round(x,y,34,30,5,[30,44,58]);c.text(x+5,y+4,">_",WHITE);}
            App::Notepad=>{c.round(x+4,y,26,34,4,[250,251,253]);for i in 0..4{c.rect(x+9,y+8+i*5,16,2,BLUE);}}
            App::Browser=>{c.round(x,y,34,34,15,[34,142,155]);c.rect(x+4,y+16,26,2,WHITE);c.rect(x+16,y+4,2,26,WHITE);}
            App::Sheet=>{c.round(x,y,34,34,5,[25,135,84]);for i in 0..3{c.rect(x+5,y+7+i*9,24,1,WHITE);c.rect(x+7+i*9,y+5,1,24,WHITE);}}
        }
    }
    fn draw(&mut self){
        self.saved_valid=false;
        let c=&self.c;let w=c.fb.width;let h=c.fb.height;let q=palette(self.dark);let by=self.bar_y();
        // Quiet, low-cost gradient and a native desktop with standard controls.
        for y in 0..by{let mix=|i:usize|((q.wall[i]as usize*(by-y)+q.wall2[i]as usize*y)/by.max(1))as u8;c.rect(0,y,w,1,[mix(0),mix(1),mix(2)]);}
        c.title(w.saturating_sub(145),20,"Hi5OS",q.ink);
        c.text(w.saturating_sub(202),54,concat!("Developer preview ",env!("CARGO_PKG_VERSION")),q.muted);
        for(i,a)in APPS.iter().enumerate(){let y=8+i*61;self.icon(*a,31,y);let label=match a{App::System=>"This PC",App::Files=>"Files",App::Network=>"Network",App::Terminal=>"Terminal",App::Notepad=>"Notepad",App::Browser=>"Browser",App::Sheet=>"Sheet"};c.text(18,y+35,label,q.ink);}
        if by>438{self.icon(App::Sheet,31,374);c.text(18,409,"Sheet",q.ink);}
        if let Some(a)=self.app{if !self.minimized{self.draw_window(a,q);}}
        c.rect(0,by,w,h-by,q.bar);c.rect(0,by,w,1,q.edge);
        self.button(Rect{x:8,y:by+8,w:78,h:36},"Start",BLUE,WHITE);
        let dock=self.dock_x();
        for(i,a)in APPS.iter().enumerate(){let x=dock+i*50;if self.app==Some(*a){c.round(x,by+5,46,42,7,q.sub);c.rect(x+14,by+47,18,3,BLUE);}self.icon(*a,x+6,by+10);}
        c.text(w.saturating_sub(100),by+6,if self.nic.as_ref().map_or(false,|n|n.config().0!=[0;4]){"IPv4 ready"}else{"Offline"},q.muted);c.text(w.saturating_sub(100),by+25,"Hi5Kernel",q.ink);
        if self.menu{let r=self.menu_rect();c.round(r.x+3,r.y+4,r.w,r.h,12,q.edge);c.round(r.x,r.y,r.w,r.h,12,q.panel);c.title(r.x+16,r.y+10,"Start",q.ink);
            for(i,a)in APPS.iter().enumerate(){let y=r.y+49+i*40;self.icon(*a,r.x+14,y);c.text(r.x+60,y+6,a.title(),q.ink);}
            self.button(Rect{x:r.x+12,y:r.y+298,w:r.w-24,h:38},if self.dark{"Light appearance (T)"}else{"Dark appearance (T)"},q.sub,q.ink);
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
                c.text(x,y+163,"Kernel",q.muted);c.text(x+200,y+163,concat!("Hi5Kernel ",env!("CARGO_PKG_VERSION")),q.ink);
                c.text(x,y+191,"PE and compound-file probes passed",q.green);
                self.button(Rect{x,y:y+223,w:180,h:34},if self.dark{"Use light theme"}else{"Use dark theme"},q.sub,q.ink);
            }
            App::Files=>{
                c.title(x,y,if self.store.attached(){"Hi5OS Data"}else{"RAM workspace"},q.ink);
                self.button(Rect{x,y:y+43,w:106,h:34},"New note",BLUE,WHITE);
                self.button(Rect{x:x+116,y:y+43,w:90,h:34},"Delete",q.sub,q.ink);
                for i in 0..FILE_COUNT{
                    let yy=y+88+i*31;
                    c.round(x,yy,r.w-44,29,4,if i==self.file{q.edge}else{q.sub});
                    c.text(x+8,yy+1,NAMES[i],q.ink);
                    c.text(x+156,yy+1,if self.cache[i].present{"Document"}else{"Empty slot"},q.muted);
                }
                c.text_limit(x,y+216,if self.store.attached(){"Data disk attached - four document slots."}else{"RAM only - no persistent data disk."},q.muted,false,right);
                c.text(x,y+242,"F7: Next file   N: New   D: Delete   E: Edit",q.muted);
            }
            App::Network=>{
                c.title(x,y,"Network status",q.ink);c.round(x,y+44,r.w-44,91,8,q.sub);c.text(x+16,y+55,if self.nic.is_some(){"Intel E1000"}else{"Not connected"},q.ink);
                c.text(x+16,y+83,"Detected PCI network devices:",q.muted);c.num(x+260,y+83,self.nics as u64,q.ink);
                if let Some(n)=&self.nic{let (ip,gw,dns)=n.config();for (i,(label,value)) in [("IP address",ip),("Gateway",gw),("DNS server",dns)].iter().enumerate(){let yy=y+148+i*24;c.text(x,yy,label,q.muted);draw_ip(c,x+140,yy,*value,q.ink);}}
                self.button(Rect{x,y:y+229,w:190,h:34},"Connect / DHCP (C)",BLUE,WHITE);
            }
            App::Browser=>{
                c.title(x,y,"Hi5Browser - Web preview",q.ink);
                c.round(x,y+42,r.w-132,34,5,q.sub);
                if let Ok(t)=core::str::from_utf8(&self.url[..self.url_len]){c.text_limit(x+6,y+48,t,q.ink,false,right-90);}
                self.button(Rect{x:r.x+r.w-95,y:y+42,w:72,h:34},"Go",BLUE,WHITE);
                c.text_limit(x,y+84,self.web_msg,q.muted,false,right);
                if self.web_ok{
                    c.text(x,y+112,"Received bytes:",q.ink);c.num(x+132,y+112,self.web_len as u64,q.ink);
                    let kind=match crate::compat::classify(&self.web[..self.web_len]){crate::compat::Kind::Pe64=>"PE64 executable candidate",crate::compat::Kind::Pe32=>"PE32 executable candidate",crate::compat::Kind::Compound=>"OLE compound-file candidate",_=>"Text or unknown binary"};
                    c.text_limit(x,y+137,kind,q.ink,false,right);
                    if self.web[..self.web_len].iter().all(|b|*b==9||*b==10||*b==13||*b>=32)&&core::str::from_utf8(&self.web[..self.web_len]).is_ok(){
                        let mut preview=[0u8;768];let len=crate::http::preview(&self.web[..self.web_len],&mut preview);
                        let mut yy=y+163;for line in preview[..len].split(|b|*b==10){for part in line.chunks(45){if yy>=y+223{break;}if let Ok(t)=core::str::from_utf8(part){c.text_limit(x,yy,t,q.ink,false,right);}yy+=20;}if yy>=y+223{break;}}
                    }else{
                        c.text(x,y+165,"Binary preview - execution disabled",q.muted);
                        let src=&self.source[..self.source_len];let path=src.split(|b|*b==b'?').next().unwrap_or(&[]);
                        let name=path.rsplit(|b|*b==b'/').next().unwrap_or(&[]);
                        if let Ok(t)=core::str::from_utf8(name){if !t.is_empty(){c.text_limit(x,y+188,t,q.ink,false,right);}}
                    }
                    self.button(Rect{x,y:y+229,w:180,h:34},"Save download (F9)",q.sub,q.ink);
                }else{c.text_limit(x,y+120,"HTTP only. Preview 4 MiB; save 1 MiB.",q.ink,false,right);c.text(x,y+149,"Ctrl+L clears URL; F10 opens last download.",q.muted);}
            }
            App::Notepad=>{
                self.button(Rect{x,y:y+1,w:82,h:33},"Save",BLUE,WHITE);
                self.button(Rect{x:x+92,y:y+1,w:82,h:33},"Clear",q.sub,q.ink);
                c.text_limit(x+186,y+6,if self.save_status==2{"RAM only / save failed"}else if self.dirty{"Unsaved changes"}else if self.save_status==1{"Saved to data disk"}else{NAMES[self.file]},q.muted,false,right);
                c.round(x,y+44,r.w-44,r.h.saturating_sub(162),6,q.sub);
                let mut yy=y+52;
                for line in self.note[..self.note_len].split(|b|*b==10){
                    if line.is_empty(){yy+=20;continue;}
                    for part in line.chunks(38){if yy>=r.y+r.h-48{break;}if let Ok(t)=core::str::from_utf8(part){c.text_limit(x+8,yy,t,q.ink,false,right-8);}yy+=20;}
                    if yy>=r.y+r.h-48{break;}
                }
                c.text_limit(x,r.y+r.h-33,"F6 / Ctrl+S save - F7 next file - Esc close",q.muted,false,right);
            }
            App::Sheet=>{
                c.title(x,y,"Hi5Sheet - workbook",q.ink);
                let name=crate::sheet::Sheet::cell_name(self.sheet_cell);let label=core::str::from_utf8(&name).unwrap_or("a1");
                c.text(x,y+36,label,q.ink);c.round(x+44,y+32,r.w-88,30,4,q.sub);
                if let Ok(t)=core::str::from_utf8(&self.sheet_edit[..self.sheet_len]){c.text_limit(x+50,y+36,t,q.ink,false,right);}
                let cw=(r.w-76)/4;
                for col in 0..4{let b=[b'A'+col as u8];c.text(x+30+col*cw+8,y+67,core::str::from_utf8(&b).unwrap_or(""),q.muted);}
                for row in 0..4{let b=[b'1'+row as u8];c.text(x,y+90+row*30,core::str::from_utf8(&b).unwrap_or(""),q.muted);
                    for col in 0..4{let i=row*4+col;let xx=x+30+col*cw;let yy=y+87+row*30;
                        c.rect(xx,yy,cw-2,28,if i==self.sheet_cell{q.edge}else{q.sub});
                        if let Some(v)=self.sheet.value(i){let mut num=[0u8;20];let mut j=num.len();let mut n=v.unsigned_abs();loop{j-=1;num[j]=b'0'+(n%10)as u8;n/=10;if n==0{break;}}if v<0{j-=1;num[j]=b'-';}
                            if let Ok(t)=core::str::from_utf8(&num[j..]){c.text_limit(xx+4,yy+2,t,q.ink,false,xx+cw-3);}}
                    }}
                c.text_limit(x,y+212,self.sheet_msg,q.muted,false,right);
                c.text_limit(x,y+238,"Tab: next cell   Enter: apply   F11: open",q.muted,false,right);
            }
            App::Terminal=>{
                c.rect(r.x+8,r.y+49,r.w-16,r.h-57,[22,32,43]);let fg=[220,234,247];let green=[112,220,164];
                c.text_limit(x,y,"Hi5Shell - native diagnostic console",fg,false,right);
                c.text_limit(x,y+29,"Type help for available commands.",fg,false,right);
                if self.term.last_len>0{c.text(x,y+63,">",green);if let Ok(t)=core::str::from_utf8(&self.term.last[..self.term.last_len]){c.text_limit(x+16,y+63,t,fg,false,right);}}
                match self.term.result{
                    1=>{c.text_limit(x,y+91,"winfault winconsole install7zip 7za -h",fg,false,right);}
                    2=>{c.text(x,y+91,"Managed MiB:",fg);c.num(x+126,y+91,self.ram,green);}
                    3=>c.text(x,y+91,concat!("Hi5OS ",env!("CARGO_PKG_VERSION")),fg),
                    4=>{for i in 0..FILE_COUNT{if self.cache[i].present{c.text(x+i*110,y+91,NAMES[i],fg);}}},
                    5=>c.text(x,y+91,"Appearance changed",fg),
                    6=>c.text(x,y+91,"Unknown command",fg),7=>c.text_limit(x,y+91,"PE32/PE64 + OLE probe only; no execution",fg,false,right),
                    8=>{c.text_limit(x,y+91,crate::security::status_text(),fg,false,right);if let Some(t)=crate::security::now(){c.text(x,y+111,"UTC epoch:",fg);c.num(x+105,y+111,t,green);}else{c.text(x,y+111,"Clock unavailable",fg);}},
                    9=>{c.text_limit(x,y+91,"Hi5Sheet: set a1 12 / get a1 / SUM",fg,false,right);c.text_limit(x,y+111,"F11: graphical sheet - RAM only",fg,false,right);},
                    10=>{c.text(x,y+91,"Cell value:",fg);let v=self.sheet_result;if v<0{c.text(x+105,y+91,"-",green);}c.num(x+119,y+91,v.unsigned_abs(),green);},
                    11=>c.text_limit(x,y+91,"Invalid cell, formula, cycle or overflow",fg,false,right),
                    12=>{c.text_limit(x,y+91,"Windows PE64 fixture exited successfully",fg,false,right);c.text(x,y+111,"ExitProcess result: 42",green);},
                    13=>c.text_limit(x,y+91,"Windows PE fixture failed",fg,false,right),
                    14=>{c.text(x,y+91,"Imported DLLs:",fg);c.num(x+150,y+91,self.pe_report.dlls as u64,green);c.text(x+230,y+91,"Functions:",fg);c.num(x+330,y+91,self.pe_report.functions as u64,green);c.text_limit(x,y+111,"Read-only inventory; external execution blocked",fg,false,right);},
                    15=>c.text_limit(x,y+91,"No valid PE64 download or unsupported imports",fg,false,right),
                    16=>{c.text_limit(x,y+91,"Four Windows API calls verified",fg,false,right);c.text(x,y+111,"Runtime exit code: 42",green);},
                    17=>c.text_limit(x,y+91,"Windows runtime test failed",fg,false,right),
                    18=>{c.text(x,y+91,"DIR64 relocations:",fg);c.num(x+185,y+91,self.rebase.count as u64,green);c.text_limit(x,y+111,"Inert image mapped; execution disabled",fg,false,right);},
                    19=>c.text_limit(x,y+91,"No supported relocatable PE64 download",fg,false,right),
                    20=>{let mut b=[0u8;128];let n=crate::platform::console_output(&mut b);if let Ok(t)=core::str::from_utf8(&b[..n]){c.text_limit(x,y+91,t,fg,false,right);}c.text(x,y+111,"Console exit code: 42",green);},
                    21=>c.text_limit(x,y+91,"Windows console test failed",fg,false,right),
                    22=>c.text_limit(x,y+91,"User fault and timeout recovered",green,false,right),23=>c.text_limit(x,y+91,"Trial containment unavailable or failed",fg,false,right),
                    24=>{c.text_limit(x,y+91,if crate::trial::help_ok(){"7-Zip help qualified"}else{"7-Zip startup stopped"},fg,false,right);let mut b=[0u8;192];let n=if crate::trial::help_ok(){crate::trial::output(&mut b)}else{crate::trial::diagnostic(&mut b)};if let Ok(t)=core::str::from_utf8(&b[..n]){for (i,line) in t.lines().take(4).enumerate(){c.text_limit(x,y+111+i*20,line,fg,false,right);}}},
                    25=>c.text_limit(x,y+91,"Trial blocked: verified 7za.exe required",fg,false,right),26=>c.text_limit(x,y+91,"7-Zip portable package installed on data disk",green,false,right),27=>c.text_limit(x,y+91,"Install requires Hi5OS data disk",fg,false,right),_=>{}
                }
                c.text(x,y+133,">",green);
                if let Ok(t)=core::str::from_utf8(&self.term.buf[..self.term.len]){c.text_limit(x+16,y+133,t,fg,false,right);}
                c.text(x,y+174,"Esc: close terminal",fg);
            }
        }
    }
    fn select(&mut self,a:App){if a==App::Notepad&&!self.dirty{self.load_note();}self.app=Some(a);self.minimized=false;self.menu=false;serial::line(match a{App::System=>"H5K: GUI_SYSTEM_OPEN",App::Files=>"H5K: GUI_FILES_OPEN",App::Network=>"H5K: GUI_NETWORK_OPEN",App::Terminal=>"H5K: GUI_TERMINAL_OPEN",App::Notepad=>"H5K: GUI_NOTEPAD_OPEN",App::Browser=>"H5K: GUI_BROWSER_OPEN",App::Sheet=>"H5K: GUI_SHEET_OPEN"});self.draw();}
    fn theme(&mut self){self.dark=!self.dark;serial::line(if self.dark{"H5K: GUI_THEME_DARK"}else{"H5K: GUI_THEME_LIGHT"});self.draw();}
    fn cache_current(&mut self){let s=&mut self.cache[self.file];s.data=self.files.data;s.len=self.files.len;s.present=self.files.present;}
    fn switch_file(&mut self,i:usize){
        if i>=FILE_COUNT||i==self.file{return;}
        if self.dirty{serial::line("H5K: FILE_SWITCH_UNSAVED");return;}
        self.cache_current();self.file=i;
        if self.store.attached(){let _=self.store.load(i);}
        let s=self.cache[i];self.files.data=s.data;self.files.len=s.len;self.files.present=s.present;
        self.load_note();self.save_status=0;serial::metric("H5K: GUI_FILE_SELECTED=",i as u64);self.draw();
    }
    fn load_note(&mut self){self.note.fill(0);self.note_len=0;if let Some(b)=self.files.read(){self.note[..b.len()].copy_from_slice(b);self.note_len=b.len();}}
    fn save_note(&mut self){if self.files.write(&self.note[..self.note_len]){
        self.cache_current();if self.store.save(true,&self.note[..self.note_len]){self.save_status=1;self.dirty=false;serial::line("H5K: GUI_NOTE_SAVED");}
        else{self.save_status=2;self.dirty=false;serial::line("H5K: GUI_NOTE_RAM_ONLY");}
    }self.draw();}
    fn clear_note(&mut self){self.note.fill(0);self.note_len=0;self.dirty=true;self.save_status=0;self.draw();}
    fn notepad_key(&mut self,key:u8){if key==0x40||(self.ctrl&&key==0x1f){self.save_note();return;}if self.ctrl{return;}
        if key==0x0e{self.note_len=self.note_len.saturating_sub(1);self.dirty=true;self.save_status=0;self.draw();return;}
        let b=if key==0x1c{Some(10)}else{ascii(key,self.term.shift)};
        if let Some(b)=b{if self.note_len<CAP{self.note[self.note_len]=b;self.note_len+=1;self.dirty=true;self.save_status=0;self.draw();}}
    }
    fn new_file(&mut self){self.files.write(b"New Hi5OS note");self.cache_current();if self.store.attached(){self.store.save(true,b"New Hi5OS note");}self.dirty=false;self.save_status=0;serial::line("H5K: GUI_FILE_CREATED");self.draw();}
    fn delete_file(&mut self){self.files.remove();self.cache_current();if self.store.attached(){self.store.save(false,&[]);}self.dirty=false;self.note_len=0;self.save_status=0;serial::line("H5K: GUI_FILE_DELETED");self.draw();}
    fn close(&mut self){self.app=None;self.minimized=false;self.menu=false;self.drag=None;serial::line("H5K: GUI_HOME");self.draw();}
    fn maximize(&mut self){if self.maximized{self.win=self.restore;}else{self.restore=self.win;self.win=Rect{x:6,y:6,w:self.c.fb.width.saturating_sub(12),h:self.bar_y().saturating_sub(12)};}self.maximized=!self.maximized;serial::line(if self.maximized{"H5K: GUI_MAXIMIZED"}else{"H5K: GUI_RESTORED"});self.draw();}
    fn terminal_key(&mut self,key:u8){
        match key{
            0x1c=>{
                let cmd=&self.term.buf[..self.term.len];let result=match cmd{b"help"=>1,b"mem"=>2,b"ver"=>3,b"ls"=>4,b"theme"=>5,b"clear"=>0,b"compat"=>7,b"security"=>8,b"sheet"=>9,b"winhello"=>{if crate::platform::run_pe_fixture(){12}else{13}},b"winfault"=>{if crate::trial::self_test(){22}else{23}},b"install7zip"=>{let src=if self.web_ok&&crate::trial::verified(&self.web[..self.web_len]){&self.web[..self.web_len]}else{crate::trial::BUNDLED};if self.store.install_7zip(src){26}else{27}},b"7za -h"=>{let buf=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};let r=if let Some(n)=self.store.load_7zip(buf){crate::trial::run(&buf[..n])}else if self.web_ok{crate::trial::run(&self.web[..self.web_len])}else{Err("Verified 7za.exe required")};match r{Ok(_)=>24,Err(e)=>{serial::line(e);25}}},b"winconsole"=>{if crate::platform::run_console_fixture(){20}else{21}},b"winruntime"=>{if crate::platform::run_runtime_fixture(){16}else{17}},b"pereloc"=>{if self.web_ok{match crate::relocate::inspect(&self.web[..self.web_len]){Ok(r)=>{self.rebase=r;18},Err(e)=>{serial::line(e);19}}}else{19}},b"pehello"=>{match crate::imports::log(crate::compat::HELLO){Ok(r)=>{self.pe_report=r;14},Err(_)=>15}},b"peinfo"=>{if self.web_ok{match crate::imports::log(&self.web[..self.web_len]){Ok(r)=>{self.pe_report=r;14},Err(e)=>{serial::line(e);15}}}else{15}},_ if cmd.starts_with(b"set ")||cmd.starts_with(b"get ")=>{if let Some(v)=self.sheet.command(cmd){self.sheet_result=v;10}else{11}},_=>6};
                self.term.last=self.term.buf;self.term.last_len=if result==0{0}else{self.term.len};self.term.len=0;self.term.result=result;
                serial::line(match result{1=>"H5K: SHELL_HELP_OK",2=>"H5K: SHELL_MEM_OK",3=>"H5K: SHELL_VER_OK",4=>"H5K: SHELL_LS_OK",5=>"H5K: SHELL_THEME_OK",0=>"H5K: SHELL_CLEAR_OK",7=>"H5K: SHELL_COMPAT_OK",8=>"H5K: SHELL_SECURITY_OK",9=>"H5K: SHELL_SHEET_OK",10=>"H5K: SHELL_CELL_OK",11=>"H5K: SHELL_CELL_REJECTED",12=>"H5K: SHELL_WINHELLO_OK",13=>"H5K: SHELL_WINHELLO_FAILED",14=>"H5K: SHELL_PEINFO_OK",15=>"H5K: SHELL_PEINFO_FAILED",16=>"H5K: SHELL_WINRUNTIME_OK",17=>"H5K: SHELL_WINRUNTIME_FAILED",18=>"H5K: SHELL_PERELOC_OK",19=>"H5K: SHELL_PERELOC_FAILED",20=>"H5K: SHELL_WINCONSOLE_OK",21=>"H5K: SHELL_WINCONSOLE_FAILED",22=>"H5K: SHELL_WINFAULT_OK",23=>"H5K: SHELL_WINFAULT_FAILED",24=>"H5K: SHELL_7ZIP_RETURNED",25=>"H5K: SHELL_7ZIP_BLOCKED",26=>"H5K: SHELL_INSTALL7ZIP_OK",27=>"H5K: SHELL_INSTALL7ZIP_FAILED",_=>"H5K: SHELL_UNKNOWN"});
                if result==10{serial::metric("H5K: SHEET_RESULT=",self.sheet_result as u64);}
                if result==5{self.theme();}else{self.draw();}
            }
            0x0e=>{self.term.len=self.term.len.saturating_sub(1);self.draw();}
            _=>{if let Some(b)=ascii(key,self.term.shift){if self.term.len<self.term.buf.len(){self.term.buf[self.term.len]=b;self.term.len+=1;self.draw();}}}
        }
    }
    fn connect_net(&mut self){if self.nic.is_none(){self.nic=Nic::open();}if let Some(n)=&mut self.nic{if n.configure().is_err(){serial::line("H5K: DHCP_CONNECT_FAILED");}}self.draw();}
    fn reopen_web(&mut self){
        if let Some(d)=self.store.load_download(self.web){
            self.web_len=d.len;self.web_ok=true;self.source=d.url;self.source_len=d.url_len;self.url=d.url;self.url_len=d.url_len;
            self.web_msg=if d.url_len==0{"Recovered legacy download - source unknown"}else{"Recovered saved download - not authenticated"};
            serial::line("H5K: GUI_DOWNLOAD_REOPENED");
        }else{self.web_ok=false;self.web_len=0;self.web_msg="No valid saved download";}self.draw();
    }
    fn fetch_web(&mut self){
        self.web_ok=false;self.web_len=0;self.web_msg="Connecting via DHCP / HTTP...";self.draw();
        if self.nic.is_none(){self.nic=Nic::open();}
        if let Some(nic)=&mut self.nic{match nic.get(&self.url[..self.url_len],self.web){Ok(n)=>{self.web_len=n;self.web_ok=true;self.source=self.url;self.source_len=self.url_len;self.web_msg="Download complete - not authenticated";},Err(e)=>{self.web_msg=e;serial::line("H5K: HTTP_DOWNLOAD_FAILED");serial::line(e);}}}else{self.web_msg="No supported E1000 network adapter";}
        self.draw();
    }
    fn save_web(&mut self){if self.web_ok{if self.web_len>crate::disk::DL_CAP{self.web_msg="Save limit 1 MiB - preview only";self.draw();return;}self.web_msg=if self.store.save_download(&self.web[..self.web_len],&self.source[..self.source_len]){"Saved binary download to data disk"}else{"Save failed - data disk required"};self.draw();}}
    fn browser_key(&mut self,key:u8){
        if key==0x1c{self.fetch_web();return;}if key==0x43{self.save_web();return;}if key==0x44{self.reopen_web();return;}
        if self.ctrl{if key==0x26{self.url_len=0;self.draw();}return;}
        if key==0x0e{self.url_len=self.url_len.saturating_sub(1);self.draw();return;}
        if let Some(b)=ascii(key,self.term.shift){if self.url_len<self.url.len(){self.url[self.url_len]=b;self.url_len+=1;self.draw();}}
    }
    fn sheet_key(&mut self,key:u8){
        match key{
            0x0f=>{self.sheet_cell=(self.sheet_cell+1)%16;self.sheet_len=0;},
            0x1c=>{let name=crate::sheet::Sheet::cell_name(self.sheet_cell);
                if self.sheet.set(&name,&self.sheet_edit[..self.sheet_len]){
                    self.sheet_msg="Cell updated";serial::line("H5K: GUI_SHEET_CELL_OK");serial::metric("H5K: GUI_SHEET_VALUE=",self.sheet.get(&name).unwrap_or(0)as u64);self.sheet_len=0;
                }else{self.sheet_msg="Invalid formula or value";serial::line("H5K: GUI_SHEET_REJECTED");}},
            0x0e=>self.sheet_len=self.sheet_len.saturating_sub(1),
            _=>{if let Some(b)=ascii(key,self.term.shift){if self.sheet_len<self.sheet_edit.len(){self.sheet_edit[self.sheet_len]=b;self.sheet_len+=1;}}}
        }self.draw();
    }
    fn key(&mut self,key:u8){
        match key{0x1d=>{self.ctrl=true;return;},0x9d=>{self.ctrl=false;return;},0x2a|0x36=>{self.term.shift=true;return;},0xaa|0xb6=>{self.term.shift=false;return;},_=>{}}
        if key&0x80!=0{return;}
        if key==0x01{if self.menu{self.menu=false;self.draw();}else{self.close();}return;}
        if key==0x41&&(self.app==Some(App::Files)||self.app==Some(App::Notepad)){self.switch_file((self.file+1)%FILE_COUNT);return;}
        if key==0x57{self.select(App::Sheet);return;}
        if key==0x42{self.select(App::Browser);return;}
        if key>=0x3b&&key<=0x3f{self.select(APPS[(key-0x3b)as usize]);return;}
        if self.app==Some(App::Sheet)&&!self.menu&&!self.minimized{self.sheet_key(key);return;}
        if self.app==Some(App::Browser)&&!self.menu&&!self.minimized{self.browser_key(key);return;}
        if self.app==Some(App::Notepad)&&!self.menu&&!self.minimized{self.notepad_key(key);return;}
        if self.app==Some(App::Terminal)&&!self.menu&&!self.minimized{self.terminal_key(key);return;}
        match key{
            0x02..=0x07=>self.select(APPS[(key-2)as usize]),0x32=>{self.menu=!self.menu;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();},
            0x2e if self.app==Some(App::Network)=>self.connect_net(),0x14=>self.theme(),0x2d if self.app.is_some()=>self.maximize(),
            0x12 if self.app==Some(App::Files)=>self.select(App::Notepad),0x31 if self.app==Some(App::Files)=>self.new_file(),0x20 if self.app==Some(App::Files)=>self.delete_file(),_=>{}
        }
    }
    fn click(&mut self){
        let x=self.mx;let y=self.my;let by=self.bar_y();let dock=self.dock_x();
        if y>=by{if x<92{self.menu=!self.menu;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();return;}if x>=dock&&x<dock+300{let a=APPS[(x-dock)/50];if self.app==Some(a)&&!self.minimized{self.minimized=true;self.draw();}else{self.select(a);}return;}}
        if self.menu{let m=self.menu_rect();if m.hit(x,y){if y>=m.y+49&&y<m.y+289{self.select(APPS[(y-m.y-49)/40]);}else if y>=m.y+298{self.menu=false;self.theme();}return;}self.menu=false;self.draw();}
        if let Some(a)=self.app{let r=self.win;if !self.minimized&&r.hit(x,y){
            if y<r.y+42{if x>=r.x+r.w-38{self.close();}else if x>=r.x+r.w-73{self.maximize();}else if x>=r.x+r.w-108{self.minimized=true;self.draw();}else if !self.maximized{self.drag=Some((x-r.x,y-r.y));}return;}
            let bx=r.x+22;let yy=r.y+58;
            if a==App::Sheet&&x>=bx+30&&y>=yy+87&&y<yy+207{let cw=(r.w-76)/4;let col=(x-bx-30)/cw;let row=(y-yy-87)/30;if col<4{self.sheet_cell=row*4+col;self.sheet_len=0;self.draw();}return;}
            if a==App::Network&&y>=yy+229&&y<yy+263&&x>=bx&&x<bx+190{self.connect_net();return;}
            if a==App::Browser&&y>=yy+42&&y<yy+76&&x>=r.x+r.w-95{self.fetch_web();return;}
            if a==App::Browser&&y>=yy+229&&y<yy+263&&x>=bx&&x<bx+180{self.save_web();return;}
            if a==App::Notepad&&y>=yy+1&&y<yy+34{if x>=bx&&x<bx+82{self.save_note();}else if x>=bx+92&&x<bx+174{self.clear_note();}return;}
            if a==App::Files&&y>=yy+88&&y<yy+88+FILE_COUNT*31{let i=(y-yy-88)/31;if i==self.file{self.select(App::Notepad);}else{self.switch_file(i);}return;}
            if a==App::Files&&y>=yy+43&&y<yy+77{if x>=bx&&x<bx+106{self.new_file();}else if x>=bx+116&&x<bx+206{self.delete_file();}}
            if a==App::System&&x>=bx&&x<bx+180&&y>=yy+223&&y<yy+257{self.theme();}return;
        }}
        if x<100&&y>=374&&y<435&&by>438{self.select(App::Sheet);return;}
        if x<100&&y>=8&&y<374{let i=(y-8)/61;if i<6{self.select(APPS[i]);}}
    }
    fn move_pointer(&mut self,dx:isize,dy:isize,pressed:bool,was:bool){
        self.cursor_hide();self.mouse=true;self.mx=(self.mx as isize+dx).clamp(0,self.c.fb.width as isize-1)as usize;self.my=(self.my as isize-dy).clamp(0,self.c.fb.height as isize-1)as usize;
        if pressed&&!was{self.click();}
        if !pressed&&was{if let Some((ox,oy))=self.drag.take(){self.win.x=self.mx.saturating_sub(ox).min(self.c.fb.width.saturating_sub(self.win.w));self.win.y=self.my.saturating_sub(oy).min(self.bar_y().saturating_sub(self.win.h));serial::line("H5K: GUI_WINDOW_MOVED");self.draw();return;}}
        if !self.saved_valid{self.cursor_show();}
    }
}
fn draw_ip(c:&Canvas,x:usize,y:usize,ip:[u8;4],color:Color){let mut b=[0u8;15];let mut n=0;for(i,v)in ip.iter().enumerate(){if i!=0{b[n]=b'.';n+=1;}if *v>=100{b[n]=b'0'+*v/100;n+=1;}if *v>=10{b[n]=b'0'+(*v/10)%10;n+=1;}b[n]=b'0'+*v%10;n+=1;}if let Ok(s)=core::str::from_utf8(&b[..n]){c.text(x,y,s,color);}}
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

pub fn run(fb:Framebuffer,ram:u64,nics:u32,files:Files,tasks:Tasks,mut store:Store)->!{
    if fb.width<640||fb.height<480{serial::line("H5K: FAIL DESKTOP_RESOLUTION");crate::kernel::halt();}
    let w=560.min(fb.width.saturating_sub(108));let h=365.min(fb.height.saturating_sub(66));let win=Rect{x:(fb.width-w)/2+20,y:(fb.height-52-h)/2,w,h};
    let mut cache=[EMPTY_FILE;FILE_COUNT];cache[0]=FileSlot{data:files.data,len:files.len,present:files.present};
    for i in 1..FILE_COUNT{if let Some((present,data,len))=store.load(i){cache[i]=FileSlot{data,len,present};}}
    let _=store.load(0);
    let mut url=[0;128];let default=b"http://10.0.2.200:8080/hello.exe";url[..default.len()].copy_from_slice(default);
    let mut ui=Ui{c:Canvas::new(fb),app:None,minimized:false,menu:false,dark:false,ram,nics,files,tasks,mx:fb.width/2,my:fb.height/2,mouse:false,saved:[0;256],saved_valid:false,win,restore:win,maximized:false,drag:None,term:Terminal::new(),sheet:crate::sheet::Sheet::new(),sheet_result:0,sheet_cell:0,sheet_edit:[0;48],sheet_len:0,sheet_msg:"Select cell and type value or formula",pe_report:crate::imports::Report::default(),rebase:crate::relocate::Report::default(),store,note:[0;CAP],note_len:0,dirty:false,save_status:0,ctrl:false,file:0,cache,nic:None,url,url_len:default.len(),web:unsafe{&mut *ptr::addr_of_mut!(WEB_BUFFER)},source:[0;128],source_len:0,web_len:0,web_ok:false,web_msg:"Offline until Go is pressed"};
    ui.mouse=init_input();serial::line(if ui.mouse{"H5K: PS2_MOUSE_ACK"}else{"H5K: PS2_MOUSE_UNAVAILABLE"});
    serial::line(if ui.c.buffered{"H5K: DOUBLE_BUFFER_OK"}else{"H5K: DIRECT_RENDER_FALLBACK"});
    ui.draw();serial::line("H5K: FRAMEBUFFER_OK");serial::line("H5K: DESKTOP_READY");
    p::timer_init();let start=p::ticks();while p::ticks().wrapping_sub(start)<3{p::wait_tick();}
    serial::line("H5K: TIMER_IRQ_OK");if !crate::net::self_test(){serial::line("H5K: FAIL NETWORK_FORMAT_TEST");crate::kernel::halt();}serial::line("H5K: NET_FORMAT_TEST_OK");serial::line("H5K: KERNEL_READY");
    let mut packet=[0u8;3];let mut pos=0;let mut down=false;let mut ext=false;
    loop{match ui.tasks.dispatch(){
        0=>{for _ in 0..32{let st=p::input(0x64);if st&1==0{break;}let b=p::input(0x60);if st&0x20!=0{
            if pos==0&&b&8==0{continue;}packet[pos]=b;pos+=1;if pos==3{pos=0;let flags=packet[0];if flags&0xc0!=0{continue;}let dx=packet[1]as isize-if flags&0x10!=0{256}else{0};let dy=packet[2]as isize-if flags&0x20!=0{256}else{0};let pressed=flags&1!=0;ui.move_pointer(dx,dy,pressed,down);down=pressed;}
        }else if b==0xe0{ext=true;}else{if !ext{ui.key(b);}ext=false;}}}
        1=>core::hint::spin_loop(),_=>p::wait_tick(),
    }}
}
