//! Hi5Desktop v0.3.4 — native software GUI; single foreground window.
use crate::{boot::Framebuffer,ui::{Canvas,Color},platform as p,services::{Files,Tasks},serial,disk::{Store,CAP,FILE_COUNT,NAMES}};
use core::ptr;
use crate::net::{Nic,MAX_RESPONSE};
const BLUE:Color=[34,112,195];const WHITE:Color=[255,255,255];const GOLD:Color=[234,178,66];
#[derive(Clone,Copy)]struct Palette{wall:Color,wall2:Color,bar:Color,panel:Color,sub:Color,ink:Color,muted:Color,edge:Color,green:Color}
fn palette(dark:bool)->Palette{if dark{Palette{wall:[19,39,61],wall2:[32,65,92],bar:[27,42,58],panel:[34,50,67],sub:[45,63,81],ink:[231,240,247],muted:[169,189,207],edge:[58,80,102],green:[113,221,166]}}else{Palette{wall:[211,229,244],wall2:[175,205,232],bar:[244,248,252],panel:[255,255,255],sub:[238,244,249],ink:[27,43,61],muted:[83,105,127],edge:[204,218,230],green:[23,128,83]}}}
#[derive(Clone,Copy,PartialEq)]enum App{System,Files,Network,Terminal,Notepad,Browser,Sheet,Store,WinDemo,WinLive}
impl App{fn title(self)->&'static str{match self{Self::System=>"Settings",Self::Files=>"File Explorer",Self::Network=>"Network",Self::Terminal=>"Terminal",Self::Notepad=>"Notepad",Self::Browser=>"Hi5Browser",Self::Sheet=>"Hi5Sheet",Self::Store=>"Hi5Store",Self::WinDemo=>"Windows GUI test",Self::WinLive=>"7-Zip File Manager"}}}
const APPS:[App;6]=[App::System,App::Files,App::Network,App::Terminal,App::Notepad,App::Browser];
#[derive(Clone,Copy)]struct Rect{x:usize,y:usize,w:usize,h:usize}
impl Rect{fn hit(self,x:usize,y:usize)->bool{x>=self.x&&x<self.x+self.w&&y>=self.y&&y<self.y+self.h}}
struct Terminal{buf:[u8;112],len:usize,last:[u8;112],last_len:usize,result:u8,shift:bool,scroll:usize,workspace:bool}
impl Terminal{fn new()->Self{Self{buf:[0;112],len:0,last:[0;112],last_len:0,result:0,shift:false,scroll:0,workspace:false}}}
#[derive(Clone,Copy)]struct FileSlot{data:[u8;CAP],len:usize,present:bool}
const EMPTY_FILE:FileSlot=FileSlot{data:[0;CAP],len:0,present:false};
// The large response buffer is image-owned static memory, not the 64 KiB kernel stack.
static mut WEB_BUFFER:[u8;MAX_RESPONSE]=[0;MAX_RESPONSE];
static mut WEB_ASSET:[u8;MAX_RESPONSE]=[0;MAX_RESPONSE];
static mut WEB_CSS:[u8;128*1024]=[0;128*1024];
static mut WORK_BUFFER:[u8;crate::trialfs::SNAP_CAP]=[0;crate::trialfs::SNAP_CAP];
static mut APP_BUFFER:[u8;crate::trial::PACKAGE_SIZE]=[0;crate::trial::PACKAGE_SIZE];
#[derive(Clone,Copy)]struct WindowSlot{app:App,win:Rect,restore:Rect,maximized:bool,minimized:bool}
struct Auth{stage:u8,account:Option<crate::account::Account>,region:u8,dark:bool,name:[u8;32],host:[u8;32],len:usize,host_len:usize,password:[u8;64],pass_len:usize,confirm:[u8;64],confirm_len:usize,delay:u64,msg:&'static str}
impl Auth{fn new(store:&Store)->Self{let (stage,account,msg)=match store.account(){Ok(Some(a))=>(7,Some(a),"Enter your password"),Ok(None)if store.setup_requested()=>(1,None,"Choose regional preferences"),Ok(None)=>(0,None,""),Err(())=>(8,None,"Account data unreadable. Recovery required.")};Self{stage,account,region:account.map_or(0,|a|a.region),dark:account.is_some_and(|a|a.dark),name:[0;32],host:[0;32],len:0,host_len:0,password:[0;64],pass_len:0,confirm:[0;64],confirm_len:0,delay:0,msg}}fn clear_password(&mut self){crate::account::wipe(&mut self.password);crate::account::wipe(&mut self.confirm);self.pass_len=0;self.confirm_len=0;}}
struct Ui{auth:Auth,auto_graphics:bool,tabs:[crate::http::Link;4],tab_count:usize,tab_index:usize,page:&'static mut crate::webpage::Page,history:[crate::http::Link;16],history_len:usize,history_pos:usize,history_replay:bool,web_status:u16,web_links:[crate::http::Link;8],web_link_count:usize,web_scroll:usize,web_text:bool,background:[Option<WindowSlot>;9],display_msg:&'static str,package_installed:bool,store_msg:&'static str,c:Canvas,app:Option<App>,minimized:bool,menu:bool,dark:bool,ram:u64,nics:u32,files:Files,tasks:Tasks,mx:usize,my:usize,mouse:bool,saved:[u32;256],saved_valid:bool,win:Rect,restore:Rect,maximized:bool,drag:Option<(usize,usize)>,term:Terminal,sheet:crate::sheet::Sheet,sheet_result:i64,sheet_cell:usize,sheet_edit:[u8;48],sheet_len:usize,sheet_msg:&'static str,pe_report:crate::imports::Report,rebase:crate::relocate::Report,store:Store,note:[u8;CAP],note_len:usize,dirty:bool,save_status:u8,ctrl:bool,file:usize,cache:[FileSlot;FILE_COUNT],nic:Option<Nic>,url:[u8;128],url_len:usize,web:&'static mut [u8;MAX_RESPONSE],source:[u8;128],source_len:usize,web_len:usize,web_ok:bool,web_msg:&'static str}
impl Ui{
    fn cursor_hide(&mut self){if !self.saved_valid{return;}for y in 0..16{for x in 0..16{self.c.restore_pixel(self.mx+x,self.my+y);}}self.saved_valid=false;}
    fn cursor_show(&mut self){if !self.mouse{return;}for y in 0..14{for x in 0..=y/2{self.c.front_pixel(self.mx+x,self.my+y,if x==0||x==y/2||y==13{[22,32,43]}else{WHITE});}}self.saved_valid=true;}
    fn bar_y(&self)->usize{self.c.fb.height.saturating_sub(52)}
    fn full_window(&self)->Rect{Rect{x:6,y:6,w:self.c.fb.width.saturating_sub(12),h:self.bar_y().saturating_sub(12)}}
    fn save_display(&mut self){if let Some(i)=crate::display::MODES.iter().position(|m|m.0==self.c.physical.width&&m.1==self.c.physical.height){if self.store.save_display(i,self.c.scale){serial::line("H5K: DISPLAY_SETTINGS_SAVED");}}}
    fn reflow(&mut self){let fb=self.c.fb;self.mx=self.mx.min(fb.width-1);self.my=self.my.min(fb.height-1);self.saved_valid=false;self.drag=None;let w=560.min(fb.width.saturating_sub(108));let h=365.min(fb.height.saturating_sub(66));self.restore=Rect{x:(fb.width-w)/2+20,y:(fb.height-52-h)/2,w,h};self.win=if self.maximized{self.full_window()}else{self.restore};let full=self.full_window();let restore=self.restore;for slot in self.background.iter_mut().flatten(){slot.restore=restore;slot.win=if slot.maximized{full}else{restore};}}
    fn apply_display(&mut self,i:usize){self.cursor_hide();if let Some(fb)=crate::display::set_mode(self.c.physical,i){self.c=Canvas::scaled(fb,self.c.scale);self.reflow();self.save_display();self.display_msg="Resolution applied";self.draw();serial::metric("H5K: GRAPHICS_WIDTH=",fb.width as u64);serial::metric("H5K: GRAPHICS_HEIGHT=",fb.height as u64);serial::line("H5K: GRAPHICS_MODE_OK");}else{self.display_msg="Mode unavailable - previous resolution retained";self.draw();serial::line("H5K: GRAPHICS_MODE_UNAVAILABLE");}}
    fn apply_scale(&mut self,scale:usize){if !Canvas::supported(self.c.physical,scale){self.display_msg="Scale needs a larger resolution";self.draw();return;}self.cursor_hide();self.c=Canvas::scaled(self.c.physical,scale);self.reflow();self.save_display();self.display_msg="Interface scale applied";self.draw();serial::metric("H5K: DISPLAY_SCALE=",scale as u64);}
    fn park_window(&mut self){if let Some(a)=self.app{if a==App::WinLive{return;}let slot=WindowSlot{app:a,win:self.win,restore:self.restore,maximized:self.maximized,minimized:self.minimized};if let Some(i)=self.background.iter().position(|s|s.is_none()){self.background[i]=Some(slot);}}}
    fn take_window(&mut self,a:App)->Option<WindowSlot>{let i=self.background.iter().position(|s|s.is_some_and(|v|v.app==a))?;let out=self.background[i];for j in i..8{self.background[j]=self.background[j+1];}self.background[8]=None;out}
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
            App::Store|App::WinDemo|App::WinLive=>{c.round(x,y,34,34,5,BLUE);c.text(x+5,y+5,"APP",WHITE);}
            App::Sheet=>{c.round(x,y,34,34,5,[25,135,84]);for i in 0..3{c.rect(x+5,y+7+i*9,24,1,WHITE);c.rect(x+7+i*9,y+5,1,24,WHITE);}}
        }
    }
    fn draw(&mut self){
        if self.auth.stage!=0{self.draw_auth();return;}
        self.saved_valid=false;
        let c=&self.c;let w=c.fb.width;let h=c.fb.height;let q=palette(self.dark);let by=self.bar_y();
        // Quiet, low-cost gradient and a native desktop with standard controls.
        for y in 0..by{let mix=|i:usize|((q.wall[i]as usize*(by-y)+q.wall2[i]as usize*y)/by.max(1))as u8;c.rect(0,y,w,1,[mix(0),mix(1),mix(2)]);}
        c.title(w.saturating_sub(145),20,"Hi5OS",q.ink);
        c.text(w.saturating_sub(202),54,concat!("Developer preview ",env!("CARGO_PKG_VERSION")),q.muted);
        for(i,a)in APPS.iter().enumerate(){let y=8+i*61;self.icon(*a,31,y);let label=match a{App::System=>"This PC",App::Files=>"Files",App::Network=>"Network",App::Terminal=>"Terminal",App::Notepad=>"Notepad",App::Browser=>"Browser",App::Sheet=>"Sheet",App::Store=>"Store",App::WinDemo=>"WinDemo",App::WinLive=>"Windows"};c.text(18,y+35,label,q.ink);}
        if by>438{self.icon(App::Sheet,31,374);c.text(18,409,"Sheet",q.ink);}
        let saved=self.win;for i in 0..9{if let Some(s)=self.background[i]{if !s.minimized{self.win=s.win;self.draw_window(s.app,q);}}}self.win=saved;if let Some(a)=self.app{if !self.minimized{self.draw_window(a,q);}}
        c.rect(0,by,w,h-by,q.bar);c.rect(0,by,w,1,q.edge);
        self.button(Rect{x:8,y:by+8,w:78,h:36},"Start",BLUE,WHITE);
        let dock=self.dock_x();
        for(i,a)in APPS.iter().enumerate(){let x=dock+i*50;if self.app==Some(*a)||self.background.iter().any(|s|s.is_some_and(|v|v.app==*a)){c.round(x,by+5,46,42,7,q.sub);c.rect(x+14,by+47,18,3,BLUE);}self.icon(*a,x+6,by+10);}
        c.text(w.saturating_sub(100),by+6,if self.nic.as_ref().map_or(false,|n|n.config().0!=[0;4]){"IPv4 ready"}else{"Offline"},q.muted);c.text(w.saturating_sub(100),by+25,"Hi5Kernel",q.ink);
        if self.menu{let r=self.menu_rect();c.round(r.x+3,r.y+4,r.w,r.h,12,q.edge);c.round(r.x,r.y,r.w,r.h,12,q.panel);c.title(r.x+16,r.y+10,"Start",q.ink);
            for(i,a)in APPS.iter().enumerate(){let y=r.y+49+i*40;self.icon(*a,r.x+14,y);c.text(r.x+60,y+6,a.title(),q.ink);}
            self.button(Rect{x:r.x+12,y:r.y+298,w:r.w-24,h:38},if self.dark{"Light appearance (T)"}else{"Dark appearance (T)"},q.sub,q.ink);
        }
        c.present();self.cursor_show();
    }
    fn draw_auth(&mut self){self.saved_valid=false;let c=&self.c;let q=palette(self.auth.dark);let w=c.fb.width;let h=c.fb.height;let x=(w-520)/2;let y=(h-400)/2;c.rect(0,0,w,h,q.wall);c.round(x,y,520,400,12,q.panel);c.title(x+24,y+20,if self.auth.stage==7{"Hi5OS - Sign in"}else{"Hi5OS - First-run setup"},q.ink);
        let a=&self.auth;let (heading,hint,value)=match a.stage{1=>("1 / 6  Language and region","1: English (UK)   2: English (US)",if a.region==0{"English (UK)"}else{"English (US)"}),2=>("2 / 6  Local username","Letters, digits, hyphen or underscore",core::str::from_utf8(&a.name[..a.len]).unwrap_or("")),3=>("3 / 6  Computer name","Letters, digits, hyphen or underscore",core::str::from_utf8(&a.host[..a.host_len]).unwrap_or("")),4=>("4 / 6  Create password","Use at least 12 characters (max 64)",""),5=>("5 / 6  Confirm password","Enter the same password again",""),6=>("6 / 6  Appearance","1: Light   2: Dark",if a.dark{"Dark"}else{"Light"}),7=>("Local account",a.account.as_ref().map_or("",|v|v.username()),""),_=>("Recovery required","Account data cannot be verified","")};
        c.title(x+24,y+68,heading,q.ink);c.text_limit(x+24,y+112,hint,q.muted,false,x+496);c.round(x+24,y+152,472,52,5,q.sub);
        if matches!(a.stage,4|5|7){let n=if a.stage==5{a.confirm_len}else{a.pass_len};let stars=[b'*';64];c.text_limit(x+36,y+165,core::str::from_utf8(&stars[..n]).unwrap_or(""),q.ink,false,x+488);}else{c.text_limit(x+36,y+165,value,q.ink,false,x+488);}
        c.text_limit(x+24,y+226,a.msg,q.muted,false,x+496);c.text_limit(x+24,y+256,"Local preview account - disk is not encrypted",q.muted,false,x+496);
        if a.stage!=8{self.button(Rect{x:x+24,y:y+308,w:472,h:48},if a.stage==7{"Sign in (Enter)"}else if a.stage==6{"Create account (Enter)"}else{"Next (Enter)"},BLUE,WHITE);}
        c.present();self.cursor_show();
    }
    fn auth_enter(&mut self){match self.auth.stage{
        1=>{self.auth.stage=2;self.auth.msg="Choose your local username";},
        2=>{if crate::account::valid_name(&self.auth.name[..self.auth.len]){self.auth.stage=3;self.auth.msg="Choose a computer name";}else{self.auth.msg="Invalid username";}},
        3=>{if crate::account::valid_name(&self.auth.host[..self.auth.host_len]){self.auth.stage=4;self.auth.msg="Create a unique test password";}else{self.auth.msg="Invalid computer name";}},
        4=>{if self.auth.pass_len>=12{self.auth.stage=5;self.auth.msg="Confirm password";}else{self.auth.msg="Minimum 12 characters";}},
        5=>{if self.auth.pass_len==self.auth.confirm_len&&self.auth.password[..self.auth.pass_len]==self.auth.confirm[..self.auth.confirm_len]{crate::account::wipe(&mut self.auth.confirm);self.auth.confirm_len=0;self.auth.stage=6;self.auth.msg="Choose appearance";}else{self.auth.clear_password();self.auth.stage=4;self.auth.msg="Passwords did not match - retry";}},
        6=>{self.auth.msg="Creating account - password derivation";self.draw();let a=crate::account::Account::new(&self.auth.name[..self.auth.len],&self.auth.host[..self.auth.host_len],self.auth.region,self.auth.dark,&self.auth.password[..self.auth.pass_len]);self.auth.clear_password();if let Some(a)=a{if self.store.create_account(&a){self.auth.account=Some(a);self.auth.stage=7;self.auth.msg="Account created - sign in";serial::line("H5K: ACCOUNT_CREATED");}else{self.auth=Auth::new(&self.store);if self.auth.stage==0{self.auth.stage=1;}self.auth.msg="Account save failed - check data disk";}}else{self.auth.stage=4;self.auth.msg="Hardware entropy required - check VM CPU";}},
        7=>{if p::ticks()<self.auth.delay{return;}self.auth.msg="Verifying password";self.draw();let ok=self.auth.account.is_some_and(|a|a.verify(&self.auth.password[..self.auth.pass_len]));self.auth.clear_password();if ok{self.auth.stage=0;self.dark=self.auth.dark;serial::line("H5K: ACCOUNT_LOGIN_OK");}else{self.auth.delay=p::ticks().saturating_add(300);self.auth.msg="Incorrect password - retry in 3 seconds";serial::line("H5K: ACCOUNT_LOGIN_REJECTED");}},
        _=>{}}
        self.draw();
    }
    fn auth_key(&mut self,key:u8){if key==0x1c{self.auth_enter();return;}let a=&mut self.auth;let ch=ascii(key,self.term.shift);if matches!(a.stage,1|6){if let Some(b'1'|b'2')=ch{let second=ch==Some(b'2');if a.stage==1{a.region=second as u8;}else{a.dark=second;}}}else{let (buf,len):(&mut[u8],&mut usize)=match a.stage{2=>(&mut a.name,&mut a.len),3=>(&mut a.host,&mut a.host_len),4|7=>(&mut a.password,&mut a.pass_len),5=>(&mut a.confirm,&mut a.confirm_len),_=>return};if key==0x0e{if *len>0{*len-=1;buf[*len]=0;}}else if let Some(b)=ch{if *len<buf.len(){buf[*len]=b;*len+=1;}}}self.draw();}
    fn lock_account(&mut self){if self.auth.account.is_none(){return;}if self.app==Some(App::WinLive){crate::trial::live_close();self.app=None;}self.menu=false;self.drag=None;self.auth.clear_password();self.auth.stage=7;self.auth.msg="Enter your password";self.auth.delay=0;serial::line("H5K: ACCOUNT_LOCKED");self.draw();}
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
            App::WinLive=>{crate::trial::live_draw(c,x,y,right.saturating_sub(x),(r.y+r.h).saturating_sub(y+8));},
            App::WinDemo=>{
                c.title(x,y,"Compiled Windows PE GUI fixture",q.ink);
                c.text_limit(x,y+38,"USER32/GDI32 calls through Hi5Compat",q.muted,false,right);
                c.text_limit(x,y+62,self.store_msg,q.ink,false,right);
                if let Some((xx,yy,ww,hh))=crate::trial::gui_rect(){let left=x+xx as usize;let top=y+90+yy as usize;let width=(ww as usize).min(right.saturating_sub(left));let height=(hh as usize).min((r.y+r.h).saturating_sub(top+20));if width>2&&height>2{c.rect(left,top,width,height,q.ink);c.rect(left+1,top+1,width-2,height-2,q.sub);}}
                c.text_limit(x,y+242,"Original 7-Zip File Manager not launched",q.muted,false,right);
            }
            App::Store=>{
                c.title(x,y,"Hi5Store - verified offline catalogue",q.ink);
                c.text_limit(x,y+40,"7-Zip 26.06 x64 - File Manager and console",q.ink,false,right);
                c.text_limit(x,y+65,if self.package_installed{"Installed - pinned SHA-256 verified"}else{"Not installed"},q.muted,false,right);
                self.button(Rect{x,y:y+98,w:110,h:34},"Install",BLUE,WHITE);
                self.button(Rect{x:x+120,y:y+98,w:110,h:34},"Uninstall",q.sub,q.ink);
                self.button(Rect{x,y:y+144,w:230,h:34},"Open terminal",q.sub,q.ink);self.button(Rect{x:x+240,y:y+144,w:215,h:34},"Open File Manager",if self.package_installed{BLUE}else{q.sub},if self.package_installed{WHITE}else{q.muted});
                c.text_limit(x,y+192,self.store_msg,q.muted,false,right);
                c.text_limit(x,y+220,"I: install   U: uninstall   G: File Manager",q.muted,false,right);
                c.text_limit(x,y+246,"Original EXE/MSI installer not yet supported",q.muted,false,right);
            }
            App::System=>{
                c.title(x,y,"Display settings",q.ink);
                c.text(x,y+38,"Current resolution:",q.muted);c.num(x+170,y+38,c.physical.width as u64,q.ink);c.text(x+224,y+38,"x",q.muted);c.num(x+242,y+38,c.physical.height as u64,q.ink);
                let native=crate::display::runtime_available(c.physical);
                c.text_limit(x,y+67,if native{"Standard VGA / Bochs - 32-bit native modes"}else{"UEFI framebuffer - fixed mode on this adapter"},q.muted,false,right);
                for (i,&(w,h,label)) in crate::display::MODES.iter().enumerate(){
                    let active=w==c.physical.width&&h==c.physical.height;let enabled=crate::display::mode_available(c.physical,i);
                    self.button(Rect{x:x+(i%3)*142,y:y+96+(i/3)*38,w:134,h:34},label,if active{BLUE}else{q.sub},if active{WHITE}else if enabled{q.ink}else{q.muted});
                }
                c.text_limit(x,y+176,self.display_msg,q.muted,false,right);
                c.text(x,y+202,"Interface scaling (Ctrl+Shift+1 to 5)",q.muted);
                for (i,(scale,label)) in [(100,"100%"),(125,"125%"),(150,"150%"),(175,"175%"),(200,"200%")].into_iter().enumerate(){let active=scale==c.scale;self.button(Rect{x:x+i*83,y:y+228,w:79,h:32},label,if active{BLUE}else{q.sub},if active{WHITE}else if Canvas::supported(c.physical,scale){q.ink}else{q.muted});}
                self.button(Rect{x,y:y+266,w:180,h:30},if self.dark{"Use light theme"}else{"Use dark theme"},q.sub,q.ink);c.text_limit(x+186,y+270,if self.auth.account.is_some(){"Ctrl+Shift+L: Lock"}else{"Ctrl+Shift+L: Account setup"},q.muted,false,right);
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
                c.title(x,y,"Hi5Browser",q.ink);self.button(Rect{x:x+155,y,w:50,h:30},"<",q.sub,q.ink);self.button(Rect{x:x+210,y,w:50,h:30},">",q.sub,q.ink);self.button(Rect{x:x+265,y,w:65,h:30},"Reload",q.sub,q.ink);self.button(Rect{x:x+335,y,w:105,h:30},"Graphics",q.sub,q.ink);
                c.round(x,y+42,r.w-132,34,5,q.sub);
                if let Ok(t)=core::str::from_utf8(&self.url[..self.url_len]){c.text_limit(x+6,y+48,t,q.ink,false,right-90);}
                self.button(Rect{x:r.x+r.w-95,y:y+42,w:72,h:34},"Go",BLUE,WHITE);
                c.text_limit(x,y+84,self.web_msg,q.muted,false,right);if self.web_status!=0{c.text(x,y+108,"HTTP status:",q.muted);c.num(x+114,y+108,self.web_status as u64,if self.web_status==200{q.ink}else{[205,110,80]});}
                if self.web_ok{
                    c.text(x+185,y+108,"Bytes:",q.ink);c.num(x+250,y+108,self.web_len as u64,q.ink);c.text(x+340,y+108,"Tab",q.muted);c.num(x+377,y+108,(self.tab_index+1)as u64,q.ink);c.text(x+397,y+108,"/",q.muted);c.num(x+410,y+108,self.tab_count as u64,q.ink);
                    let kind=match crate::compat::classify(&self.web[..self.web_len]){crate::compat::Kind::Pe64=>"PE64 executable candidate",crate::compat::Kind::Pe32=>"PE32 executable candidate",crate::compat::Kind::Compound=>"OLE compound-file candidate",_=>"Text or unknown binary"};
                    if self.web_text{
                        let top=y+137;let floor=r.y+r.h-25;let w=right.saturating_sub(x+13);
                        c.rect(x,top,w,floor.saturating_sub(top),[255,255,255]);
                        self.page.draw(c,x,top+3,x+w,floor,self.web_scroll,[30,34,40],[235,238,242]);
                        let rows=self.page.rows(w).max(1);let h=floor.saturating_sub(top);if h>20{c.rect(right-10,top,9,h,q.sub);let thumb=(h*6/rows).clamp(14,h);let offset=(h-thumb)*self.web_scroll.min(rows-1)/rows.saturating_sub(1).max(1);c.round(right-10,top+offset,9,thumb,3,BLUE);}
                    }else if let Some(bitmap)=crate::webpage::Bitmap::parse(&self.web[..self.web_len]){bitmap.draw(c,x,y+165,r.w-44,(r.y+r.h).saturating_sub(y+190));}else{
                        c.text_limit(x,y+137,kind,q.ink,false,right);c.text(x,y+165,"Binary preview - execution disabled",q.muted);
                        let src=&self.source[..self.source_len];let path=src.split(|b|*b==b'?').next().unwrap_or(&[]);
                        let name=path.rsplit(|b|*b==b'/').next().unwrap_or(&[]);
                        if let Ok(t)=core::str::from_utf8(name){if !t.is_empty(){c.text_limit(x,y+188,t,q.ink,false,right);}}
                    }
                    if !self.web_text&&self.web_status==200&&crate::webpage::Bitmap::parse(&self.web[..self.web_len]).is_none(){self.button(Rect{x,y:y+229,w:180,h:34},"Save download (F9)",q.sub,q.ink);}
                }else{c.text_limit(x,y+120,"HTTP / HTTPS websites and downloads (TLS 1.2 preview).",q.ink,false,right);c.text(x,y+149,"Ctrl+L URL; Ctrl+G form; Ctrl+T new tab; Ctrl+I graphics.",q.muted);}
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
                if self.term.result==24{
                    c.text_limit(x,y+91,if crate::trial::help_ok(){"7-Zip help qualified"}else if crate::trial::command_ok(){"7-Zip command completed"}else{"7-Zip command stopped"},fg,false,right);
                    if crate::trial::command_ok(){c.text(x,y+111,"Exit code:",fg);c.num(x+95,y+111,crate::trial::exit_code()as u64,green);}else{c.text_limit(x,y+111,"Execution failed or interrupted",fg,false,right);}
                    let mut err=[0u8;192];let n=crate::trial::diagnostic(&mut err);let mut row=y+133;
                    if n!=0{if let Ok(s)=core::str::from_utf8(&err[..n]){c.text_limit(x,row,s,fg,false,right);}row+=20;}
                    let bottom=r.y+r.h;let maxrows=bottom.saturating_sub(row+58)/20;
                    let raw=crate::trial::output_bytes();let text=core::str::from_utf8(raw).unwrap_or("Non UTF-8 output");
                    for (i,line) in text.lines().skip(self.term.scroll).take(maxrows).enumerate(){c.text_limit(x,row+i*20,line,fg,false,right);}
                    let prompt=bottom.saturating_sub(48);c.text(x,prompt,">",green);
                    if let Ok(t)=core::str::from_utf8(&self.term.buf[..self.term.len]){c.text_limit(x+16,prompt,t,fg,false,right);}
                    c.text_limit(x,bottom.saturating_sub(25),"PgUp/PgDn: scroll | Esc: close",fg,false,right);return;
                }
                match self.term.result{
                    1=>{c.text_limit(x,y+91,"F12: Hi5Store | Ctrl+F6: fixture | 7zlive: live trial",fg,false,right);}
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
                    24=>{},
                    25=>c.text_limit(x,y+91,"Trial blocked: verified 7za.exe required",fg,false,right),26=>c.text_limit(x,y+91,"7-Zip portable package installed on data disk",green,false,right),27=>c.text_limit(x,y+91,"Install requires Hi5OS data disk",fg,false,right),28=>c.text_limit(x,y+91,"Archive workspace source saved",green,false,right),29=>c.text_limit(x,y+91,"Archive workspace operation failed",fg,false,right),30=>c.text_limit(x,y+91,"ZIP and 7z extracted bytes match source",green,false,right),31=>c.text_limit(x,y+91,"MSI compound streams verified; install not executed",green,false,right),32=>c.text_limit(x,y+91,"MSI container unsupported or invalid",fg,false,right),33=>c.text_limit(x,y+91,"Portable package uninstalled; files retained",green,false,right),34=>{c.text_limit(x,y+91,"Original 7-Zip GUI runtime reached (diagnostic)",green,false,right);c.text(x,y+111,"API calls:",fg);c.num(x+120,y+111,crate::trial::original_gui_calls(),green);},35=>c.text_limit(x,y+91,"Original GUI startup blocked; see serial log",fg,false,right),_=>{}
                }
                c.text(x,y+133,">",green);
                if let Ok(t)=core::str::from_utf8(&self.term.buf[..self.term.len]){c.text_limit(x+16,y+133,t,fg,false,right);}
                c.text(x,y+174,"Esc: close terminal",fg);
            }
        }
    }
    fn select(&mut self,a:App){if self.app!=Some(a){let previous=self.app;if previous==Some(App::WinLive){crate::trial::live_close();}else{self.park_window();}if let Some(s)=self.take_window(a){self.win=s.win;self.restore=s.restore;self.maximized=s.maximized;}else{self.win=self.restore;self.win.x=(self.win.x+24).min(self.c.fb.width.saturating_sub(self.win.w));self.win.y=(self.win.y+18).min(self.bar_y().saturating_sub(self.win.h));self.restore=self.win;self.maximized=false;}serial::line("H5K: GUI_WINDOW_FOCUSED");}if a==App::WinLive&&!self.maximized{self.restore=self.win;self.win=self.full_window();self.maximized=true;}if a==App::WinDemo{self.store_msg=if crate::trial::run_gui_fixture(){"PE/DLL and Win32 drawing fixture passed"}else{"PE GUI fixture failed"};}if a==App::Store{let b=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};self.package_installed=self.store.load_7zip(b).is_some();}if a==App::Notepad&&!self.dirty{self.load_note();}self.app=Some(a);self.minimized=false;self.menu=false;serial::line(match a{App::System=>"H5K: GUI_SYSTEM_OPEN",App::Files=>"H5K: GUI_FILES_OPEN",App::Network=>"H5K: GUI_NETWORK_OPEN",App::Terminal=>"H5K: GUI_TERMINAL_OPEN",App::Notepad=>"H5K: GUI_NOTEPAD_OPEN",App::Browser=>"H5K: GUI_BROWSER_OPEN",App::Sheet=>"H5K: GUI_SHEET_OPEN",App::Store=>"H5K: GUI_STORE_OPEN",App::WinDemo=>"H5K: GUI_WINDEMO_OPEN",App::WinLive=>"H5K: GUI_WINLIVE_OPEN"});self.draw();}
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
    fn close(&mut self){if self.app==Some(App::WinLive){crate::trial::live_close();}self.app=None;self.minimized=false;self.menu=false;self.drag=None;serial::line("H5K: GUI_HOME");self.draw();}
    fn maximize(&mut self){if self.maximized{self.win=self.restore;}else{self.restore=self.win;self.win=Rect{x:6,y:6,w:self.c.fb.width.saturating_sub(12),h:self.bar_y().saturating_sub(12)};}self.maximized=!self.maximized;serial::line(if self.maximized{"H5K: GUI_MAXIMIZED"}else{"H5K: GUI_RESTORED"});self.draw();}
    fn load_workspace(&mut self){if !self.term.workspace{let b=unsafe{&mut *ptr::addr_of_mut!(WORK_BUFFER)};if let Some(n)=self.store.load_workspace(b){let _=crate::trial::workspace_load(&b[..n]);}else{crate::trial::workspace_ready();}self.term.workspace=true;}}
    fn save_workspace(&mut self)->bool{let b=unsafe{&mut *ptr::addr_of_mut!(WORK_BUFFER)};if let Some(n)=crate::trial::workspace_pack(b){self.store.save_workspace(&b[..n])}else{false}}
    fn terminal_key(&mut self,key:u8){
        match key{
            0x1c=>{
                let cmd_copy=self.term.buf;let cmd=&cmd_copy[..self.term.len];let result=match cmd{b"help"=>1,b"mem"=>2,b"ver"=>3,b"ls"=>4,b"theme"=>5,b"clear"=>0,b"compat"=>7,b"security"=>8,b"sheet"=>9,b"winhello"=>{if crate::platform::run_pe_fixture(){12}else{13}},b"winfault"=>{if crate::trial::self_test(){22}else{23}},b"install7zip"=>{let src=if self.web_ok&&crate::trial::verified(&self.web[..self.web_len]){&self.web[..self.web_len]}else{crate::trial::BUNDLED};if self.store.install_7zip(src){26}else{27}},b"arcbig"=>{self.load_workspace();if crate::trial::workspace_large_seed()&&self.save_workspace(){28}else{29}},b"arcupdate"=>{self.load_workspace();if crate::trial::workspace_update_seed()&&self.save_workspace(){28}else{29}},b"uninstall7zip"=>{if self.store.uninstall_7zip(){33}else{27}},b"7zfm"=>{if crate::trial::original_gui_probe(){34}else{35}},b"7zlive"=>{if crate::trial::original_gui_live(self.c.fb.width.saturating_sub(48),self.bar_y().saturating_sub(78)){self.select(App::WinLive);serial::line("H5K: SHELL_ORIGINAL_GUI_LIVE");34}else{serial::line("H5K: SHELL_ORIGINAL_GUI_LIVE_BLOCKED");35}},b"arcseed"=>{self.load_workspace();if crate::trial::workspace_seed(crate::trial::SAMPLE)&&self.save_workspace(){28}else{29}},b"arcimport"=>{self.load_workspace();let s=self.cache[self.file];if s.present&&crate::trial::workspace_seed(&s.data[..s.len])&&self.save_workspace(){28}else{29}},b"msiinfo"=>{if self.web_ok&&crate::msi::inspect(&self.web[..self.web_len]).is_some(){31}else{32}},b"arcverify"=>{self.load_workspace();if crate::trial::workspace_verify(){30}else{29}},_ if cmd.starts_with(b"7za ")=>{self.load_workspace();let buf=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};let r=if let Some(n)=self.store.load_7zip(buf){crate::trial::run_command(&buf[..n],cmd)}else{Err("Install verified 7za.exe via Hi5Store first")};match r{Ok(_)=>{if crate::trial::command_ok()&&cmd!=b"7za -h"&&cmd!=b"7za i"&&!self.save_workspace(){serial::line("H5K: WORKSPACE_SAVE_FAILED");29}else{24}},Err(e)=>{serial::line(e);25}}},b"winconsole"=>{if crate::platform::run_console_fixture(){20}else{21}},b"winruntime"=>{if crate::platform::run_runtime_fixture(){16}else{17}},b"pereloc"=>{if self.web_ok{match crate::relocate::inspect(&self.web[..self.web_len]){Ok(r)=>{self.rebase=r;18},Err(e)=>{serial::line(e);19}}}else{19}},b"pehello"=>{match crate::imports::log(crate::compat::HELLO){Ok(r)=>{self.pe_report=r;14},Err(_)=>15}},b"peinfo"=>{if self.web_ok{match crate::imports::log(&self.web[..self.web_len]){Ok(r)=>{self.pe_report=r;14},Err(e)=>{serial::line(e);15}}}else{15}},_ if cmd.starts_with(b"set ")||cmd.starts_with(b"get ")=>{if let Some(v)=self.sheet.command(cmd){self.sheet_result=v;10}else{11}},_=>6};
                self.term.last=self.term.buf;self.term.last_len=if result==0{0}else{self.term.len};self.term.len=0;self.term.result=result;self.term.scroll=0;
                serial::line(match result{1=>"H5K: SHELL_HELP_OK",2=>"H5K: SHELL_MEM_OK",3=>"H5K: SHELL_VER_OK",4=>"H5K: SHELL_LS_OK",5=>"H5K: SHELL_THEME_OK",0=>"H5K: SHELL_CLEAR_OK",7=>"H5K: SHELL_COMPAT_OK",8=>"H5K: SHELL_SECURITY_OK",9=>"H5K: SHELL_SHEET_OK",10=>"H5K: SHELL_CELL_OK",11=>"H5K: SHELL_CELL_REJECTED",12=>"H5K: SHELL_WINHELLO_OK",13=>"H5K: SHELL_WINHELLO_FAILED",14=>"H5K: SHELL_PEINFO_OK",15=>"H5K: SHELL_PEINFO_FAILED",16=>"H5K: SHELL_WINRUNTIME_OK",17=>"H5K: SHELL_WINRUNTIME_FAILED",18=>"H5K: SHELL_PERELOC_OK",19=>"H5K: SHELL_PERELOC_FAILED",20=>"H5K: SHELL_WINCONSOLE_OK",21=>"H5K: SHELL_WINCONSOLE_FAILED",22=>"H5K: SHELL_WINFAULT_OK",23=>"H5K: SHELL_WINFAULT_FAILED",24=>"H5K: SHELL_7ZIP_RETURNED",25=>"H5K: SHELL_7ZIP_BLOCKED",26=>"H5K: SHELL_INSTALL7ZIP_OK",27=>"H5K: SHELL_INSTALL7ZIP_FAILED",28=>"H5K: SHELL_ARCSEED_OK",29=>"H5K: SHELL_WORKSPACE_FAILED",30=>"H5K: SHELL_ARCHIVE_VERIFY_OK",31=>"H5K: SHELL_MSIINFO_OK",32=>"H5K: SHELL_MSIINFO_FAILED",33=>"H5K: SHELL_UNINSTALL7ZIP_OK",34=>"H5K: SHELL_ORIGINAL_GUI_PROBED",35=>"H5K: SHELL_ORIGINAL_GUI_BLOCKED",_=>"H5K: SHELL_UNKNOWN"});
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
            self.web_len=d.len;self.web_ok=true;self.web_status=200;self.source=d.url;self.source_len=d.url_len;self.url=d.url;self.url_len=d.url_len;
            self.web_msg=if d.url_len==0{"Recovered legacy download - source unknown"}else{"Recovered saved download - not authenticated"};
            self.classify_web();serial::line("H5K: GUI_DOWNLOAD_REOPENED");
        }else{self.web_ok=false;self.web_len=0;self.web_msg="No valid saved download";}self.draw();
    }
    fn classify_web(&mut self){self.page.clear();self.web_scroll=0;self.web_link_count=0;self.web_text=self.web_ok&&self.web[..self.web_len].iter().all(|b|*b==9||*b==10||*b==13||*b>=32)&&core::str::from_utf8(&self.web[..self.web_len]).is_ok();if self.web_text{self.page.parse_at(&self.web[..self.web_len],&self.source[..self.source_len],&[]);serial::metric("H5K: BROWSER_LAYOUT_BLOCKS=",self.page.count as u64);serial::metric("H5K: WEB_CORE_DOM_NODES=",self.page.dom_nodes as u64);self.web_document_diagnostics();if self.page.form.ready{serial::line("H5K: WEB_CORE_FORM_READY");}serial::metric("H5K: BROWSER_STYLE_BLOCKS=",self.page.styled as u64);self.web_link_count=crate::http::links(&self.web[..self.web_len],&self.source[..self.source_len],&mut self.web_links);serial::line("H5K: BROWSER_PAGE_RENDERED");serial::metric("H5K: BROWSER_LINKS=",self.web_link_count as u64);}else if self.web_ok&&crate::webpage::Bitmap::parse(&self.web[..self.web_len]).is_some(){serial::line("H5K: BROWSER_BITMAP_RENDERED");}}
    fn web_document_diagnostics(&mut self){serial::metric("H5K: WEB_CORE_SCRIPTS=",self.page.scripts as u64);if self.page.requires_js{self.web_msg="Website requires JavaScript - search results not supplied";serial::line("H5K: WEB_CORE_JS_REQUIRED");}else if self.page.empty_document{self.web_msg="HTML received - no supported visible content";serial::line("H5K: WEB_CORE_EMPTY_DOCUMENT");}if self.page.truncated{serial::line("H5K: WEB_CORE_DOCUMENT_TRUNCATED");}}
    fn follow_link(&mut self,i:usize){if i>=self.web_link_count{return;}let l=self.web_links[i];self.url=l.url;self.url_len=l.len;serial::line("H5K: BROWSER_LINK_OPENED");self.fetch_web();}
    fn remember_page(&mut self){if self.history_replay||!self.web_ok{return;}let l=crate::http::Link{url:self.source,len:self.source_len};if self.history_len>0&&self.history[self.history_pos].url[..self.history[self.history_pos].len]==l.url[..l.len]{return;}if self.history_len>0{self.history_len=self.history_pos+1;}if self.history_len==16{self.history.copy_within(1..16,0);self.history_len=15;}self.history[self.history_len]=l;self.history_pos=self.history_len;self.history_len+=1;}
    fn history_go(&mut self,forward:bool){if self.history_len==0{return;}let next=if forward{(self.history_pos+1).min(self.history_len-1)}else{self.history_pos.saturating_sub(1)};if next==self.history_pos{return;}self.history_pos=next;let l=self.history[next];self.url=l.url;self.url_len=l.len;self.history_replay=true;self.fetch_web();self.history_replay=false;serial::line("H5K: BROWSER_HISTORY_NAVIGATED");}
    fn dynamic_preview(&mut self){if !self.web_text{return;}let n=self.page.literal_update(&self.web[..self.web_len]);self.web_msg=if n>0{"Limited textContent update applied"}else{"No supported script update (general JS disabled)"};serial::metric("H5K: BROWSER_LITERAL_UPDATES=",n as u64);self.draw();}
    fn load_graphics(&mut self){
        if !self.web_ok||!self.web_text||self.web_status!=200{return;}self.web_msg="Loading bounded stylesheet and image resources";self.draw();let Some(nic)=self.nic.as_mut()else{return;};
        let buffer=unsafe{&mut *ptr::addr_of_mut!(WEB_ASSET)};let css=unsafe{&mut *ptr::addr_of_mut!(WEB_CSS)};let mut used=0;let mut sheets=0;
        self.web_msg="Loading bounded stylesheet and image resources";
        let list=self.page.stylesheets;for l in &list[..self.page.stylesheet_count]{if let Ok((n,final_url,len,status))=nic.get_follow(&l.url[..l.len],buffer){if status==200&&(!self.source[..self.source_len].starts_with(b"https://")||final_url[..len].starts_with(b"https://"))&&n<css.len().saturating_sub(used+1)&&core::str::from_utf8(&buffer[..n]).is_ok(){css[used..used+n].copy_from_slice(&buffer[..n]);used+=n;css[used]=b'\n';used+=1;sheets+=1;}}}
        self.page.parse_at(&self.web[..self.web_len],&self.source[..self.source_len],&css[..used]);let list=self.page.image_urls;for (i,l) in list[..self.page.image_count].iter().enumerate(){if let Ok((n,final_url,len,status))=nic.get_follow(&l.url[..l.len],buffer){if status==200&&(!self.source[..self.source_len].starts_with(b"https://")||final_url[..len].starts_with(b"https://")){self.page.load_image(i,&buffer[..n]);}}}
        self.web_msg="Graphics pass complete - limited CSS and images";self.web_document_diagnostics();self.web_scroll=0;serial::metric("H5K: BROWSER_STYLESHEETS_LOADED=",sheets as u64);serial::metric("H5K: BROWSER_STYLE_BLOCKS=",self.page.styled as u64);serial::metric("H5K: BROWSER_IMAGES_LOADED=",self.page.loaded_images as u64);self.draw();
    }
    fn tab_save(&mut self){self.tabs[self.tab_index]=crate::http::Link{url:self.url,len:self.url_len};}
    fn tab_open(&mut self){if self.tabs[self.tab_index].len>0{let t=self.tabs[self.tab_index];self.url=t.url;self.url_len=t.len;self.fetch_web();}else{self.url_len=0;self.web_ok=false;self.web_len=0;self.web_status=0;self.web_text=false;self.web_link_count=0;self.page.clear();self.web_msg="New tab - enter address";self.draw();}serial::metric("H5K: BROWSER_TAB_COUNT=",self.tab_count as u64);serial::metric("H5K: BROWSER_TAB_ACTIVE=",self.tab_index as u64);}
    fn tab_new(&mut self){if self.tab_count==4{return;}self.tab_save();self.tab_index=self.tab_count;self.tab_count+=1;self.tabs[self.tab_index]=crate::http::EMPTY_LINK;self.tab_open();}
    fn tab_next(&mut self){if self.tab_count<2{return;}self.tab_save();self.tab_index=(self.tab_index+1)%self.tab_count;self.tab_open();}
    fn tab_close(&mut self){if self.tab_count<=1{return;}for i in self.tab_index..self.tab_count-1{self.tabs[i]=self.tabs[i+1];}self.tab_count-=1;self.tab_index=self.tab_index.min(self.tab_count-1);self.tab_open();}
    fn submit_form(&mut self){let Some(l)=self.page.form.target()else{self.web_msg="Search URL too long or unsupported form";self.draw();return;};self.page.form_focus=false;self.url=l.url;self.url_len=l.len;serial::line("H5K: WEB_CORE_FORM_SUBMITTED");self.fetch_web();}
    fn fetch_web(&mut self){
        self.web_ok=false;self.web_len=0;self.web_status=0;self.web_text=false;self.web_link_count=0;self.web_msg="Connecting via DHCP / HTTP...";self.draw();
        if !self.url[..self.url_len].starts_with(b"http://")&&!self.url[..self.url_len].starts_with(b"https://")&&self.url_len+8<=self.url.len(){self.url.copy_within(..self.url_len,8);self.url[..8].copy_from_slice(b"https://");self.url_len+=8;}
        if self.nic.is_none(){self.nic=Nic::open();}
        if let Some(nic)=&mut self.nic{match nic.get_follow(&self.url[..self.url_len],self.web){Ok((n,url,len,status))=>{self.web_len=n;self.web_ok=true;self.web_status=status;self.source=url;self.source_len=len;self.url=url;self.url_len=len;self.web_msg=if status!=200{"Server returned an HTTP error page"}else if url[..len].starts_with(b"https://"){"HTTPS - certificate validated"}else{"HTTP response complete - not authenticated"};},Err(e)=>{self.web_msg=e;serial::line("H5K: HTTP_DOWNLOAD_FAILED");serial::line(e);}}}else{self.web_msg="No supported E1000 network adapter";}
        self.classify_web();self.remember_page();self.tab_save();self.draw();if self.auto_graphics&&self.web_text&&(self.page.stylesheet_count>0||self.page.image_count>0){self.load_graphics();}
    }
    fn previous_download(&mut self){if let Some(d)=self.store.load_previous_download(self.web){self.web_len=d.len;self.web_ok=true;self.web_status=200;self.source=d.url;self.source_len=d.url_len;self.url=d.url;self.url_len=d.url_len;self.web_msg="Previous saved download - not authenticated";self.classify_web();serial::line("H5K: BROWSER_PREVIOUS_DOWNLOAD");}else{self.web_msg="No previous valid download";}self.draw();}
    fn save_web(&mut self){if self.web_ok&&self.web_status==200{if self.web_len>crate::disk::DL_CAP{self.web_msg="Save limit 1 MiB - preview only";self.draw();return;}self.web_msg=if self.store.save_download(&self.web[..self.web_len],&self.source[..self.source_len]){"Saved binary download to data disk"}else{"Save failed - data disk required"};self.draw();}}
    fn browser_key(&mut self,key:u8){
        if key==0x1c{if self.page.form_focus{self.submit_form();}else{self.fetch_web();}return;}if key==0x43{self.save_web();return;}if key==0x44{self.reopen_web();return;}
        if self.ctrl{if key==0x26{self.page.form_focus=false;self.url_len=0;self.draw();}else if key==0x14{self.tab_new();}else if key==0x0f{self.tab_next();}else if key==0x11{self.tab_close();}else if key==0x22&&self.page.form.ready{self.page.form_focus=true;self.draw();}else if key==0x17&&self.term.shift{self.auto_graphics=!self.auto_graphics;self.web_msg=if self.auto_graphics{"Automatic graphics enabled"}else{"Automatic graphics disabled"};self.draw();}else if key==0x24{self.dynamic_preview();}else if key==0x17{self.load_graphics();}else if key==0x30{self.history_go(false);}else if key==0x21{self.history_go(true);}else if key==0x13{self.fetch_web();}else if key==0x20{self.previous_download();}else if (0x02..=0x09).contains(&key){self.follow_link((key-2)as usize);}return;}
        if key==0x0f&&self.page.form.ready{self.page.form_focus=!self.page.form_focus;self.draw();return;}
        if self.page.form_focus{if key==0x0e{self.page.form.backspace();}else if let Some(ch)=ascii(key,self.term.shift){self.page.form.input(ch);}self.draw();return;}
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
        if self.auth.stage!=0{self.auth_key(key);return;}
        if self.ctrl&&self.term.shift&&key==0x26{if self.auth.account.is_some(){self.lock_account();}else if self.app==Some(App::System)&&self.store.attached(){self.auth.stage=1;self.auth.msg="Choose regional preferences";self.draw();}return;}
        if self.app==Some(App::System)&&self.ctrl&&(0x02..=0x07).contains(&key){if self.term.shift{if key<=6{self.apply_scale([100,125,150,175,200][(key-2)as usize]);}}else{self.apply_display((key-2)as usize);}return;}
        if self.app==Some(App::WinLive){if key==1&&self.ctrl{self.close();return;}let ch=ascii(key,self.term.shift);let vk=match key{0x01=>27,0x0e=>8,0x0f=>9,0x1c=>13,0x3b..=0x44=>112+(key-0x3b)as u32,_=>ch.map_or(0,|v|v.to_ascii_uppercase()as u32)};if vk!=0{crate::trial::live_key(vk,ch);if !crate::trial::live_active(){self.close();}else{self.draw();}}return;}
        if key==0x01{if self.menu{self.menu=false;self.draw();}else{self.close();}return;}
        if key==0x41&&(self.app==Some(App::Files)||self.app==Some(App::Notepad)){self.switch_file((self.file+1)%FILE_COUNT);return;}
        // Preserve F6 as the existing Notepad save shortcut. GUI fixture is Ctrl+F6.
        if key==0x40&&self.ctrl{self.select(App::WinDemo);return;}
        if key==0x58{self.select(App::Store);return;}
        if self.app==Some(App::Store)&&!self.menu&&!self.minimized{match key{0x17=>self.package_action(true),0x16=>self.package_action(false),0x22=>self.launch_registered(),0x1c=>self.select(App::Terminal),_=>{}}return;}
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
    fn launch_registered(&mut self){let buf=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};if self.store.load_7zip(buf).is_none(){self.store_msg="Install the verified package first";self.draw();return;}if crate::trial::original_gui_live(self.c.fb.width.saturating_sub(48),self.bar_y().saturating_sub(78)){self.select(App::WinLive);serial::line("H5K: STORE_GUI_LAUNCHED");}else{self.store_msg="Original GUI startup blocked";self.draw();}}
    fn package_action(&mut self,install:bool){let ok=if install{self.store.install_7zip(crate::trial::BUNDLED)}else{self.store.uninstall_7zip()};if ok{self.package_installed=install;}self.store_msg=if ok{if install{"Portable package installed"}else{"Package disabled; user files retained"}}else{"Package transaction failed"};serial::line(if ok{if install{"H5K: STORE_INSTALL_OK"}else{"H5K: STORE_UNINSTALL_OK"}}else{"H5K: STORE_ACTION_FAILED"});self.draw();}
    fn click(&mut self){
        let x=self.mx;let y=self.my;if self.auth.stage!=0{let ox=(self.c.fb.width-520)/2;let oy=(self.c.fb.height-400)/2;if x>=ox+24&&x<ox+496&&y>=oy+308&&y<oy+356{self.auth_enter();}return;}let by=self.bar_y();let dock=self.dock_x();
        if y>=by{if x<92{self.menu=!self.menu;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();return;}if x>=dock&&x<dock+300{let a=APPS[(x-dock)/50];if self.app==Some(a)&&!self.minimized{self.minimized=true;self.draw();}else{self.select(a);}return;}}
        if self.menu{let m=self.menu_rect();if m.hit(x,y){if y>=m.y+49&&y<m.y+289{self.select(APPS[(y-m.y-49)/40]);}else if y>=m.y+298{self.menu=false;self.theme();}return;}self.menu=false;self.draw();}
        if let Some(a)=self.app{let r=self.win;if !self.minimized&&r.hit(x,y){
            if y<r.y+42{if x>=r.x+r.w-38{self.close();}else if x>=r.x+r.w-73{self.maximize();}else if x>=r.x+r.w-108{self.minimized=true;self.draw();}else if !self.maximized{self.drag=Some((x-r.x,y-r.y));}return;}
            let bx=r.x+22;let yy=r.y+58;
            if a==App::WinLive{if x>=bx&&y>=yy{crate::trial::live_click(x-bx,y-yy);if !crate::trial::live_active(){self.close();}else{self.draw();}}return;}
            if a==App::Store&&x>=bx+240&&x<bx+455&&y>=yy+144&&y<yy+178{self.launch_registered();return;}
            if a==App::Store&&x>=bx&&x<bx+230{if y>=yy+98&&y<yy+132{self.package_action(x<bx+120);return;}if y>=yy+144&&y<yy+178{self.select(App::Terminal);return;}}
            if a==App::Sheet&&x>=bx+30&&y>=yy+87&&y<yy+207{let cw=(r.w-76)/4;let col=(x-bx-30)/cw;let row=(y-yy-87)/30;if col<4{self.sheet_cell=row*4+col;self.sheet_len=0;self.draw();}return;}
            if a==App::Network&&y>=yy+229&&y<yy+263&&x>=bx&&x<bx+190{self.connect_net();return;}
            if a==App::Browser&&y>=yy&&y<yy+30{if x>=bx+155&&x<bx+205{self.history_go(false);return;}if x>=bx+210&&x<bx+260{self.history_go(true);return;}if x>=bx+265&&x<bx+330{self.fetch_web();return;}if x>=bx+335&&x<bx+440{self.load_graphics();return;}}
            if a==App::Browser&&y>=yy+42&&y<yy+76{if x>=r.x+r.w-95{self.fetch_web();return;}if x>=bx{self.page.form_focus=false;self.draw();return;}}
            if a==App::Browser&&self.web_text&&y>=yy+137&&y<r.y+r.h-25&&x>=bx{let right=r.x+r.w-22;let w=right.saturating_sub(bx+13);if x>=right-10{let rows=self.page.rows(w).max(1);self.web_scroll=((y-(yy+137))*rows/(r.y+r.h-25-(yy+137)).max(1)).min(rows-1);self.draw();return;}let control=self.page.control_hit(w,yy+140,r.y+r.h-25,self.web_scroll,y);if control==3{self.page.form_focus=true;self.draw();return;}if control==4{self.submit_form();return;}if let Some(l)=self.page.hit(w,yy+140,r.y+r.h-25,self.web_scroll,y){self.url=l.url;self.url_len=l.len;serial::line("H5K: BROWSER_INLINE_LINK");self.fetch_web();return;}}
            if a==App::Browser&&!self.web_text&&y>=yy+229&&y<yy+263&&x>=bx&&x<bx+180{self.save_web();return;}
            if a==App::Notepad&&y>=yy+1&&y<yy+34{if x>=bx&&x<bx+82{self.save_note();}else if x>=bx+92&&x<bx+174{self.clear_note();}return;}
            if a==App::Files&&y>=yy+88&&y<yy+88+FILE_COUNT*31{let i=(y-yy-88)/31;if i==self.file{self.select(App::Notepad);}else{self.switch_file(i);}return;}
            if a==App::Files&&y>=yy+43&&y<yy+77{if x>=bx&&x<bx+106{self.new_file();}else if x>=bx+116&&x<bx+206{self.delete_file();}}
            if a==App::System{
                if x>=bx&&x<bx+426&&y>=yy+96&&y<yy+168{let row=(y-yy-96)/38;let col=(x-bx)/142;if (x-bx)%142<134&&(y-yy-96)%38<34{self.apply_display(row*3+col);return;}}
                if x>=bx&&x<bx+415&&y>=yy+228&&y<yy+260{self.apply_scale([100,125,150,175,200][(x-bx)/83]);return;}if x>=bx&&x<bx+180&&y>=yy+266&&y<yy+296{self.theme();}
            }return;
        }}
        if let Some(s)=self.background.iter().rev().flatten().find(|s|!s.minimized&&s.win.hit(x,y)).copied(){self.select(s.app);return;}
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
    let mut canvas=Canvas::new(fb);if let Some((mode,scale))=store.display_settings(){if let Some(actual)=crate::display::set_mode(fb,mode){canvas=Canvas::scaled(actual,scale);serial::line("H5K: DISPLAY_SETTINGS_RESTORED");serial::metric("H5K: DISPLAY_SCALE=",canvas.scale as u64);}}let fb=canvas.fb;
    if fb.width<640||fb.height<480{serial::line("H5K: FAIL DESKTOP_RESOLUTION");crate::kernel::halt();}
    let w=560.min(fb.width.saturating_sub(108));let h=365.min(fb.height.saturating_sub(66));let win=Rect{x:(fb.width-w)/2+20,y:(fb.height-52-h)/2,w,h};
    let mut cache=[EMPTY_FILE;FILE_COUNT];cache[0]=FileSlot{data:files.data,len:files.len,present:files.present};
    for i in 1..FILE_COUNT{if let Some((present,data,len))=store.load(i){cache[i]=FileSlot{data,len,present};}}
    let _=store.load(0);
    let mut url=[0;128];let default=b"http://10.0.2.200:8080/hello.exe";url[..default.len()].copy_from_slice(default);
    let auth=Auth::new(&store);let mut ui=Ui{auth,auto_graphics:false,tabs:[crate::http::EMPTY_LINK;4],tab_count:1,tab_index:0,page:unsafe{&mut *ptr::addr_of_mut!(crate::webpage::PAGE)},history:[crate::http::EMPTY_LINK;16],history_len:0,history_pos:0,history_replay:false,web_status:0,web_links:[crate::http::EMPTY_LINK;8],web_link_count:0,web_scroll:0,web_text:false,background:[None;9],display_msg:"Select a supported display mode",package_installed:false,store_msg:"F12 opens Hi5Store",c:canvas,app:None,minimized:false,menu:false,dark:false,ram,nics,files,tasks,mx:fb.width/2,my:fb.height/2,mouse:false,saved:[0;256],saved_valid:false,win,restore:win,maximized:false,drag:None,term:Terminal::new(),sheet:crate::sheet::Sheet::new(),sheet_result:0,sheet_cell:0,sheet_edit:[0;48],sheet_len:0,sheet_msg:"Select cell and type value or formula",pe_report:crate::imports::Report::default(),rebase:crate::relocate::Report::default(),store,note:[0;CAP],note_len:0,dirty:false,save_status:0,ctrl:false,file:0,cache,nic:None,url,url_len:default.len(),web:unsafe{&mut *ptr::addr_of_mut!(WEB_BUFFER)},source:[0;128],source_len:0,web_len:0,web_ok:false,web_msg:"Offline until Go is pressed"};
    ui.mouse=init_input();serial::line(if ui.mouse{"H5K: PS2_MOUSE_ACK"}else{"H5K: PS2_MOUSE_UNAVAILABLE"});
    serial::line(if ui.c.buffered{"H5K: DOUBLE_BUFFER_OK"}else{"H5K: DIRECT_RENDER_FALLBACK"});
    ui.draw();if ui.auth.stage==1{serial::line("H5K: ACCOUNT_SETUP_READY");}else if ui.auth.stage==7{serial::line("H5K: ACCOUNT_LOGIN_READY");}serial::line("H5K: FRAMEBUFFER_OK");serial::line("H5K: DESKTOP_READY");
    p::timer_init();let start=p::ticks();while p::ticks().wrapping_sub(start)<3{p::wait_tick();}
    serial::line("H5K: TIMER_IRQ_OK");if !crate::net::self_test()||!crate::http::navigation_test()||!crate::account::self_test()||!crate::webpage::self_test(){serial::line("H5K: FAIL NETWORK_FORMAT_TEST");crate::kernel::halt();}serial::line("H5K: NET_FORMAT_TEST_OK");serial::line("H5K: BROWSER_LAYOUT_SELFTEST_OK");serial::line("H5K: KERNEL_READY");
    let mut packet=[0u8;3];let mut pos=0;let mut down=false;let mut ext=false;
    loop{match ui.tasks.dispatch(){
        0=>{for _ in 0..32{let st=p::input(0x64);if st&1==0{break;}let b=p::input(0x60);if st&0x20!=0{
            if pos==0&&b&8==0{continue;}packet[pos]=b;pos+=1;if pos==3{pos=0;let flags=packet[0];if flags&0xc0!=0{continue;}let dx=packet[1]as isize-if flags&0x10!=0{256}else{0};let dy=packet[2]as isize-if flags&0x20!=0{256}else{0};let pressed=flags&1!=0;ui.move_pointer(dx,dy,pressed,down);down=pressed;}
        }else if b==0xe0{ext=true;}else{if !ext{ui.key(b);}else if ui.auth.stage!=0{ }else if ui.app==Some(App::WinLive)&&b&0x80==0{let vk=match b{0x48=>38,0x50=>40,0x4b=>37,0x4d=>39,0x47=>36,0x4f=>35,0x49=>33,0x51=>34,0x53=>46,_=>0};if vk!=0{crate::trial::live_key(vk,None);if !crate::trial::live_active(){ui.close();}else{ui.draw();}}}else if ui.app==Some(App::Browser)&&ui.web_text&&b&0x80==0{if b==0x49||b==0x48{ui.web_scroll=ui.web_scroll.saturating_sub(if b==0x49{5}else{1});ui.draw();}else if b==0x51||b==0x50{ui.web_scroll=(ui.web_scroll+if b==0x51{5}else{1}).min(ui.page.rows(ui.win.w.saturating_sub(57)).saturating_sub(1));ui.draw();}}else if ui.app==Some(App::Terminal)&&ui.term.result==24{if b==0x49{ui.term.scroll=ui.term.scroll.saturating_sub(5);ui.draw();}else if b==0x51{let n=crate::trial::output_bytes().iter().filter(|c|**c==10).count();ui.term.scroll=(ui.term.scroll+5).min(n.saturating_sub(1));ui.draw();}}ext=false;}}}
        1=>{if ui.auth.stage==0&&ui.app==Some(App::WinLive){crate::trial::live_tick();if !crate::trial::live_active(){ui.close();}}else{core::hint::spin_loop();}},_=>p::wait_tick(),
    }}
}
