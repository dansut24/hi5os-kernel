//! Hi5Desktop v0.3.4 — native software GUI; single foreground window.
use crate::{boot::Framebuffer,ui::{Canvas,Color},platform as p,services::{Files,Tasks},serial,disk::{Store,CAP,FILE_COUNT,NAMES}};
use core::ptr;
use crate::net::{Nic,MAX_RESPONSE};
use crate::ui::{Palette,palette,ACCENT as BLUE,TEAL};
const WHITE:Color=[255,255,255];const GOLD:Color=[234,178,66];
#[derive(Clone,Copy,PartialEq)]enum App{System,Files,Network,Terminal,Notepad,Browser,Sheet,Store,WinDemo,WinLive,Word}
impl App{fn title(self)->&'static str{match self{Self::System=>"Settings",Self::Files=>"File Explorer",Self::Network=>"Network",Self::Terminal=>"Terminal",Self::Notepad=>"Notepad",Self::Browser=>"Hi5Browser",Self::Sheet=>"Hi5Sheet",Self::Store=>"Hi5Store",Self::WinDemo=>"Windows GUI test",Self::WinLive=>"7-Zip File Manager",Self::Word=>"Hi5Word - Hi5Office"}}}
const APPS:[App;7]=[App::System,App::Files,App::Network,App::Terminal,App::Notepad,App::Browser,App::Word];
#[derive(Clone,Copy)]struct Rect{x:usize,y:usize,w:usize,h:usize}
impl Rect{fn hit(self,x:usize,y:usize)->bool{x>=self.x&&x<self.x+self.w&&y>=self.y&&y<self.y+self.h}}
struct Terminal{buf:[u8;112],len:usize,last:[u8;112],last_len:usize,result:u8,shift:bool,scroll:usize,workspace:bool}
impl Terminal{fn new()->Self{Self{buf:[0;112],len:0,last:[0;112],last_len:0,result:0,shift:false,scroll:0,workspace:false}}}
#[derive(Clone,Copy)]struct FileSlot{data:[u8;CAP],len:usize,present:bool}
const EMPTY_FILE:FileSlot=FileSlot{data:[0;CAP],len:0,present:false};
// The large response buffer is image-owned static memory, not the 64 KiB kernel stack.
static mut WEB_BUFFER:[u8;MAX_RESPONSE]=[0;MAX_RESPONSE];
static mut WEB_ASSET:[u8;MAX_RESPONSE]=[0;MAX_RESPONSE];
static mut WEB_CSS:[u8;1024*1024]=[0;1024*1024];
static mut WORK_BUFFER:[u8;crate::trialfs::SNAP_CAP]=[0;crate::trialfs::SNAP_CAP];
static mut APP_BUFFER:[u8;crate::trial::PACKAGE_SIZE]=[0;crate::trial::PACKAGE_SIZE];
#[derive(Clone,Copy)]struct WindowSlot{app:App,win:Rect,restore:Rect,maximized:bool,minimized:bool}
struct Auth{stage:u8,account:Option<crate::account::Account>,region:u8,dark:bool,name:[u8;32],host:[u8;32],len:usize,host_len:usize,password:[u8;64],pass_len:usize,confirm:[u8;64],confirm_len:usize,delay:u64,msg:&'static str}
impl Auth{fn new(store:&Store)->Self{let (stage,account,msg)=match store.account(){Ok(Some(a))=>(7,Some(a),"Enter your password"),Ok(None)if store.setup_requested()=>(1,None,"Choose regional preferences"),Ok(None)=>(0,None,""),Err(())=>(8,None,"Account data unreadable. Recovery required.")};Self{stage,account,region:account.map_or(0,|a|a.region),dark:account.map_or(stage==1,|a|a.dark),name:[0;32],host:[0;32],len:0,host_len:0,password:[0;64],pass_len:0,confirm:[0;64],confirm_len:0,delay:0,msg}}fn clear_password(&mut self){crate::account::wipe(&mut self.password);crate::account::wipe(&mut self.confirm);self.pass_len=0;self.confirm_len=0;}}
#[derive(Clone,Copy)]struct BrowserHistory{links:[crate::http::Link;16],len:usize,pos:usize}
const EMPTY_HISTORY:BrowserHistory=BrowserHistory{links:[crate::http::EMPTY_LINK;16],len:0,pos:0};
static mut TAB_HISTORIES:[BrowserHistory;4]=[EMPTY_HISTORY;4];
const CACHE_CAP:usize=1536*1024;
#[derive(Clone,Copy)]struct CachedWeb{valid:bool,len:usize,source:crate::http::Link,status:u16,links:[crate::http::Link;8],link_count:usize,scroll:usize,msg:&'static str}
const EMPTY_WEB:CachedWeb=CachedWeb{valid:false,len:0,source:crate::http::EMPTY_LINK,status:0,links:[crate::http::EMPTY_LINK;8],link_count:0,scroll:0,msg:""};
static mut WEB_CACHE:[CachedWeb;4]=[EMPTY_WEB;4];
static mut WEB_CACHE_BYTES:[[u8;CACHE_CAP];4]=[[0;CACHE_CAP];4];
struct Ui{word_clip:alloc::vec::Vec<(u8,u8)>,word:crate::office::Word,word_slot:usize,word_loaded:bool,word_msg:&'static str,word_scroll:usize,auth:Auth,auto_graphics:bool,auto_script:bool,browser_settings:bool,browser_diagnostics:bool,tabs:[crate::http::Link;4],tab_count:usize,tab_index:usize,page:&'static mut crate::webpage::Page,history:[crate::http::Link;16],history_len:usize,history_pos:usize,history_replay:bool,web_status:u16,web_links:[crate::http::Link;8],web_link_count:usize,web_scroll:usize,web_text:bool,background:[Option<WindowSlot>;10],display_msg:&'static str,package_installed:bool,store_msg:&'static str,c:Canvas,app:Option<App>,minimized:bool,menu:bool,dark:bool,ram:u64,nics:u32,files:Files,tasks:Tasks,mx:usize,my:usize,mouse:bool,saved:[u32;256],saved_valid:bool,win:Rect,restore:Rect,maximized:bool,drag:Option<(usize,usize)>,term:Terminal,sheet:crate::sheet::Sheet,sheet_result:i64,sheet_cell:usize,sheet_edit:[u8;48],sheet_len:usize,sheet_msg:&'static str,pe_report:crate::imports::Report,rebase:crate::relocate::Report,store:Store,note:[u8;CAP],note_len:usize,dirty:bool,save_status:u8,ctrl:bool,file:usize,cache:[FileSlot;FILE_COUNT],nic:Option<Nic>,url:[u8;128],url_len:usize,web:&'static mut [u8;MAX_RESPONSE],source:[u8;128],source_len:usize,web_len:usize,web_ok:bool,web_msg:&'static str}
// Single-CPU cooperative input pump. Only called from the desktop or its
// synchronous browser work, never from interrupt context. No nested UI actions.
struct PointerPacket{bytes:[u8;3],pos:usize,down:bool}
static mut POINTER_PACKET:PointerPacket=PointerPacket{bytes:[0;3],pos:0,down:false};
fn mouse_byte(b:u8)->Option<(isize,isize,bool,bool)>{
    let p=unsafe{&mut *ptr::addr_of_mut!(POINTER_PACKET)};
    if p.pos==0&&b&8==0{return None;}p.bytes[p.pos]=b;p.pos+=1;if p.pos!=3{return None;}p.pos=0;
    let flags=p.bytes[0];if flags&0xc0!=0{return None;}
    let dx=p.bytes[1]as isize-if flags&0x10!=0{256}else{0};let dy=p.bytes[2]as isize-if flags&0x20!=0{256}else{0};
    let pressed=flags&1!=0;let was=p.down;p.down=pressed;Some((dx,dy,pressed,was))
}
struct BusyCursor{canvas:Canvas,x:usize,y:usize,visible:bool,cancelled:bool,moved:bool,last:u64,ext:bool}
static mut BUSY_CURSOR:Option<BusyCursor>=None;
fn busy_hide(b:&BusyCursor){if b.visible&&b.canvas.buffered{for y in 0..16{for x in 0..16{b.canvas.restore_pixel(b.x+x,b.y+y);}}}}
fn busy_show(b:&BusyCursor){if b.visible&&b.canvas.buffered{for y in 0..14{for x in 0..=y/2{b.canvas.front_pixel(b.x+x,b.y+y,if x==0||x==y/2||y==13{[22,32,43]}else{WHITE});}}}}
// Returns true if Escape cancelled this operation. Other clicks/keys are
// deliberately consumed rather than replayed against a different document.
pub fn browser_poll()->bool{
    let state=unsafe{&mut *ptr::addr_of_mut!(BUSY_CURSOR)};let Some(b)=state.as_mut()else{return false;};
    let now=p::ticks();if now==b.last{return b.cancelled;}b.last=now;
    for _ in 0..64{let st=p::input(0x64);if st&1==0{break;}let v=p::input(0x60);
        if st&0x20!=0{if let Some((dx,dy,_,_))=mouse_byte(v){if dx!=0||dy!=0{busy_hide(b);b.visible=true;b.x=(b.x as isize+dx).clamp(0,b.canvas.fb.width as isize-1)as usize;b.y=(b.y as isize-dy).clamp(0,b.canvas.fb.height as isize-1)as usize;busy_show(b);if !b.moved{b.moved=true;serial::line("H5K: BROWSER_BUSY_POINTER_MOVED");}}}}
        else if v==0xe0{b.ext=true;}else{if !b.ext&&v==0x01&&!b.cancelled{b.cancelled=true;serial::line("H5K: BROWSER_CANCEL_REQUESTED");}b.ext=false;}
    }b.cancelled
}
impl Ui{
    fn browser_busy_begin(&mut self){
        // Drawing is complete before this snapshot; no back-buffer mutation
        // or desktop dispatch until browser_busy_end restores the cursor.
        let c=Canvas{fb:self.c.fb,physical:self.c.physical,scale:self.c.scale,buffered:self.c.buffered};
        unsafe{*ptr::addr_of_mut!(BUSY_CURSOR)=Some(BusyCursor{canvas:c,x:self.mx,y:self.my,visible:self.mouse,cancelled:false,moved:false,last:u64::MAX,ext:false});}
        serial::line("H5K: BROWSER_BUSY_BEGIN");
    }
    fn browser_busy_end(&mut self)->bool{
        browser_poll();let state=unsafe{(&mut *ptr::addr_of_mut!(BUSY_CURSOR)).take()};
        if let Some(b)=state{busy_hide(&b);self.mx=b.x;self.my=b.y;self.mouse=b.visible;self.saved_valid=false;self.ctrl=false;self.term.shift=false;self.cursor_show();serial::line("H5K: BROWSER_BUSY_END");b.cancelled}else{false}
    }
    fn cursor_hide(&mut self){if !self.saved_valid{return;}for y in 0..16{for x in 0..16{self.c.restore_pixel(self.mx+x,self.my+y);}}self.saved_valid=false;}
    fn cursor_show(&mut self){if !self.mouse{return;}for y in 0..14{for x in 0..=y/2{self.c.front_pixel(self.mx+x,self.my+y,if x==0||x==y/2||y==13{[22,32,43]}else{WHITE});}}self.saved_valid=true;}
    fn bar_y(&self)->usize{self.c.fb.height.saturating_sub(52)}
    fn full_window(&self)->Rect{Rect{x:6,y:6,w:self.c.fb.width.saturating_sub(12),h:self.bar_y().saturating_sub(12)}}
    fn save_display(&mut self){if let Some(i)=crate::display::MODES.iter().position(|m|m.0==self.c.physical.width&&m.1==self.c.physical.height){if self.store.save_display(i,self.c.scale){serial::line("H5K: DISPLAY_SETTINGS_SAVED");}}}
    fn reflow(&mut self){let fb=self.c.fb;self.mx=self.mx.min(fb.width-1);self.my=self.my.min(fb.height-1);self.saved_valid=false;self.drag=None;let w=560.min(fb.width.saturating_sub(108));let h=365.min(fb.height.saturating_sub(66));self.restore=Rect{x:(fb.width-w)/2+20,y:(fb.height-52-h)/2,w,h};self.win=if self.maximized{self.full_window()}else{self.restore};let full=self.full_window();let restore=self.restore;for slot in self.background.iter_mut().flatten(){slot.restore=restore;slot.win=if slot.maximized{full}else{restore};}}
    fn apply_display(&mut self,i:usize){self.cursor_hide();if let Some(fb)=crate::display::set_mode(self.c.physical,i){self.c=Canvas::scaled(fb,self.c.scale);self.reflow();self.save_display();self.display_msg="Resolution applied";self.draw();serial::metric("H5K: GRAPHICS_WIDTH=",fb.width as u64);serial::metric("H5K: GRAPHICS_HEIGHT=",fb.height as u64);serial::line("H5K: GRAPHICS_MODE_OK");}else{self.display_msg="Mode unavailable - previous resolution retained";self.draw();serial::line("H5K: GRAPHICS_MODE_UNAVAILABLE");}}
    fn apply_scale(&mut self,scale:usize){if !Canvas::supported(self.c.physical,scale){self.display_msg="Scale needs a larger resolution";self.draw();return;}self.cursor_hide();self.c=Canvas::scaled(self.c.physical,scale);self.reflow();self.save_display();self.display_msg="Interface scale applied";self.draw();serial::metric("H5K: DISPLAY_SCALE=",scale as u64);}
    fn park_window(&mut self){if let Some(a)=self.app{if a==App::WinLive{return;}let slot=WindowSlot{app:a,win:self.win,restore:self.restore,maximized:self.maximized,minimized:self.minimized};if let Some(i)=self.background.iter().position(|s|s.is_none()){self.background[i]=Some(slot);}}}
    fn take_window(&mut self,a:App)->Option<WindowSlot>{let i=self.background.iter().position(|s|s.is_some_and(|v|v.app==a))?;let out=self.background[i];for j in i..9{self.background[j]=self.background[j+1];}self.background[9]=None;out}
    fn dock_x(&self)->usize{self.c.fb.width.saturating_sub(350)/2}
    fn menu_rect(&self)->Rect{Rect{x:8,y:self.bar_y().saturating_sub(398),w:270,h:392}}
    fn button(&self,r:Rect,t:&str,fill:Color,ink:Color){self.c.control(r.x,r.y,r.w,r.h,t,fill,ink);}
    fn icon(&self,a:App,x:usize,y:usize){let c=&self.c;
        let tile=match a{App::System=>[38,103,169],App::Files=>[176,117,34],App::Network=>[17,119,133],App::Terminal=>[19,38,56],App::Notepad=>[90,112,146],App::Browser=>[16,128,142],App::Word=>[28,88,180],App::Sheet=>[24,123,83],_=>[61,88,143]};
        c.round(x,y,34,34,9,tile);
        match a{
            App::System=>{c.round(x+5,y+6,24,17,3,WHITE);c.rect(x+7,y+8,20,12,tile);c.rect(x+15,y+23,4,4,WHITE);c.rect(x+10,y+27,14,2,WHITE);}
            App::Files=>{c.round(x+5,y+11,24,17,3,[255,219,143]);c.round(x+5,y+7,13,9,3,GOLD);c.rect(x+7,y+14,20,2,[255,239,193]);}
            App::Network=>{for i in 0..3{let h=7+i*5;c.round(x+6+i*8,y+28-h,5,h,2,WHITE);}}
            App::Terminal=>{c.text(x+6,y+5,">_",TEAL);}
            App::Notepad|App::Word=>{c.round(x+8,y+4,18,26,3,WHITE);if a==App::Word{c.text(x+10,y+6,"W",tile);}else{for i in 0..4{c.rect(x+11,y+9+i*4,12,1,tile);}}}
            App::Browser=>{c.round(x+5,y+5,24,24,12,WHITE);c.round(x+8,y+8,18,18,9,tile);c.rect(x+6,y+16,22,2,WHITE);c.rect(x+16,y+7,2,20,WHITE);}
            App::Sheet=>{c.round(x+6,y+6,22,22,3,WHITE);for i in 1..3{c.rect(x+6+i*7,y+6,1,22,tile);c.rect(x+6,y+6+i*7,22,1,tile);}}
            _=>{c.round(x+7,y+11,20,17,3,WHITE);c.round(x+12,y+6,10,8,3,WHITE);c.round(x+14,y+8,6,5,2,tile);}
        }
    }
    fn draw(&mut self){
        if self.auth.stage!=0{self.draw_auth();return;}
        self.saved_valid=false;
        let c=&self.c;let w=c.fb.width;let h=c.fb.height;let q=palette(self.dark);let by=self.bar_y();
        c.aurora(by,self.dark);
        let bx=w.saturating_sub(220);c.brand(bx,18,TEAL);c.title(bx+42,17,"Hi5OS",q.ink);
        c.text(w.saturating_sub(220),58,"Business desktop preview",q.muted);
        if w>=900&&self.app.is_none()&&self.background.iter().all(|s|s.is_none()||s.is_some_and(|v|v.minimized)){
            let cx=w-258;c.panel(cx,102,238,128,14,q);c.text(cx+14,114,"Device overview",q.ink);
            let connected=self.nic.as_ref().is_some_and(|n|n.config().0!=[0;4]);
            c.round(cx+14,148,8,8,4,if connected{q.green}else{q.muted});
            c.text(cx+30,140,if connected{"IPv4 connected"}else{"Network offline"},q.muted);
            c.text(cx+14,166,if self.store.attached(){"Data disk attached"}else{"RAM workspace"},q.muted);
            c.text(cx+14,191,"Hi5Central: not enrolled",q.muted);
        }
        for(i,a)in APPS.iter().enumerate(){let y=8+i*61;self.icon(*a,31,y);let label=match a{App::System=>"This PC",App::Files=>"Files",App::Network=>"Network",App::Terminal=>"Terminal",App::Notepad=>"Notepad",App::Browser=>"Browser",App::Sheet=>"Sheet",App::Store=>"Store",App::WinDemo=>"WinDemo",App::WinLive=>"Windows",App::Word=>"Hi5Word"};c.text(18,y+35,label,q.ink);}
        if by>438{self.icon(App::Sheet,111,374);c.text(105,409,"Sheet",q.ink);}
        let saved=self.win;for i in 0..10{if let Some(s)=self.background[i]{if !s.minimized{self.win=s.win;self.draw_window(s.app,q);}}}self.win=saved;if let Some(a)=self.app{if !self.minimized{self.draw_window(a,q);}}
        c.rect(0,by,w,h-by,q.bar);c.rect(0,by,w,1,q.edge);c.round(self.dock_x().saturating_sub(4),by+3,358,46,12,q.panel);
        c.round(8,by+8,78,36,10,BLUE);c.brand(12,by+10,TEAL);c.text_limit(47,by+13,"Start",WHITE,false,84);
        let dock=self.dock_x();
        for(i,a)in APPS.iter().enumerate(){let x=dock+i*50;if self.app==Some(*a)||self.background.iter().any(|s|s.is_some_and(|v|v.app==*a)){c.round(x,by+5,46,42,7,q.sub);c.round(x+14,by+47,18,3,1,TEAL);}self.icon(*a,x+6,by+10);}
        c.text(w.saturating_sub(100),by+6,if self.nic.as_ref().map_or(false,|n|n.config().0!=[0;4]){"IPv4 ready"}else{"Offline"},q.muted);c.text(w.saturating_sub(100),by+25,"Hi5OS",q.ink);
        if self.menu{let r=self.menu_rect();c.panel(r.x,r.y,r.w,r.h,16,q);c.brand(r.x+14,r.y+9,TEAL);c.title(r.x+58,r.y+8,"Apps",q.ink);c.rect(r.x+14,r.y+43,r.w-28,1,q.edge);
            for(i,a)in APPS.iter().enumerate(){let y=r.y+49+i*40;if self.app==Some(*a){c.round(r.x+8,y,r.w-16,36,8,q.sub);}self.icon(*a,r.x+14,y);c.text_limit(r.x+60,y+6,a.title(),q.ink,false,r.x+r.w-12);}
            self.button(Rect{x:r.x+12,y:r.y+338,w:r.w-24,h:38},if self.dark{"Light appearance (T)"}else{"Dark appearance (T)"},q.sub,q.ink);
        }
        c.present();self.cursor_show();
    }
    fn draw_auth(&mut self){self.saved_valid=false;let c=&self.c;let q=palette(self.auth.dark);let w=c.fb.width;let h=c.fb.height;let x=(w-520)/2;let y=(h-400)/2;c.aurora(h,self.auth.dark);c.panel(x,y,520,400,20,q);c.brand(x+24,y+16,TEAL);c.title(x+68,y+16,if self.auth.stage==7{"Welcome to Hi5OS"}else{"Set up Hi5OS"},q.ink);c.rect(x+24,y+54,472,1,q.edge);
        let a=&self.auth;let (heading,hint,value)=match a.stage{1=>("1 / 6  Language and region","1: English (UK)   2: English (US)",if a.region==0{"English (UK)"}else{"English (US)"}),2=>("2 / 6  Local username","Letters, digits, hyphen or underscore",core::str::from_utf8(&a.name[..a.len]).unwrap_or("")),3=>("3 / 6  Computer name","Letters, digits, hyphen or underscore",core::str::from_utf8(&a.host[..a.host_len]).unwrap_or("")),4=>("4 / 6  Create password","Use at least 12 characters (max 64)",""),5=>("5 / 6  Confirm password","Enter the same password again",""),6=>("6 / 6  Appearance","1: Light   2: Dark",if a.dark{"Dark"}else{"Light"}),7=>("Local account",a.account.as_ref().map_or("",|v|v.username()),""),_=>("Recovery required","Account data cannot be verified","")};
        c.title(x+24,y+68,heading,q.ink);c.text_limit(x+24,y+112,hint,q.muted,false,x+496);c.round(x+24,y+152,472,52,9,q.edge);c.round(x+25,y+153,470,50,8,q.sub);if a.stage==7{c.round(x+456,y+72,38,38,19,BLUE);c.round(x+469,y+79,12,12,6,WHITE);c.round(x+464,y+94,22,10,5,WHITE);}
        if matches!(a.stage,4|5|7){let n=if a.stage==5{a.confirm_len}else{a.pass_len};let stars=[b'*';64];c.text_limit(x+36,y+165,core::str::from_utf8(&stars[..n]).unwrap_or(""),q.ink,false,x+488);}else{c.text_limit(x+36,y+165,value,q.ink,false,x+488);}
        c.text_limit(x+24,y+226,a.msg,q.muted,false,x+496);c.text_limit(x+24,y+256,"Local account - disk is not encrypted",q.muted,false,x+496);if (1..=6).contains(&a.stage){for i in 0..6{c.round(x+24+i*79,y+291,72,4,2,if i<a.stage as usize{TEAL}else{q.edge});}}else if a.stage==7{c.text_limit(x+24,y+281,"Hi5Central enrolment not available yet",q.muted,false,x+496);}
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
    fn lock_account(&mut self){if self.auth.account.is_none(){return;}if self.app==Some(App::WinLive){crate::trial::live_close();self.app=None;}self.menu=false;self.drag=None;self.word_clip.fill((0,0));self.word_clip.clear();self.auth.clear_password();self.auth.stage=7;self.auth.msg="Enter your password";self.auth.delay=0;serial::line("H5K: ACCOUNT_LOCKED");self.draw();}
    fn draw_window(&self,a:App,q:Palette){
        let c=&self.c;let r=self.win;
        c.panel(r.x,r.y,r.w,r.h,14,q);
        c.round(r.x+1,r.y+1,r.w-2,42,13,q.bar);c.rect(r.x+1,r.y+35,r.w-2,8,q.bar);c.rect(r.x+1,r.y+42,r.w-2,1,q.edge);
        self.icon(a,r.x+8,r.y+4);c.text_limit(r.x+52,r.y+11,a.title(),q.ink,false,r.x+r.w-116);
        c.window_control(r.x+r.w-108,r.y+5,0,q);
        c.window_control(r.x+r.w-73,r.y+5,1,q);
        c.window_control(r.x+r.w-38,r.y+5,2,q);
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
                c.title(x,y,"Display & appearance",q.ink);c.rect(x,y+32,r.w-44,1,q.edge);
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
                c.title(x,y,if self.store.attached(){"Hi5OS Data"}else{"RAM workspace"},q.ink);c.rect(x,y+32,r.w-44,1,q.edge);
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
                let tw=self.tab_width();
                for i in 0..self.tab_count{let tx=x+i*tw;let selected=i==self.tab_index;c.round(tx,y,tw-4,28,5,if selected{BLUE}else{q.sub});let u=&self.tabs[i];let label=if u.len==0{"New tab"}else{let raw=&u.url[..u.len];let host=raw.strip_prefix(b"https://").or_else(||raw.strip_prefix(b"http://")).unwrap_or(raw);let host=host.split(|v|matches!(*v,b'/'|b'?')).next().unwrap_or(host);core::str::from_utf8(host).unwrap_or("Website")};let ink=if selected{WHITE}else{q.ink};c.text_limit(tx+5,y+4,label,ink,false,tx+tw-25);c.text(tx+tw-20,y+4,"x",ink);}
                self.button(Rect{x:x+self.tab_count*tw,y,w:35,h:28},"+",q.sub,q.ink);
                for (ox,w,label) in [(0,35,"<"),(39,35,">"),(78,65,"Reload"),(147,59,"Home"),(210,81,"Graphics"),(295,80,"Settings")]{self.button(Rect{x:x+ox,y:y+33,w,h:30},label,q.sub,q.ink);}
                if r.w>=540{self.button(Rect{x:x+379,y:y+33,w:90,h:30},"Downloads",q.sub,q.ink);}
                c.round(x,y+69,r.w-132,32,5,q.sub);
                let https=self.url[..self.url_len].starts_with(b"https://");c.text(x+5,y+74,if https{"HTTPS"}else{"URL"},q.muted);c.rect(x+66,y+76,1,18,q.edge);
                if let Ok(t)=core::str::from_utf8(&self.url[..self.url_len]){c.text_limit(x+76,y+74,t,q.ink,false,right-90);}
                self.button(Rect{x:r.x+r.w-95,y:y+69,w:72,h:32},"Go",BLUE,WHITE);
                c.text_limit(x,y+106,self.web_msg,q.muted,false,right);
                let top=y+130;let floor=r.y+r.h-25;
                if self.browser_settings{
                    c.rect(x,top,right.saturating_sub(x),floor.saturating_sub(top),q.sub);c.text_limit(x+10,top+3,"Browser settings",q.ink,false,right);
                    for (i,label,on) in [(0,"Automatic images and CSS",self.auto_graphics),(1,"Experimental JavaScript",self.auto_script),(2,"Show technical diagnostics",self.browser_diagnostics)]{let ty=top+27+i*32;if ty+29<=floor{c.text_limit(x+10,ty+5,label,q.ink,false,right-88);self.button(Rect{x:right-78,y:ty,w:68,h:27},if on{"On"}else{"Off"},if on{BLUE}else{q.panel},if on{WHITE}else{q.ink});}}
                    if top+151<=floor{c.text_limit(x+10,top+127,"JavaScript: bounded scripts and modules; session opt-in.",q.muted,false,right);}
                }else if self.web_ok{
                    let mut top=top;if self.browser_diagnostics{c.text(x,top,"HTTP",q.muted);c.num(x+48,top,self.web_status as u64,q.ink);c.text(x+100,top,"Bytes",q.muted);c.num(x+155,top,self.web_len as u64,q.ink);c.text(x+252,top,"DOM",q.muted);c.num(x+298,top,self.page.dom_nodes as u64,q.ink);top+=24;}
                    let kind=match crate::compat::classify(&self.web[..self.web_len]){crate::compat::Kind::Pe64=>"PE64 executable candidate",crate::compat::Kind::Pe32=>"PE32 executable candidate",crate::compat::Kind::Compound=>"OLE compound-file candidate",_=>"Text or unknown binary"};
                    if self.web_text{
                        let w=right.saturating_sub(x+13);c.rect(x,top,w,floor.saturating_sub(top),WHITE);
                        self.page.draw(c,x,top+3,x+w,floor,self.web_scroll,[30,34,40],[235,238,242]);
                        let rows=self.page.rows(w).max(1);let h=floor.saturating_sub(top);if h>20{c.rect(right-10,top,9,h,q.sub);let thumb=(h*6/rows).clamp(14,h);let offset=(h-thumb)*self.web_scroll.min(rows-1)/rows.saturating_sub(1).max(1);c.round(right-10,top+offset,9,thumb,3,BLUE);}
                    }else if let Some(bitmap)=crate::webpage::Bitmap::parse(&self.web[..self.web_len]){bitmap.draw(c,x,top+3,r.w-44,floor.saturating_sub(top+6));}else{
                        c.text_limit(x,top+2,kind,q.ink,false,right);c.text_limit(x,top+26,"Binary preview - execution disabled",q.muted,false,right);
                        let src=&self.source[..self.source_len];let path=src.split(|b|*b==b'?').next().unwrap_or(&[]);let name=path.rsplit(|b|*b==b'/').next().unwrap_or(&[]);if let Ok(t)=core::str::from_utf8(name){c.text_limit(x,top+50,t,q.ink,false,right);}
                        if self.web_status==200&&top+108<=floor{self.button(Rect{x,y:top+74,w:180,h:30},"Save download (F9)",q.sub,q.ink);}
                    }
                }else{
                    c.rect(x,top,right.saturating_sub(x),floor.saturating_sub(top),q.sub);c.text_limit(x+12,top+12,"Hi5Browser - enter a website address",q.ink,false,right);
                    if top+76<=floor{self.button(Rect{x:x+12,y:top+40,w:110,h:30},"Google",BLUE,WHITE);c.text_limit(x+132,top+45,"Ctrl+L address | Ctrl+S settings",q.muted,false,right);}
                }
            }
            App::Word=>self.word_draw(q),
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
    fn select(&mut self,a:App){if self.app!=Some(a){let previous=self.app;if previous==Some(App::WinLive){crate::trial::live_close();}else{self.park_window();}if let Some(s)=self.take_window(a){self.win=s.win;self.restore=s.restore;self.maximized=s.maximized;}else{self.win=self.restore;self.win.x=(self.win.x+24).min(self.c.fb.width.saturating_sub(self.win.w));self.win.y=(self.win.y+18).min(self.bar_y().saturating_sub(self.win.h));self.restore=self.win;self.maximized=false;}serial::line("H5K: GUI_WINDOW_FOCUSED");}if a==App::WinLive&&!self.maximized{self.restore=self.win;self.win=self.full_window();self.maximized=true;}if a==App::WinDemo{self.store_msg=if crate::trial::run_gui_fixture(){"PE/DLL and Win32 drawing fixture passed"}else{"PE GUI fixture failed"};}if a==App::Store{let b=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};self.package_installed=self.store.load_7zip(b).is_some();}if a==App::Notepad&&!self.dirty{self.load_note();}if a==App::Word&&!self.word_loaded{self.word_open(0);self.word_loaded=true;}self.app=Some(a);self.minimized=false;self.menu=false;serial::line(match a{App::System=>"H5K: GUI_SYSTEM_OPEN",App::Files=>"H5K: GUI_FILES_OPEN",App::Network=>"H5K: GUI_NETWORK_OPEN",App::Terminal=>"H5K: GUI_TERMINAL_OPEN",App::Notepad=>"H5K: GUI_NOTEPAD_OPEN",App::Browser=>"H5K: GUI_BROWSER_OPEN",App::Sheet=>"H5K: GUI_SHEET_OPEN",App::Store=>"H5K: GUI_STORE_OPEN",App::WinDemo=>"H5K: GUI_WINDEMO_OPEN",App::WinLive=>"H5K: GUI_WINLIVE_OPEN",App::Word=>"H5K: GUI_WORD_OPEN"});self.draw();}
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
    fn word_open(&mut self,slot:usize){
        if self.word.dirty{self.word_msg="Save current changes before opening another slot";return;}
        let mut b=alloc::vec![0u8;20+crate::office::MAX*2];
        if let Some(n)=self.store.load_office(slot,false,&mut b){if let Some(w)=crate::office::Word::decode(&b[..n]){self.word_slot=slot;self.word_scroll=0;self.word=w;self.word_msg="Native document opened";serial::line("H5K: OFFICE_WORD_LOADED");serial::metric("H5K: OFFICE_WORD_CRC=",crate::office::crc(&b[..n-4])as u64);return;}self.word_msg="Invalid native document";return;}
        self.word_slot=slot;self.word_scroll=0;self.word=crate::office::Word::new();self.word_msg="New document";
    }
    fn word_save(&mut self){let data=self.word.encode();if self.store.save_office(self.word_slot,false,&data){self.word.dirty=false;self.word_msg="Saved .h5doc";serial::line("H5K: OFFICE_WORD_SAVED");serial::metric("H5K: OFFICE_WORD_CRC=",crate::office::crc(&data[..data.len()-4])as u64);}else{self.word_msg="Save failed - data disk required";}}
    fn word_export(&mut self){if let Some(data)=self.word.docx(){if self.store.save_office(self.word_slot,true,&data){self.word_msg="DOCX export saved in matching slot";serial::line("H5K: OFFICE_DOCX_SAVED");return;}}self.word_msg="DOCX export failed - size or disk limit";}
    fn word_import(&mut self){
        if self.word.dirty{self.word_msg="Save changes before importing";return;}
        let mut data=alloc::vec![0u8;crate::disk::DL_CAP];let Some(r)=self.store.load_download(&mut data)else{self.word_msg="No saved browser download";return;};
        let path=&r.url[..r.url_len];let file=path.split(|b|*b==b'?').next().unwrap_or(path);
        let w=if file.ends_with(b".h5doc"){crate::office::Word::decode(&data[..r.len])}else if file.ends_with(b".txt"){crate::office::Word::text_import(&data[..r.len])}else{None};
        if let Some(mut w)=w{w.dirty=true;self.word=w;self.word_scroll=0;self.word_msg="Imported - save to native slot";}else{self.word_msg="Import supports ASCII TXT and valid .h5doc only";}
    }
    fn word_key(&mut self,key:u8){
        use crate::office::{BOLD,ITALIC,UNDER,LARGE};
        if self.ctrl{match key{0x1f=>self.word_save(),0x12=>self.word_export(),0x17 if self.term.shift=>self.word_import(),0x30=>self.word.toggle_style(BOLD),0x16=>self.word.toggle_style(UNDER),0x26=>self.word.alignment(0),0x2e if self.term.shift=>self.word.alignment(1),0x13=>self.word.alignment(2),0x17=>self.word.toggle_style(ITALIC),0x14=>self.word.toggle_style(ITALIC),0x22=>self.word.toggle_style(LARGE),0x2c=>self.word.undo(),0x15=>self.word.redo(),0x31=>self.word.clear(),0x1e=>self.word.select_all(),0x2e=>{if let Some(v)=self.word.copy(){self.word_clip=v;self.word_msg="Selection copied";}},0x2d=>{if let Some(v)=self.word.cut(){self.word_clip=v;self.word_msg="Selection cut";}},0x2f=>{if self.word.paste(&self.word_clip){self.word_msg="Selection pasted";}},0x02..=0x05=>self.word_open((key-2)as usize),_=>{}}}
        else if key==0x0e{self.word.backspace();}else if key==0x1c{if !self.word.insert(10){self.word_msg="Document size limit";}}else if let Some(b)=ascii(key,self.term.shift){if !self.word.insert(b){self.word_msg="Document size limit";}}
        self.word_follow();self.draw();
    }
    fn word_extended(&mut self,key:u8){let extend=self.term.shift;let cur=self.word.cursor;let target=match key{0x4b=>Some(if !extend{self.word.selection().map_or(cur.saturating_sub(1),|v|v.0)}else{cur.saturating_sub(1)}),0x4d=>Some(if !extend{self.word.selection().map_or((cur+1).min(self.word.len()),|v|v.1)}else{(cur+1).min(self.word.len())}),0x47=>Some(0),0x4f=>Some(self.word.len()),0x53=>{self.word.delete();None},0x48|0x50=>{let rows=self.word_rows();let i=self.word_row_index(&rows);let col=cur-rows[i].0;let next=if key==0x48{i.saturating_sub(1)}else{(i+1).min(rows.len()-1)};Some((rows[next].0+col).min(rows[next].1))},0x49=>{self.word_scroll=self.word_scroll.saturating_sub(4);None},0x51=>{self.word_scroll=(self.word_scroll+4).min(self.word_rows().len()-1);None},_=>None};if let Some(pos)=target{self.word.move_to(pos,extend);}if !matches!(key,0x49|0x51){self.word_follow();}self.draw();}
    fn word_paper(&self)->Rect{let r=self.win;let w=(r.w.saturating_sub(64)).min(700);Rect{x:r.x+(r.w-w)/2,y:r.y+153,w,h:r.h.saturating_sub(186)}}
    fn word_advance(&self,i:usize)->usize{let(b,s)=self.word.at(i);Canvas::measure_text(&[b],s&crate::office::LARGE!=0)+if s&crate::office::BOLD!=0{1}else{0}}
    fn word_rows(&self)->alloc::vec::Vec<(usize,usize,usize)>{let width=self.word_paper().w.saturating_sub(32).max(1);let mut rows=alloc::vec::Vec::new();let mut start=0;let mut i=0;let mut used=0;while i<self.word.len(){if self.word.at(i).0==10{rows.push((start,i,used));i+=1;start=i;used=0;continue;}let adv=self.word_advance(i);if i>start&&used+adv>width{let split=(start..i).rev().find(|&j|self.word.at(j).0==b' ').map(|j|j+1).filter(|&j|j>start&&j<i).unwrap_or(i);let left=(start..split).map(|j|self.word_advance(j)).sum();rows.push((start,split,left));start=split;used=(start..i).map(|j|self.word_advance(j)).sum();}used+=adv;i+=1;}rows.push((start,i,used));rows}
    fn word_row_index(&self,rows:&[(usize,usize,usize)])->usize{rows.iter().rposition(|r|r.0<=self.word.cursor).unwrap_or(0)}
    fn word_follow(&mut self){let rows=self.word_rows();let row=self.word_row_index(&rows);let visible=(self.word_paper().h.saturating_sub(20)/34).max(1);if row<self.word_scroll{self.word_scroll=row;}else if row>=self.word_scroll+visible{self.word_scroll=row+1-visible;}self.word_scroll=self.word_scroll.min(rows.len()-1);}
    fn word_row_x(&self,p:Rect,width:usize)->usize{let usable=p.w.saturating_sub(32);p.x+16+match self.word.align{1=>usable.saturating_sub(width)/2,2=>usable.saturating_sub(width),_=>0}}
    fn word_draw(&self,q:Palette){let c=&self.c;let r=self.win;let x=r.x+22;let y=r.y+58;let right=r.x+r.w-14;
        c.text_limit(x,y,crate::office::NAMES[self.word_slot],q.ink,false,right);if self.word.dirty{c.text_limit(x+180,y,"Unsaved",q.muted,false,right);}c.round(x-6,y+18,r.w-32,68,9,q.bar);
        for(i,label)in ["New","Save","DOCX","Open 1","2","3","4"].iter().enumerate(){let bx=x+i*62;if bx+59<=right{self.button(Rect{x:bx,y:y+22,w:59,h:27},label,if i==1{BLUE}else{q.sub},if i==1{WHITE}else{q.ink});}}
        for(i,label)in ["B","I","U","18pt","Left","Center","Right","Undo","Redo"].iter().enumerate(){let bx=x+i*48;if bx+46<=right{let on=if i<4{self.word.style&(1<<i)!=0}else if i<7{self.word.align as usize==i-4}else{false};self.button(Rect{x:bx,y:y+53,w:46,h:28},label,if on{BLUE}else{q.sub},if on{WHITE}else{q.ink});}}
        let paper=self.word_paper();c.round(x-7,paper.y-7,r.w-30,paper.h+14,8,q.sub);c.rect(paper.x+3,paper.y+4,paper.w,paper.h,q.edge);c.rect(paper.x,paper.y,paper.w,paper.h,WHITE);
        let rows=self.word_rows();let cursor_row=self.word_row_index(&rows);let floor=paper.y+paper.h;for (ri,&(start,end,width))in rows.iter().enumerate().skip(self.word_scroll){let py=paper.y+10+(ri-self.word_scroll)*34;if py+32>floor{break;}let mut px=self.word_row_x(paper,width);for i in start..end{if self.word.selection().is_some_and(|(a,b)|a<=i&&i<b){c.rect(px,py+1,self.word_advance(i).max(1),32,[192,218,247]);}if ri==cursor_row&&i==self.word.cursor{c.rect(px,py+3,1,25,BLUE);}let(b,s)=self.word.at(i);c.office_glyph(px,py,b,s,[27,43,61],paper.x+paper.w-10,floor);px+=self.word_advance(i);}if ri==cursor_row&&self.word.cursor==end{c.rect(px,py+3,1,25,BLUE);}}
        let sy=r.y+r.h-27;c.rect(x,sy-5,r.w-44,1,q.edge);c.text_limit(x,sy,self.word_msg,q.muted,false,right.saturating_sub(105));c.num(right.saturating_sub(100),sy,self.word.word_count()as u64,q.ink);c.text(right.saturating_sub(62),sy,"words",q.muted);
    }
    fn word_click(&mut self,x:usize,y:usize){let r=self.win;let bx=r.x+22;let yy=r.y+58;if x>=bx&&y>=yy+22&&y<yy+49{match(x-bx)/62{0=>self.word.clear(),1=>self.word_save(),2=>self.word_export(),3..=6=>self.word_open((x-bx)/62-3),_=>{}}}else if x>=bx&&y>=yy+53&&y<yy+81{match(x-bx)/48{0..=3=>self.word.toggle_style(1<<((x-bx)/48)),4..=6=>self.word.alignment(((x-bx)/48-4)as u8),7=>self.word.undo(),8=>self.word.redo(),_=>{}}}else{let p=self.word_paper();if p.hit(x,y){let rows=self.word_rows();let ri=(self.word_scroll+y.saturating_sub(p.y+10)/34).min(rows.len()-1);let(start,end,width)=rows[ri];let mut px=self.word_row_x(p,width);let mut pos=end;for i in start..end{let adv=self.word_advance(i);if x<px+adv/2{pos=i;break;}px+=adv;}self.word.move_to(pos,self.term.shift);}}self.word_follow();self.draw();}

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
    fn classify_web(&mut self){self.page.clear();self.web_scroll=0;self.web_link_count=0;self.web_text=self.web_ok&&self.web[..self.web_len].iter().all(|b|*b==9||*b==10||*b==13||*b>=32)&&core::str::from_utf8(&self.web[..self.web_len]).is_ok();if self.web_text{self.page.parse_at_width(&self.web[..self.web_len],&self.source[..self.source_len],&[],self.win.w.saturating_sub(57));serial::metric("H5K: BROWSER_LAYOUT_BLOCKS=",self.page.count as u64);serial::metric("H5K: WEB_CORE_DOM_NODES=",self.page.dom_nodes as u64);self.web_document_diagnostics();if self.page.form.ready{serial::line("H5K: WEB_CORE_FORM_READY");}serial::metric("H5K: BROWSER_STYLE_BLOCKS=",self.page.styled as u64);self.web_link_count=crate::http::links(&self.web[..self.web_len],&self.source[..self.source_len],&mut self.web_links);serial::line("H5K: BROWSER_PAGE_RENDERED");serial::metric("H5K: BROWSER_LINKS=",self.web_link_count as u64);}else if self.web_ok&&crate::webpage::Bitmap::parse(&self.web[..self.web_len]).is_some(){serial::line("H5K: BROWSER_BITMAP_RENDERED");}}
    fn web_document_diagnostics(&mut self){serial::metric("H5K: WEB_CORE_SCRIPTS=",self.page.scripts as u64);if self.page.requires_js{self.web_msg="Website requires JavaScript - search results not supplied";serial::line("H5K: WEB_CORE_JS_REQUIRED");}else if self.page.empty_document{self.web_msg="HTML received - no supported visible content";serial::line("H5K: WEB_CORE_EMPTY_DOCUMENT");}if self.page.truncated{serial::line("H5K: WEB_CORE_DOCUMENT_TRUNCATED");}}
    fn follow_link(&mut self,i:usize){if i>=self.web_link_count{return;}let l=self.web_links[i];self.url=l.url;self.url_len=l.len;serial::line("H5K: BROWSER_LINK_OPENED");self.fetch_web();}
    fn remember_page(&mut self){if self.history_replay||!self.web_ok{return;}let l=crate::http::Link{url:self.source,len:self.source_len};if self.history_len>0&&self.history[self.history_pos].url[..self.history[self.history_pos].len]==l.url[..l.len]{return;}if self.history_len>0{self.history_len=self.history_pos+1;}if self.history_len==16{self.history.copy_within(1..16,0);self.history_len=15;}self.history[self.history_len]=l;self.history_pos=self.history_len;self.history_len+=1;}
    fn history_go(&mut self,forward:bool){if self.history_len==0{return;}let next=if forward{(self.history_pos+1).min(self.history_len-1)}else{self.history_pos.saturating_sub(1)};if next==self.history_pos{return;}self.history_pos=next;let l=self.history[next];self.url=l.url;self.url_len=l.len;self.history_replay=true;self.fetch_web();self.history_replay=false;serial::line("H5K: BROWSER_HISTORY_NAVIGATED");}
    fn literal_preview(&mut self){if !self.web_text{return;}let n=self.page.literal_update(&self.web[..self.web_len]);self.web_msg=if n>0{"Limited textContent update applied"}else{"No supported script update (general JS disabled)"};serial::metric("H5K: BROWSER_LITERAL_UPDATES=",n as u64);self.draw();}
    fn dynamic_preview(&mut self){
        if !self.web_text||self.page.scripts==0{return;}
        self.web_msg="Running JavaScript - Esc to stop";self.draw();self.browser_busy_begin();
        let base=&self.source[..self.source_len];let nic=&mut self.nic;
        let result=self.page.run_js(&self.web[..self.web_len],base,self.ram,|l|{
            if browser_poll(){return None;}let nic=nic.as_mut()?;let buffer=unsafe{&mut *ptr::addr_of_mut!(WEB_ASSET)};
            let n=nic.get_script(base,&l.url[..l.len],buffer).ok()?;
            if n==0||n>crate::webjs::SOURCE_CAP{return None;}
            let mut out=alloc::vec::Vec::new();out.try_reserve_exact(n).ok()?;out.extend_from_slice(&buffer[..n]);Some(out)
        });
        let cancelled=self.browser_busy_end();if cancelled{self.web_msg="JavaScript stopped - static page retained";self.draw();return;}
        match result{Ok(r)=>{
            self.web_msg=if self.page.requires_js{"JavaScript pass - website browser APIs still incomplete"}else if r.errors>0||r.skipped>0{"JavaScript partial - unsupported or over-budget scripts skipped"}else if r.executed==0{"No supported JavaScript executed"}else{"QuickJS execution complete - limited DOM bindings"};
            serial::metric("H5K: WEB_CORE_JS_EXECUTED=",r.executed as u64);serial::metric("H5K: WEB_CORE_JS_DOM_UPDATES=",r.changes as u64);serial::metric("H5K: WEB_CORE_JS_EXTERNAL=",r.external as u64);serial::metric("H5K: WEB_CORE_JS_SKIPPED=",r.skipped as u64);serial::metric("H5K: WEB_CORE_JS_ERRORS=",r.errors as u64);serial::metric("H5K: WEB_CORE_JS_MODULES=",r.modules as u64);serial::metric("H5K: WEB_CORE_JS_MODULE_ERRORS=",r.module_errors as u64);
        },Err(e)=>{self.web_msg=e;serial::line("H5K: WEB_CORE_JS_FAILED");serial::line(e);}}self.draw();
    }
    fn load_graphics(&mut self){
        if !self.web_ok||!self.web_text||self.web_status!=200{return;}self.web_msg="Loading graphics - Esc to stop";self.draw();if self.nic.is_none(){return;}self.browser_busy_begin();let nic=self.nic.as_mut().unwrap();
        let buffer=unsafe{&mut *ptr::addr_of_mut!(WEB_ASSET)};let css=unsafe{&mut *ptr::addr_of_mut!(WEB_CSS)};let mut used=0;let mut sheets=0;
        self.web_msg="Loading bounded stylesheet and image resources";
        let list=self.page.stylesheets;for l in &list[..self.page.stylesheet_count]{if browser_poll(){break;}if let Ok((n,final_url,len,status))=nic.get_follow(&l.url[..l.len],buffer){if status==200&&(!self.source[..self.source_len].starts_with(b"https://")||final_url[..len].starts_with(b"https://"))&&n<css.len().saturating_sub(used+1)&&core::str::from_utf8(&buffer[..n]).is_ok(){css[used..used+n].copy_from_slice(&buffer[..n]);used+=n;css[used]=b'\n';used+=1;sheets+=1;}}}
        self.page.parse_at_width(&self.web[..self.web_len],&self.source[..self.source_len],&css[..used],self.win.w.saturating_sub(57));let list=self.page.image_urls;for (i,l) in list[..self.page.image_count].iter().enumerate(){if browser_poll(){break;}if let Ok((n,final_url,len,status))=nic.get_follow(&l.url[..l.len],buffer){if status==200&&(!self.source[..self.source_len].starts_with(b"https://")||final_url[..len].starts_with(b"https://")){self.page.load_image(i,&buffer[..n]);}}}
        let cancelled=self.browser_busy_end();self.web_msg=if cancelled{"Graphics stopped - partial page retained"}else{"Graphics pass complete - limited CSS and images"};if !cancelled{self.web_document_diagnostics();}self.web_scroll=0;serial::metric("H5K: BROWSER_STYLESHEETS_LOADED=",sheets as u64);serial::metric("H5K: BROWSER_STYLE_BLOCKS=",self.page.styled as u64);serial::metric("H5K: BROWSER_IMAGES_LOADED=",self.page.loaded_images as u64);self.draw();
    }
    fn tab_save(&mut self){self.tabs[self.tab_index]=crate::http::Link{url:self.url,len:self.url_len};unsafe{(*ptr::addr_of_mut!(TAB_HISTORIES))[self.tab_index]=BrowserHistory{links:self.history,len:self.history_len,pos:self.history_pos};}}
    fn tab_cache_save(&mut self){let i=self.tab_index;unsafe{(*ptr::addr_of_mut!(WEB_CACHE))[i]=EMPTY_WEB;}if !self.web_ok||!self.web_text||self.web_len>CACHE_CAP||self.source_len==0{return;}self.page.cache_save(i);unsafe{ptr::copy_nonoverlapping(self.web.as_ptr(),ptr::addr_of_mut!(WEB_CACHE_BYTES).cast::<[u8;CACHE_CAP]>().add(i).cast::<u8>(),self.web_len);(*ptr::addr_of_mut!(WEB_CACHE))[i]=CachedWeb{valid:true,len:self.web_len,source:crate::http::Link{url:self.source,len:self.source_len},status:self.web_status,links:self.web_links,link_count:self.web_link_count,scroll:self.web_scroll,msg:self.web_msg};}}
    fn tab_cache_load(&mut self)->bool{let i=self.tab_index;let c=unsafe{(*ptr::addr_of!(WEB_CACHE))[i]};let t=self.tabs[i];if !c.valid||t.url[..t.len]!=c.source.url[..c.source.len]{return false;}self.page.cache_load(i);self.page.form_focus=false;unsafe{ptr::copy_nonoverlapping(ptr::addr_of!(WEB_CACHE_BYTES).cast::<[u8;CACHE_CAP]>().add(i).cast::<u8>(),self.web.as_mut_ptr(),c.len);}self.web_len=c.len;self.web_ok=true;self.web_text=true;self.source=c.source.url;self.source_len=c.source.len;self.url=c.source.url;self.url_len=c.source.len;self.web_status=c.status;self.web_links=c.links;self.web_link_count=c.link_count;self.web_scroll=c.scroll;self.web_msg=c.msg;serial::line("H5K: BROWSER_TAB_CACHE_RESTORED");self.draw();true}
    fn tab_switch(&mut self,i:usize){if i>=self.tab_count||i==self.tab_index{return;}if self.web_ok{self.url=self.source;self.url_len=self.source_len;}self.tab_save();self.tab_cache_save();self.tab_index=i;self.tab_open();}
    fn tab_width(&self)->usize{(self.win.w.saturating_sub(96)/self.tab_count.max(1)).clamp(65,180)}
    fn browser_settings_toggle(&mut self){self.browser_settings=!self.browser_settings;self.page.form_focus=false;serial::line(if self.browser_settings{"H5K: BROWSER_SETTINGS_OPEN"}else{"H5K: BROWSER_SETTINGS_CLOSED"});self.draw();}
    fn browser_home(&mut self){let u=b"https://www.google.com/";self.url[..u.len()].copy_from_slice(u);self.url_len=u.len();self.browser_settings=false;self.fetch_web();}
    fn persist_browser_settings(&self){let flags=self.auto_graphics as u8|((self.browser_diagnostics as u8)<<1);if self.store.save_browser_settings(flags){serial::line("H5K: BROWSER_SETTINGS_SAVED");}}
    fn browser_toggle(&mut self,i:usize){match i{0=>{self.auto_graphics=!self.auto_graphics;serial::metric("H5K: BROWSER_AUTO_GRAPHICS=",self.auto_graphics as u64);self.persist_browser_settings();},1=>{self.auto_script=!self.auto_script;serial::metric("H5K: BROWSER_AUTO_SCRIPT=",self.auto_script as u64);},2=>{self.browser_diagnostics=!self.browser_diagnostics;serial::metric("H5K: BROWSER_DIAGNOSTICS=",self.browser_diagnostics as u64);self.persist_browser_settings();},_=>{}}self.draw();}
    fn tab_open(&mut self){self.browser_settings=false;let h=unsafe{(*ptr::addr_of!(TAB_HISTORIES))[self.tab_index]};self.history=h.links;self.history_len=h.len;self.history_pos=h.pos;self.page.form_focus=false;if self.tab_cache_load(){}else if self.tabs[self.tab_index].len>0{let t=self.tabs[self.tab_index];self.url=t.url;self.url_len=t.len;self.history_replay=true;self.fetch_web();self.history_replay=false;}else{self.url_len=0;self.web_ok=false;self.web_len=0;self.web_status=0;self.web_text=false;self.web_link_count=0;self.page.clear();self.web_msg="New tab - enter address";self.draw();}serial::metric("H5K: BROWSER_TAB_COUNT=",self.tab_count as u64);serial::metric("H5K: BROWSER_TAB_ACTIVE=",self.tab_index as u64);}
    fn tab_new(&mut self){if self.tab_count==4{return;}if self.web_ok{self.url=self.source;self.url_len=self.source_len;}self.tab_save();self.tab_cache_save();self.tab_index=self.tab_count;self.tab_count+=1;self.tabs[self.tab_index]=crate::http::EMPTY_LINK;unsafe{(*ptr::addr_of_mut!(TAB_HISTORIES))[self.tab_index]=EMPTY_HISTORY;(*ptr::addr_of_mut!(WEB_CACHE))[self.tab_index]=EMPTY_WEB;}self.tab_open();}
    fn tab_next(&mut self){if self.tab_count<2{return;}self.tab_switch((self.tab_index+1)%self.tab_count);}
    fn tab_close(&mut self){self.tab_close_at(self.tab_index);}
    fn tab_close_at(&mut self,index:usize){if self.tab_count<=1||index>=self.tab_count{return;}self.tab_save();self.tab_cache_save();let active=self.tab_index;let histories=unsafe{&mut *ptr::addr_of_mut!(TAB_HISTORIES)};for i in index..self.tab_count-1{self.tabs[i]=self.tabs[i+1];histories[i]=histories[i+1];crate::webpage::Page::cache_move(i+1,i);unsafe{(*ptr::addr_of_mut!(WEB_CACHE))[i]=(*ptr::addr_of!(WEB_CACHE))[i+1];ptr::copy_nonoverlapping(ptr::addr_of!(WEB_CACHE_BYTES).cast::<[u8;CACHE_CAP]>().add(i+1),ptr::addr_of_mut!(WEB_CACHE_BYTES).cast::<[u8;CACHE_CAP]>().add(i),1);}}self.tab_count-=1;self.tabs[self.tab_count]=crate::http::EMPTY_LINK;histories[self.tab_count]=EMPTY_HISTORY;unsafe{(*ptr::addr_of_mut!(WEB_CACHE))[self.tab_count]=EMPTY_WEB;}self.tab_index=if index<active{active-1}else{active.min(self.tab_count-1)};if index==active{self.tab_open();}else{serial::metric("H5K: BROWSER_TAB_COUNT=",self.tab_count as u64);self.draw();}}
    fn submit_form(&mut self){let Some(l)=self.page.form.target()else{self.web_msg="Search URL too long or unsupported form";self.draw();return;};self.page.form_focus=false;self.url=l.url;self.url_len=l.len;serial::line("H5K: WEB_CORE_FORM_SUBMITTED");self.fetch_web();}
    fn fetch_web(&mut self){
        self.browser_settings=false;unsafe{(*ptr::addr_of_mut!(WEB_CACHE))[self.tab_index]=EMPTY_WEB;}self.web_ok=false;self.web_len=0;self.web_status=0;self.web_text=false;self.web_link_count=0;self.web_msg="Connecting - Esc to stop";self.draw();
        if self.url_len==0{self.web_msg="Enter a website or search term";self.draw();return;}
        let raw=&self.url[..self.url_len];if !raw.starts_with(b"http://")&&!raw.starts_with(b"https://")&&(raw.contains(&b' ')||(!raw.contains(&b'.')&&!raw.contains(&b'/')&&!raw.contains(&b':'))){
            let mut form=crate::hi5webcore::Form::new();let prefix=b"https://www.google.com/search";form.action.url[..prefix.len()].copy_from_slice(prefix);form.action.len=prefix.len();form.name[0]=b'q';form.name_len=1;form.ready=true;if raw.len()>form.value.len(){self.web_msg="Search text too long";self.draw();return;}for &v in raw{form.input(v);}let Some(l)=form.target()else{self.web_msg="Encoded search URL too long";self.draw();return;};self.url=l.url;self.url_len=l.len;
        }
        if !self.url[..self.url_len].starts_with(b"http://")&&!self.url[..self.url_len].starts_with(b"https://")&&self.url_len+8<=self.url.len(){self.url.copy_within(..self.url_len,8);self.url[..8].copy_from_slice(b"https://");self.url_len+=8;}
        self.browser_busy_begin();if self.nic.is_none(){self.nic=Nic::open();}
        if let Some(nic)=&mut self.nic{match nic.get_follow(&self.url[..self.url_len],self.web){Ok((n,url,len,status))=>{self.web_len=n;self.web_ok=true;self.web_status=status;self.source=url;self.source_len=len;self.url=url;self.url_len=len;self.web_msg=if status!=200{"Server returned an HTTP error page"}else if url[..len].starts_with(b"https://"){"HTTPS - certificate validated"}else{"HTTP response complete - not authenticated"};},Err(e)=>{self.web_msg=e;if !browser_poll(){serial::line("H5K: HTTP_DOWNLOAD_FAILED");serial::line(e);}}}}else{self.web_msg="No supported E1000 network adapter";}
        self.classify_web();let cancelled=self.browser_busy_end();self.remember_page();self.tab_save();if cancelled{self.web_msg="Navigation stopped";}self.draw();if cancelled{return;}if self.auto_graphics&&self.web_text&&(self.page.stylesheet_count>0||self.page.image_count>0){self.load_graphics();if self.web_msg=="Graphics stopped - partial page retained"{return;}}if self.auto_script&&self.web_text{self.dynamic_preview();}
    }
    fn previous_download(&mut self){if let Some(d)=self.store.load_previous_download(self.web){self.web_len=d.len;self.web_ok=true;self.web_status=200;self.source=d.url;self.source_len=d.url_len;self.url=d.url;self.url_len=d.url_len;self.web_msg="Previous saved download - not authenticated";self.classify_web();serial::line("H5K: BROWSER_PREVIOUS_DOWNLOAD");}else{self.web_msg="No previous valid download";}self.draw();}
    fn save_web(&mut self){if self.web_ok&&self.web_status==200{if self.web_len>crate::disk::DL_CAP{self.web_msg="Save limit 1 MiB - preview only";self.draw();return;}self.web_msg=if self.store.save_download(&self.web[..self.web_len],&self.source[..self.source_len]){"Saved binary download to data disk"}else{"Save failed - data disk required"};self.draw();}}
    fn browser_key(&mut self,key:u8){
        if self.ctrl&&key==0x1f{self.browser_settings_toggle();return;}if self.browser_settings{if key==0x01{self.browser_settings_toggle();}else if (0x02..=0x04).contains(&key){self.browser_toggle((key-2)as usize);}return;}
        if key==0x1c{if self.page.form_focus{self.submit_form();}else{self.fetch_web();}return;}if key==0x43{self.save_web();return;}if key==0x44{self.reopen_web();return;}
        if self.ctrl{if key==0x26{self.page.form_focus=false;self.url_len=0;self.draw();}else if key==0x14{self.tab_new();}else if key==0x0f{self.tab_next();}else if key==0x11{self.tab_close();}else if key==0x22&&self.page.form.ready{self.page.form_focus=true;self.draw();}else if key==0x17&&self.term.shift{self.auto_graphics=!self.auto_graphics;self.persist_browser_settings();self.web_msg=if self.auto_graphics{"Automatic graphics enabled"}else{"Automatic graphics disabled"};self.draw();}else if key==0x24{if self.term.shift{self.literal_preview();}else{self.dynamic_preview();}}else if key==0x17{self.load_graphics();}else if key==0x30{self.history_go(false);}else if key==0x21{self.history_go(true);}else if key==0x13{self.fetch_web();}else if key==0x20{self.previous_download();}else if (0x02..=0x09).contains(&key){self.follow_link((key-2)as usize);}return;}
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
        if self.ctrl&&self.term.shift&&key==0x26{if self.auth.account.is_some(){self.lock_account();}else if self.app==Some(App::System)&&self.store.attached(){self.auth.stage=1;self.auth.dark=true;self.auth.msg="Choose regional preferences";self.draw();}return;}
        if self.app==Some(App::System)&&self.ctrl&&(0x02..=0x07).contains(&key){if self.term.shift{if key<=6{self.apply_scale([100,125,150,175,200][(key-2)as usize]);}}else{self.apply_display((key-2)as usize);}return;}
        if self.app==Some(App::WinLive){if key==1&&self.ctrl{self.close();return;}let ch=ascii(key,self.term.shift);let vk=match key{0x01=>27,0x0e=>8,0x0f=>9,0x1c=>13,0x3b..=0x44=>112+(key-0x3b)as u32,_=>ch.map_or(0,|v|v.to_ascii_uppercase()as u32)};if vk!=0{crate::trial::live_key(vk,ch);if !crate::trial::live_active(){self.close();}else{self.draw();}}return;}
        if key==0x01{if self.app==Some(App::Browser)&&self.browser_settings{self.browser_settings_toggle();}else if self.menu{self.menu=false;self.draw();}else{self.close();}return;}
        if key==0x41&&(self.app==Some(App::Files)||self.app==Some(App::Notepad)){self.switch_file((self.file+1)%FILE_COUNT);return;}
        // Preserve F6 as the existing Notepad save shortcut. GUI fixture is Ctrl+F6.
        if key==0x40&&self.ctrl{self.select(App::WinDemo);return;}
        if key==0x58{self.select(App::Store);return;}
        if self.app==Some(App::Store)&&!self.menu&&!self.minimized{match key{0x17=>self.package_action(true),0x16=>self.package_action(false),0x22=>self.launch_registered(),0x1c=>self.select(App::Terminal),_=>{}}return;}
        if self.ctrl&&self.term.shift&&key==0x11{self.select(App::Word);return;}
        if self.app==Some(App::Word)&&!self.menu&&!self.minimized{self.word_key(key);return;}
        if key==0x57{self.select(App::Sheet);return;}
        if key==0x42{self.select(App::Browser);return;}
        if key>=0x3b&&key<=0x3f{self.select(APPS[(key-0x3b)as usize]);return;}
        if self.app==Some(App::Sheet)&&!self.menu&&!self.minimized{self.sheet_key(key);return;}
        if self.app==Some(App::Browser)&&!self.menu&&!self.minimized{self.browser_key(key);return;}
        if self.app==Some(App::Notepad)&&!self.menu&&!self.minimized{self.notepad_key(key);return;}
        if self.app==Some(App::Terminal)&&!self.menu&&!self.minimized{self.terminal_key(key);return;}
        match key{
            0x02..=0x08=>self.select(APPS[(key-2)as usize]),0x32=>{self.menu=!self.menu;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();},
            0x2e if self.app==Some(App::Network)=>self.connect_net(),0x14=>self.theme(),0x2d if self.app.is_some()=>self.maximize(),
            0x12 if self.app==Some(App::Files)=>self.select(App::Notepad),0x31 if self.app==Some(App::Files)=>self.new_file(),0x20 if self.app==Some(App::Files)=>self.delete_file(),_=>{}
        }
    }
    fn launch_registered(&mut self){let buf=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};if self.store.load_7zip(buf).is_none(){self.store_msg="Install the verified package first";self.draw();return;}if crate::trial::original_gui_live(self.c.fb.width.saturating_sub(48),self.bar_y().saturating_sub(78)){self.select(App::WinLive);serial::line("H5K: STORE_GUI_LAUNCHED");}else{self.store_msg="Original GUI startup blocked";self.draw();}}
    fn package_action(&mut self,install:bool){let ok=if install{self.store.install_7zip(crate::trial::BUNDLED)}else{self.store.uninstall_7zip()};if ok{self.package_installed=install;}self.store_msg=if ok{if install{"Portable package installed"}else{"Package disabled; user files retained"}}else{"Package transaction failed"};serial::line(if ok{if install{"H5K: STORE_INSTALL_OK"}else{"H5K: STORE_UNINSTALL_OK"}}else{"H5K: STORE_ACTION_FAILED"});self.draw();}
    fn click(&mut self){
        let x=self.mx;let y=self.my;if self.auth.stage!=0{let ox=(self.c.fb.width-520)/2;let oy=(self.c.fb.height-400)/2;if x>=ox+24&&x<ox+496&&y>=oy+308&&y<oy+356{self.auth_enter();}return;}let by=self.bar_y();let dock=self.dock_x();
        if y>=by{if x<92{self.menu=!self.menu;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();return;}if x>=dock&&x<dock+350{let a=APPS[(x-dock)/50];if self.app==Some(a)&&!self.minimized{self.minimized=true;self.draw();}else{self.select(a);}return;}}
        if self.menu{let m=self.menu_rect();if m.hit(x,y){if y>=m.y+49&&y<m.y+329{self.select(APPS[(y-m.y-49)/40]);}else if y>=m.y+338{self.menu=false;self.theme();}return;}self.menu=false;self.draw();}
        if let Some(a)=self.app{let r=self.win;if !self.minimized&&r.hit(x,y){
            if y<r.y+42{if x>=r.x+r.w-38{self.close();}else if x>=r.x+r.w-73{self.maximize();}else if x>=r.x+r.w-108{self.minimized=true;self.draw();}else if !self.maximized{self.drag=Some((x-r.x,y-r.y));}return;}
            let bx=r.x+22;let yy=r.y+58;
            if a==App::WinLive{if x>=bx&&y>=yy{crate::trial::live_click(x-bx,y-yy);if !crate::trial::live_active(){self.close();}else{self.draw();}}return;}
            if a==App::Store&&x>=bx+240&&x<bx+455&&y>=yy+144&&y<yy+178{self.launch_registered();return;}
            if a==App::Store&&x>=bx&&x<bx+230{if y>=yy+98&&y<yy+132{self.package_action(x<bx+120);return;}if y>=yy+144&&y<yy+178{self.select(App::Terminal);return;}}
            if a==App::Sheet&&x>=bx+30&&y>=yy+87&&y<yy+207{let cw=(r.w-76)/4;let col=(x-bx-30)/cw;let row=(y-yy-87)/30;if col<4{self.sheet_cell=row*4+col;self.sheet_len=0;self.draw();}return;}
            if a==App::Network&&y>=yy+229&&y<yy+263&&x>=bx&&x<bx+190{self.connect_net();return;}
            if a==App::Browser{
                let right=r.x+r.w-22;let floor=r.y+r.h-25;let top=yy+130;
                if y>=yy&&y<yy+28&&x>=bx{let tw=self.tab_width();let offset=x-bx;let i=offset/tw;if i<self.tab_count{if offset%tw>=tw-24{self.tab_close_at(i);}else{self.tab_switch(i);}}else if offset<self.tab_count*tw+35{self.tab_new();}return;}
                if y>=yy+33&&y<yy+63&&x>=bx{let ox=x-bx;match ox{0..=34=>self.history_go(false),39..=73=>self.history_go(true),78..=142=>self.fetch_web(),147..=205=>self.browser_home(),210..=290=>self.load_graphics(),295..=374=>self.browser_settings_toggle(),379..=468 if r.w>=540=>self.previous_download(),_=>{}}return;}
                if y>=yy+69&&y<yy+101&&x>=bx{if x>=r.x+r.w-95{self.fetch_web();}else{self.browser_settings=false;self.page.form_focus=false;self.url_len=0;self.draw();}return;}
                if self.browser_settings{if x>=bx&&x<right&&y>=top+27&&y<top+123{self.browser_toggle((y-top-27)/32);}return;}
                let top=top+if self.web_ok&&self.browser_diagnostics{24}else{0};
                if self.web_text&&y>=top&&y<floor&&x>=bx&&x<right{let w=right.saturating_sub(bx+13);if x>=right-10{let rows=self.page.rows(w).max(1);self.web_scroll=((y-top)*rows/floor.saturating_sub(top).max(1)).min(rows-1);self.draw();return;}let control=self.page.control_hit_xy(w,top+3,floor,self.web_scroll,x-bx,y);if control==3{self.page.form_focus=true;self.draw();return;}if control==4{self.submit_form();return;}if let Some(l)=self.page.hit_xy(w,top+3,floor,self.web_scroll,x-bx,y){self.url=l.url;self.url_len=l.len;serial::line("H5K: BROWSER_INLINE_LINK");self.fetch_web();return;}}
                if self.web_ok&&!self.web_text&&self.web_status==200&&crate::webpage::Bitmap::parse(&self.web[..self.web_len]).is_none()&&y>=top+74&&y<top+104&&x>=bx&&x<bx+180{self.save_web();return;}
                if !self.web_ok&&y>=top+40&&y<top+70&&x>=bx+12&&x<bx+122{self.browser_home();}
                return;
            }
            if a==App::Word{self.word_click(x,y);return;}
            if a==App::Notepad&&y>=yy+1&&y<yy+34{if x>=bx&&x<bx+82{self.save_note();}else if x>=bx+92&&x<bx+174{self.clear_note();}return;}
            if a==App::Files&&y>=yy+88&&y<yy+88+FILE_COUNT*31{let i=(y-yy-88)/31;if i==self.file{self.select(App::Notepad);}else{self.switch_file(i);}return;}
            if a==App::Files&&y>=yy+43&&y<yy+77{if x>=bx&&x<bx+106{self.new_file();}else if x>=bx+116&&x<bx+206{self.delete_file();}}
            if a==App::System{
                if x>=bx&&x<bx+426&&y>=yy+96&&y<yy+168{let row=(y-yy-96)/38;let col=(x-bx)/142;if (x-bx)%142<134&&(y-yy-96)%38<34{self.apply_display(row*3+col);return;}}
                if x>=bx&&x<bx+415&&y>=yy+228&&y<yy+260{self.apply_scale([100,125,150,175,200][(x-bx)/83]);return;}if x>=bx&&x<bx+180&&y>=yy+266&&y<yy+296{self.theme();}
            }return;
        }}
        if let Some(s)=self.background.iter().rev().flatten().find(|s|!s.minimized&&s.win.hit(x,y)).copied(){self.select(s.app);return;}
        if x>=100&&x<180&&y>=374&&y<435&&by>438{self.select(App::Sheet);return;}
        if x<100&&y>=8&&y<435{let i=(y-8)/61;if i<7{self.select(APPS[i]);}}
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
    let auth=Auth::new(&store);let prefs=store.browser_settings();let mut ui=Ui{word_clip:alloc::vec::Vec::new(),word:crate::office::Word::new(),word_slot:0,word_loaded:false,word_msg:"New document",word_scroll:0,auth,auto_graphics:prefs&1!=0,auto_script:false,browser_settings:false,browser_diagnostics:prefs&2!=0,tabs:[crate::http::EMPTY_LINK;4],tab_count:1,tab_index:0,page:unsafe{&mut *ptr::addr_of_mut!(crate::webpage::PAGE)},history:[crate::http::EMPTY_LINK;16],history_len:0,history_pos:0,history_replay:false,web_status:0,web_links:[crate::http::EMPTY_LINK;8],web_link_count:0,web_scroll:0,web_text:false,background:[None;10],display_msg:"Select a supported display mode",package_installed:false,store_msg:"F12 opens Hi5Store",c:canvas,app:None,minimized:false,menu:false,dark:false,ram,nics,files,tasks,mx:fb.width/2,my:fb.height/2,mouse:false,saved:[0;256],saved_valid:false,win,restore:win,maximized:false,drag:None,term:Terminal::new(),sheet:crate::sheet::Sheet::new(),sheet_result:0,sheet_cell:0,sheet_edit:[0;48],sheet_len:0,sheet_msg:"Select cell and type value or formula",pe_report:crate::imports::Report::default(),rebase:crate::relocate::Report::default(),store,note:[0;CAP],note_len:0,dirty:false,save_status:0,ctrl:false,file:0,cache,nic:None,url,url_len:default.len(),web:unsafe{&mut *ptr::addr_of_mut!(WEB_BUFFER)},source:[0;128],source_len:0,web_len:0,web_ok:false,web_msg:"Offline until Go is pressed"};
    ui.mouse=init_input();serial::line(if ui.mouse{"H5K: PS2_MOUSE_ACK"}else{"H5K: PS2_MOUSE_UNAVAILABLE"});
    serial::line(if ui.c.buffered{"H5K: DOUBLE_BUFFER_OK"}else{"H5K: DIRECT_RENDER_FALLBACK"});
    ui.draw();if ui.auth.stage==1{serial::line("H5K: ACCOUNT_SETUP_READY");}else if ui.auth.stage==7{serial::line("H5K: ACCOUNT_LOGIN_READY");}serial::line("H5K: FRAMEBUFFER_OK");serial::line("H5K: DESKTOP_READY");
    p::timer_init();let start=p::ticks();while p::ticks().wrapping_sub(start)<3{p::wait_tick();}
    serial::line("H5K: TIMER_IRQ_OK");if !crate::net::self_test()||!crate::http::navigation_test()||!crate::account::self_test()||!crate::webpage::self_test(){serial::line("H5K: FAIL NETWORK_FORMAT_TEST");crate::kernel::halt();}serial::line("H5K: NET_FORMAT_TEST_OK");serial::line("H5K: BROWSER_LAYOUT_SELFTEST_OK");serial::line("H5K: KERNEL_READY");
    let mut ext=false;
    loop{match ui.tasks.dispatch(){
        0=>{for _ in 0..32{let st=p::input(0x64);if st&1==0{break;}let b=p::input(0x60);if st&0x20!=0{
            if let Some((dx,dy,pressed,was))=mouse_byte(b){ui.move_pointer(dx,dy,pressed,was);}
        }else if b==0xe0{ext=true;}else{if !ext{ui.key(b);}else if ui.auth.stage!=0{ }else if ui.app==Some(App::WinLive)&&b&0x80==0{let vk=match b{0x48=>38,0x50=>40,0x4b=>37,0x4d=>39,0x47=>36,0x4f=>35,0x49=>33,0x51=>34,0x53=>46,_=>0};if vk!=0{crate::trial::live_key(vk,None);if !crate::trial::live_active(){ui.close();}else{ui.draw();}}}else if ui.app==Some(App::Word)&&b&0x80==0{ui.word_extended(b);}else if ui.app==Some(App::Browser)&&ui.web_text&&b&0x80==0{if b==0x49||b==0x48{ui.web_scroll=ui.web_scroll.saturating_sub(if b==0x49{5}else{1});ui.draw();}else if b==0x51||b==0x50{ui.web_scroll=(ui.web_scroll+if b==0x51{5}else{1}).min(ui.page.rows(ui.win.w.saturating_sub(57)).saturating_sub(1));ui.draw();}}else if ui.app==Some(App::Terminal)&&ui.term.result==24{if b==0x49{ui.term.scroll=ui.term.scroll.saturating_sub(5);ui.draw();}else if b==0x51{let n=crate::trial::output_bytes().iter().filter(|c|**c==10).count();ui.term.scroll=(ui.term.scroll+5).min(n.saturating_sub(1));ui.draw();}}ext=false;}}}
        1=>{if ui.auth.stage==0&&ui.app==Some(App::WinLive){crate::trial::live_tick();if !crate::trial::live_active(){ui.close();}}else{core::hint::spin_loop();}},_=>p::wait_tick(),
    }}
}
