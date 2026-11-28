//! Hi5Desktop v0.3.4 — native software GUI; single foreground window.
use crate::{boot::Framebuffer,ui::{Canvas,Color},platform as p,services::{Files,Tasks},serial,disk::{Store,CAP,FILE_COUNT,NAMES}};
use core::ptr;
use crate::net::{Nic,MAX_RESPONSE};
use crate::ui::{Palette,palette,ACCENT as BLUE,TEAL};
const WHITE:Color=[255,255,255];const GOLD:Color=[234,178,66];
#[derive(Clone,Copy,PartialEq)]enum App{System,Files,Network,Terminal,Notepad,Browser,Sheet,Store,WinDemo,WinLive,Word}
impl App{fn title(self)->&'static str{match self{Self::System=>"Settings",Self::Files=>"Hi5Explorer",Self::Network=>"Network",Self::Terminal=>"Terminal",Self::Notepad=>"Notepad",Self::Browser=>"Hi5Browser",Self::Sheet=>"Hi5Sheet",Self::Store=>"Hi5Store",Self::WinDemo=>"Windows GUI test",Self::WinLive=>"7-Zip File Manager",Self::Word=>"Hi5Word - Hi5Office"}}}
const APPS:[App;7]=[App::System,App::Files,App::Network,App::Terminal,App::Notepad,App::Browser,App::Word];
const LAUNCH:[App;9]=[App::Word,App::Sheet,App::Browser,App::Files,App::Notepad,App::Store,App::System,App::Network,App::Terminal];
const LABELS:[&str;9]=["Hi5Word","Hi5Sheet","Browser","Files","Notepad","Hi5Store","Settings","Network","Terminal"];
const PINNED:[App;6]=[App::Files,App::Browser,App::Word,App::Sheet,App::Store,App::System];
const RAIL:[&str;7]=["Home","Hi5Central","Hi5Browser","Hi5Word","Hi5Sheet","Hi5Slides","More Apps"];
const GLASS:Color=[7,31,51];const RIM:Color=[37,120,153];const SOFT:Color=[179,216,234];
fn footer_height(h:usize)->usize{if h>=700{54}else{0}}
fn dock_top(h:usize)->usize{h.saturating_sub(footer_height(h)+60)}
fn date_text(t:u64)->[u8;10]{let mut days=t/86400;let mut year=1970u64;let leap=|y:u64|y%4==0&&(y%100!=0||y%400==0);while year<9999{let n=365+leap(year)as u64;if days<n{break;}days-=n;year+=1;}let mut month=1u64;for n in [31,28+leap(year)as u64,31,30,31,30,31,31,30,31,30,31]{if days<n{break;}days-=n;month+=1;}let day=(days+1).min(31);let dig=|v:u64|b'0'+(v%10)as u8;[dig(year/1000),dig(year/100),dig(year/10),dig(year),b'-',dig(month/10),dig(month),b'-',dig(day/10),dig(day)]}
fn app_label_matches(label:&str,q:&[u8])->bool{q.is_empty()||label.as_bytes().windows(q.len()).any(|w|w.eq_ignore_ascii_case(q))}
fn card_rect(w:usize,by:usize,i:usize)->Rect{let top=56;let avail=by.saturating_sub(top+8);let wide=w>=900&&by>=540;let width=if wide{310}else{(w/3).clamp(206,282)};let (start,end)=if wide{let boundaries=[0,3,7,10,12];(avail*boundaries[i]/12,avail*boundaries[i+1]/12)}else{(avail*i/4,avail*(i+1)/4)};Rect{x:w-width-12,y:top+start,w:width,h:end.saturating_sub(start+7)}}
fn cards_fit(w:usize,by:usize)->bool{w>=640&&by>=360}
pub fn shell_test()->bool{[640usize,800,1024,1280,1920].iter().all(|&w|[420usize,480,606,660,966].iter().all(|&by|{let a=card_rect(w,by,0);let b=card_rect(w,by,3);a.x>=120&&a.x+a.w<=w&&b.y+b.h<=by&&(0..3).all(|i|{let a=card_rect(w,by,i);let b=card_rect(w,by,i+1);a.y+a.h<=b.y})}))&&[480usize,600,720,768,1080].iter().all(|&h|dock_top(h)+60+footer_height(h)==h)&&date_text(0)==*b"1970-01-01"&&date_text(951782400)==*b"2000-02-29"&&app_label_matches("Hi5Word",b"word")&&!app_label_matches("Hi5Word",b"slides")}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]struct Rect{x:usize,y:usize,w:usize,h:usize}
#[derive(Clone,Copy)]struct Resize{old:Rect,x:usize,y:usize,edges:u8}
#[derive(Clone,Copy)]struct Motion{from:Rect,to:Rect,start:u64,last:Option<Rect>}
fn interpolate_rect(a:Rect,b:Rect,n:usize)->Rect{
    let f=|x:usize,y:usize|((x as i64+(y as i64-x as i64)*n.min(12)as i64/12).max(0))as usize;
    Rect{x:f(a.x,b.x),y:f(a.y,b.y),w:f(a.w,b.w),h:f(a.h,b.h)}
}
const LEFT:u8=1;const RIGHT:u8=2;const TOP:u8=4;const BOTTOM:u8=8;
fn minimum(w:usize,h:usize)->(usize,usize){(560.min(w.saturating_sub(108)),365.min(h.saturating_sub(12)))}
fn resize_rect(s:Resize,x:usize,y:usize,w:usize,h:usize)->Rect{
    let (mw,mh)=minimum(w,h);let dx=x as isize-s.x as isize;let dy=y as isize-s.y as isize;
    let r=s.old;let mut l=r.x;let mut t=r.y;let mut rr=r.x+r.w;let mut bb=r.y+r.h;
    if s.edges&LEFT!=0{l=(r.x as isize+dx).clamp(0,rr.saturating_sub(mw)as isize)as usize;}
    if s.edges&RIGHT!=0{rr=(rr as isize+dx).clamp((l+mw).min(w)as isize,w as isize)as usize;}
    if s.edges&TOP!=0{t=(r.y as isize+dy).clamp(0,bb.saturating_sub(mh)as isize)as usize;}
    if s.edges&BOTTOM!=0{bb=(bb as isize+dy).clamp((t+mh).min(h)as isize,h as isize)as usize;}
    Rect{x:l,y:t,w:rr-l,h:bb-t}
}
fn snap_rect(w:usize,h:usize,side:u8)->Option<Rect>{
    let (mw,mh)=minimum(w,h);let half=(w.saturating_sub(18))/2;
    let r=match side{0=>Rect{x:6,y:6,w:w-12,h:h-12},1=>Rect{x:6,y:6,w:half,h:h-12},2=>Rect{x:w-half-6,y:6,w:half,h:h-12},_=>return None};
    (r.w>=mw&&r.h>=mh).then_some(r)
}
pub fn window_refinement_test()->bool{
    let s=Resize{old:Rect{x:100,y:90,w:560,h:365},x:659,y:454,edges:RIGHT|BOTTOM};
    resize_rect(s,800,550,1024,654)==(Rect{x:100,y:90,w:701,h:461})
    &&resize_rect(s,300,300,1024,654)==s.old
    &&interpolate_rect(s.old,Rect{x:0,y:0,w:40,h:30},12)==(Rect{x:0,y:0,w:40,h:30})&&snap_rect(1024,654,1).is_none()
    &&snap_rect(1280,606,2).is_some_and(|r|r.x+r.w==1274&&r.w>=560)
}

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
struct Ui{motion:Option<Motion>,explorer_grid:bool,file_search:[u8;32],file_search_len:usize,file_search_focus:bool,resize:Option<Resize>,outline:Option<Rect>,context:Option<(Rect,bool)>,notifications:bool,notices:[Option<&'static str>;6],task_switch:bool,alt:bool,alt_apps:[Option<App>;11],alt_index:usize,drag_layer:Option<crate::compositor::DragLayer>,drag_target:Option<(usize,usize)>,mode_preview:Option<(usize,usize,u64)>,net_status:&'static str,net_auto_done:bool,damage:Option<Rect>,alternate_wall:bool,settings_tab:usize,explorer_tab:usize,search:[u8;32],search_len:usize,search_focus:bool,shell_notice:Option<&'static str>,last_minute:u64,word_clip:alloc::vec::Vec<(u8,u8)>,word:crate::office::Word,word_slot:usize,word_loaded:bool,word_msg:&'static str,word_scroll:usize,auth:Auth,auto_graphics:bool,auto_script:bool,browser_settings:bool,browser_diagnostics:bool,tabs:[crate::http::Link;4],tab_count:usize,tab_index:usize,page:&'static mut crate::webpage::Page,history:[crate::http::Link;16],history_len:usize,history_pos:usize,history_replay:bool,web_status:u16,web_links:[crate::http::Link;8],web_link_count:usize,web_scroll:usize,web_text:bool,background:[Option<WindowSlot>;10],display_msg:&'static str,package_installed:bool,store_msg:&'static str,c:Canvas,app:Option<App>,minimized:bool,menu:bool,dark:bool,ram:u64,nics:u32,files:Files,tasks:Tasks,mx:usize,my:usize,mouse:bool,saved:[u32;256],saved_valid:bool,win:Rect,restore:Rect,maximized:bool,drag:Option<(usize,usize)>,term:Terminal,sheet:crate::sheet::Sheet,sheet_result:i64,sheet_cell:usize,sheet_edit:[u8;48],sheet_len:usize,sheet_msg:&'static str,pe_report:crate::imports::Report,rebase:crate::relocate::Report,store:Store,note:[u8;CAP],note_len:usize,dirty:bool,save_status:u8,ctrl:bool,file:usize,cache:[FileSlot;FILE_COUNT],nic:Option<Nic>,url:[u8;128],url_len:usize,web:&'static mut [u8;MAX_RESPONSE],source:[u8;128],source_len:usize,web_len:usize,web_ok:bool,web_msg:&'static str}
// Single-CPU cooperative input pump. Only called from the desktop or its
// synchronous browser work, never from interrupt context. No nested UI actions.
struct PointerPacket{bytes:[u8;3],pos:usize,down:bool,right:bool}
static mut POINTER_PACKET:PointerPacket=PointerPacket{bytes:[0;3],pos:0,down:false,right:false};
fn mouse_byte(b:u8)->Option<(isize,isize,bool,bool,bool,bool)>{
    if crate::pointer::active(){return None;}
    let p=unsafe{&mut *ptr::addr_of_mut!(POINTER_PACKET)};
    if p.pos==0&&b&8==0{return None;}p.bytes[p.pos]=b;p.pos+=1;if p.pos!=3{return None;}p.pos=0;
    let flags=p.bytes[0];if flags&0xc0!=0{return None;}
    let dx=p.bytes[1]as isize-if flags&0x10!=0{256}else{0};let dy=p.bytes[2]as isize-if flags&0x20!=0{256}else{0};
    let pressed=flags&1!=0;let was=p.down;p.down=pressed;let right=flags&2!=0;let right_was=p.right;p.right=right;Some((dx,dy,pressed,was,right,right_was))
}
fn busy_pointer(b:&mut BusyCursor,e:crate::pointer::Event){let (x,y)=e.position(b.x,b.y,b.canvas.fb.width,b.canvas.fb.height);if x==b.x&&y==b.y{return;}busy_hide(b);b.visible=true;b.x=x;b.y=y;busy_show(b);if !b.moved{b.moved=true;serial::line("H5K: BROWSER_BUSY_POINTER_MOVED");}}
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
        if st&0x20!=0{if let Some((dx,dy,_,_,_,_))=mouse_byte(v){if dx!=0||dy!=0{busy_hide(b);b.visible=true;b.x=(b.x as isize+dx).clamp(0,b.canvas.fb.width as isize-1)as usize;b.y=(b.y as isize-dy).clamp(0,b.canvas.fb.height as isize-1)as usize;busy_show(b);if !b.moved{b.moved=true;serial::line("H5K: BROWSER_BUSY_POINTER_MOVED");}}}}
        else if v==0xe0{b.ext=true;}else{if !b.ext&&v==0x01&&!b.cancelled{b.cancelled=true;serial::line("H5K: BROWSER_CANCEL_REQUESTED");}b.ext=false;}
    }
    for _ in 0..32{let Some(e)=crate::pointer::next()else{break;};busy_pointer(b,e);}
    b.cancelled
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
        if let Some(b)=state{busy_hide(&b);self.mx=b.x;self.my=b.y;self.mouse=b.visible;self.saved_valid=false;self.ctrl=false;self.alt=false;self.term.shift=false;self.cursor_show();serial::line("H5K: BROWSER_BUSY_END");b.cancelled}else{false}
    }
    fn cursor_hide(&mut self){if !self.saved_valid{return;}for y in 0..16{for x in 0..16{self.c.restore_pixel(self.mx+x,self.my+y);}}self.saved_valid=false;}
    fn cursor_show(&mut self){if !self.mouse{return;}for y in 0..14{for x in 0..=y/2{self.c.front_pixel(self.mx+x,self.my+y,if x==0||x==y/2||y==13{[22,32,43]}else{WHITE});}}self.saved_valid=true;}
    fn bar_y(&self)->usize{dock_top(self.c.fb.height)}
    fn full_window(&self)->Rect{Rect{x:6,y:6,w:self.c.fb.width.saturating_sub(12),h:self.bar_y().saturating_sub(12)}}
    fn save_display(&mut self){if let Some(i)=crate::display::MODES.iter().position(|m|m.0==self.c.physical.width&&m.1==self.c.physical.height){if self.store.save_display(i,self.c.scale){serial::line("H5K: DISPLAY_SETTINGS_SAVED");}}}
    fn reflow(&mut self){let fb=self.c.fb;self.mx=self.mx.min(fb.width-1);self.my=self.my.min(fb.height-1);self.saved_valid=false;self.drag=None;self.motion=None;self.resize=None;self.outline=None;self.context=None;self.notifications=false;self.task_switch=false;self.drag_layer=None;self.drag_target=None;let w=if crate::boot::vmware(){900}else{560}.min(fb.width.saturating_sub(108));let h=if crate::boot::vmware(){640}else{365}.min(dock_top(fb.height).saturating_sub(12));self.restore=Rect{x:(fb.width-w)/2+20,y:(dock_top(fb.height).saturating_sub(h))/2,w,h};self.win=if self.maximized{self.full_window()}else{self.restore};let full=self.full_window();let restore=self.restore;for slot in self.background.iter_mut().flatten(){slot.restore=restore;slot.win=if slot.maximized{full}else{restore};}}
    fn apply_display(&mut self,i:usize){
        if self.mode_preview.is_some(){return;}
        self.cursor_hide();let previous=self.c.physical;let scale=self.c.scale;
        let native=crate::display::native_preview(previous);
        // Rollback must have a known previous mode.
        let old=crate::display::MODES.iter().position(|m|m.0==previous.width&&m.1==previous.height);
        if native&&old.is_none(){self.display_msg="Unlisted boot mode - native changes disabled";self.draw();return;}
        if let Some(fb)=crate::display::set_mode(previous,i){
            self.c=Canvas::scaled(fb,scale);self.reflow();
            if native&&old!=Some(i){self.mode_preview=Some((old.unwrap(),scale,p::ticks().saturating_add(1500)));self.display_msg="Preview: Enter to keep, Esc to restore (15 seconds)";}
            else{if !native{self.save_display();}self.display_msg="Resolution applied";}
            self.draw();serial::metric("H5K: GRAPHICS_WIDTH=",fb.width as u64);serial::metric("H5K: GRAPHICS_HEIGHT=",fb.height as u64);serial::line("H5K: GRAPHICS_MODE_OK");
        }else{self.display_msg="Mode unavailable - previous resolution retained";self.draw();serial::line("H5K: GRAPHICS_MODE_UNAVAILABLE");}
    }
    fn finish_mode(&mut self,keep:bool){if let Some((old,scale,_))=self.mode_preview.take(){
        if keep{self.display_msg="Mode kept for this session; reboot restores firmware";serial::line("H5K: SVGA2_PREVIEW_KEPT");}
        else{self.cursor_hide();if let Some(fb)=crate::display::set_mode(self.c.physical,old){self.c=Canvas::scaled(fb,scale);self.reflow();self.display_msg="Previous resolution restored";serial::line("H5K: SVGA2_PREVIEW_REVERTED");}else{self.display_msg="Rollback failed - restart VM to restore firmware";serial::line("H5K: SVGA2_PREVIEW_REVERT_FAILED");}}
        self.draw();
    }}
    fn apply_scale(&mut self,scale:usize){if self.mode_preview.is_some(){return;}if !Canvas::supported(self.c.physical,scale){self.display_msg="Scale needs a larger resolution";self.draw();return;}self.cursor_hide();self.c=Canvas::scaled(self.c.physical,scale);self.reflow();self.save_display();self.display_msg="Interface scale applied";self.draw();serial::metric("H5K: DISPLAY_SCALE=",scale as u64);}
    fn park_window(&mut self){if let Some(a)=self.app{if a==App::WinLive{return;}let slot=WindowSlot{app:a,win:self.win,restore:self.restore,maximized:self.maximized,minimized:self.minimized};if let Some(i)=self.background.iter().position(|s|s.is_none()){self.background[i]=Some(slot);}}}
    fn take_window(&mut self,a:App)->Option<WindowSlot>{let i=self.background.iter().position(|s|s.is_some_and(|v|v.app==a))?;let out=self.background[i];for j in i..9{self.background[j]=self.background[j+1];}self.background[9]=None;out}
    fn dock_x(&self)->usize{if self.c.fb.width>=960{self.c.fb.width/2-76}else{self.c.fb.width.saturating_sub(300)/2}}
    fn search_rect(&self)->Rect{let w=self.c.fb.width;Rect{x:70,y:self.bar_y()+11,w:if w>=960{self.dock_x().saturating_sub(86)}else{80},h:38}}
    fn rail_rect(&self,i:usize)->Rect{let step=((self.bar_y().saturating_sub(70))/7).min(78);Rect{x:14,y:58+i*step,w:102,h:step}}
    fn widget_rect(&self,i:usize)->Rect{card_rect(self.c.fb.width,self.bar_y(),i)}
    fn show_notice(&mut self,t:&'static str){self.context=None;self.notifications=false;self.task_switch=false;serial::line("H5K: SHELL_NOTICE_OPEN");self.menu=false;self.search_focus=false;self.shell_notice=Some(t);self.draw();}
    fn toggle_menu(&mut self,search:bool){self.context=None;self.notifications=false;self.task_switch=false;self.shell_notice=None;self.menu=if search{true}else{!self.menu};self.search_focus=search;self.search_len=0;serial::line("H5K: GUI_MENU_TOGGLED");self.draw();}
    fn launch_rail(&mut self,i:usize){match i{0=>self.select(App::Files),1=>self.show_notice("Hi5Central is not enrolled on this device."),2=>self.select(App::Browser),3=>self.select(App::Word),4=>self.select(App::Sheet),5=>self.show_notice("Hi5Slides is not installed in this preview."),_=>self.toggle_menu(false)}}
    fn menu_rect(&self)->Rect{Rect{x:12,y:self.bar_y().saturating_sub(400),w:456,h:392}}
    fn menu_tile(r:Rect,i:usize)->Rect{Rect{x:r.x+20+(i%3)*139,y:r.y+88+(i/3)*75,w:132,h:68}}
    fn button(&self,r:Rect,t:&str,fill:Color,ink:Color){self.c.control(r.x,r.y,r.w,r.h,t,fill,ink);}
    fn icon(&self,a:App,x:usize,y:usize){let c=&self.c;
        let tile=match a{App::System=>[38,103,169],App::Files=>[176,117,34],App::Network=>[17,119,133],App::Terminal=>[19,38,56],App::Notepad=>[90,112,146],App::Browser=>[16,128,142],App::Word=>[28,88,180],App::Sheet=>[24,123,83],_=>[61,88,143]};
        c.tile_gradient(x,y,34,34,core::array::from_fn(|i|(tile[i]as u16+55).min(255)as u8),tile);c.round(x+5,y+2,24,2,1,[125,180,209]);
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
        self.outline=None;self.motion=None;self.drag_layer=None;self.drag_target=None;
        if self.auth.stage!=0{self.draw_auth();return;}
        self.cursor_hide();self.saved_valid=false;self.paint_scene(true);
        self.last_minute=crate::security::now().map_or(u64::MAX,|t|t/60);if let Some(r)=self.damage.take(){self.c.present_rect(r.x,r.y,r.w,r.h);}else{self.c.present();}self.cursor_show();
    }
    fn paint_scene(&mut self,foreground:bool){
        let c=&self.c;let w=c.fb.width;let h=c.fb.height;let q=palette(self.dark);let chrome=palette(true);let by=self.bar_y();
        c.wallpaper(h,self.alternate_wall);
        c.round(2,2,w-4,2,1,RIM);c.rect(2,4,1,h-8,RIM);c.rect(w-3,4,1,h-8,RIM);c.rect(2,h-3,w-4,1,RIM);
        c.brand(16,12,TEAL);c.title(56,12,"Hi5OS",WHITE);
        let profile=self.auth.account.as_ref().map_or("Local",|a|a.username());
        c.search_mark(w-156,18,SOFT);
        c.round(w-112,18,18,15,6,SOFT);c.round(w-110,19,14,11,5,GLASS);c.rect(w-114,32,22,2,SOFT);
        c.round(w-62,11,36,36,18,BLUE);c.text_limit(w-52,17,profile.get(..1).unwrap_or("U"),WHITE,false,w-28);
        for i in 0..7{let t=self.rail_rect(i);let ix=t.x+(t.w-34)/2;let iy=t.y+2;
            match i{0=>self.icon(App::Files,ix,iy),1=>{c.round(ix,iy,34,34,9,[17,66,94]);c.brand(ix+1,iy+1,TEAL);},2=>self.icon(App::Browser,ix,iy),3=>self.icon(App::Word,ix,iy),4=>self.icon(App::Sheet,ix,iy),5=>{c.round(ix,iy,34,34,9,[196,101,22]);c.text(ix+10,iy+5,"S",WHITE);},_=>{c.round(ix,iy,34,34,9,[22,52,74]);c.grid_mark(ix+8,iy+8,SOFT);}}
            if t.h>=54{let label=RAIL[i];let tx=t.x+t.w.saturating_sub(c.text_width(label))/2;c.text_limit(tx,iy+35,label,WHITE,false,t.x+t.w);}
        }
        let clear=(self.app.is_none()||self.minimized)&&self.background.iter().all(|s|s.is_none()||s.is_some_and(|v|v.minimized));
        if cards_fit(w,by)&&clear{
            let connected=self.nic.as_ref().is_some_and(|n|n.config().0!=[0;4]);
            for i in 0..4{let a=self.widget_rect(i);c.frost(a.x,a.y,a.w,a.h,14,GLASS,RIM);let x=a.x+12;let y=a.y+6;let right=a.x+a.w-10;let large=a.w>=300&&a.h>=94;
                match i{
                    0=>{if large{c.brand(x,y+5,TEAL);c.title(x+42,y,"Hi5Central",WHITE);c.text_limit(x+42,y+30,"IT Operations & Support",SOFT,false,right);c.text_limit(x+42,y+55,"Not enrolled",SOFT,false,right);}else{c.text_limit(x,y,"Hi5Central",WHITE,false,right);c.text_limit(x,y+24,"Not enrolled",SOFT,false,right);}},
                    1=>{c.text_limit(x,y,"Device (RMM)",WHITE,large,right);c.text_limit(x,y+27,if connected{"Network: connected"}else{"Network: offline"},if connected{chrome.green}else{SOFT},false,right);if a.h>=76{c.text_limit(x,y+49,if self.store.attached(){"Storage: attached"}else{"Storage: RAM only"},SOFT,false,right);}if a.h>=108{c.text_limit(x,y+72,"Security / patches: unverified",SOFT,false,right);}if a.h>=132{c.text_limit(x,y+96,"Remote access: not enrolled",SOFT,false,right);}},
                    2=>{c.text_limit(x,y,"Company Resources",WHITE,large,right);if a.h>=90&&large{for(j,label)in ["Support","Request","Knowledge"].iter().enumerate(){c.control(x+j*96,y+40,92,36,label,[24,58,80],SOFT);}}else{c.text_limit(x,y+26,"Support / Request / KB",SOFT,false,right);}},
                    _=>{c.text_limit(x,y,"Announcements",WHITE,false,right);c.text_limit(x,y+25,"Company feed not connected",SOFT,false,right);}
                }
            }
        }
        let saved=self.win;for i in 0..10{if let Some(s)=self.background[i]{if !s.minimized{self.win=s.win;self.draw_window(s.app,q);}}}self.win=saved;if foreground{if let Some(a)=self.app{if !self.minimized{self.draw_window(a,q);}}}
        // Always draw shell controls last, above all application surfaces.
        c.frost(8,by+3,w-16,54,16,[6,30,48],RIM);
        c.round(15,by+10,44,40,11,[16,76,103]);c.grid_mark(27,by+20,TEAL);
        let sr=self.search_rect();c.round(sr.x,sr.y,sr.w,sr.h,14,[18,47,69]);c.search_mark(sr.x+10,sr.y+9,SOFT);
        if w>=960{c.text_limit(sr.x+37,sr.y+9,if self.search_len>0{core::str::from_utf8(&self.search[..self.search_len]).unwrap_or("")}else{"Search apps..."},SOFT,false,sr.x+sr.w-8);}
        let dock=self.dock_x();for(i,a)in PINNED.iter().enumerate(){let x=dock+i*50;if self.app==Some(*a)||self.background.iter().any(|s|s.is_some_and(|v|v.app==*a)){c.round(x,by+8,46,44,9,[23,66,86]);c.round(x+14,by+49,18,3,1,TEAL);}self.icon(*a,x+6,by+11);}
        let tray=w.saturating_sub(146);let online=self.nic.as_ref().is_some_and(|n|n.config().0!=[0;4]);
        c.round(tray,by+17,8,8,4,if online{chrome.green}else{SOFT});if w<960||crate::security::now().is_none(){c.text(tray+14,by+9,if online{"Online"}else{"Offline"},SOFT);}
        if let Some(t)=crate::security::now(){let min=t/60%60;let hour=t/3600%24;let v=[b'0'+(hour/10)as u8,b'0'+(hour%10)as u8,b':',b'0'+(min/10)as u8,b'0'+(min%10)as u8];c.text(tray+14,by+29,core::str::from_utf8(&v).unwrap_or("--:--"),WHITE);if w>=960{c.text_limit(tray+62,by+29,if online{"Online"}else{"Offline"},SOFT,false,w-12);}if w>=960{let date=date_text(t);c.text_limit(tray+14,by+9,core::str::from_utf8(&date).unwrap_or(""),SOFT,false,w-12);}}else{c.text(tray+14,by+29,"--:--",WHITE);}
        if footer_height(h)>0{c.rect(3,by+60,w-6,footer_height(h)-3,[5,25,43]);let labels=["Enterprise Security","Integrated IT (Hi5Central)","High Productivity","Unified Workspace"];for(i,label)in labels.iter().enumerate(){let x=14+i*(w-28)/4;if i>0{c.rect(x-8,by+69,1,34,RIM);}c.text_limit(x,by+67,label,WHITE,false,x+(w-28)/4-8);c.text_limit(x,by+88,["Security foundations","Company enrolment planned","Native productivity apps","Local business desktop"][i],SOFT,false,x+(w-28)/4-8);}}
        if self.menu{let r=self.menu_rect();c.frost(r.x,r.y,r.w,r.h,18,GLASS,RIM);c.brand(r.x+18,r.y+14,TEAL);c.title(r.x+60,r.y+11,"Hi5OS",WHITE);
            c.round(r.x+18,r.y+48,r.w-36,34,10,[18,47,69]);c.search_mark(r.x+28,r.y+55,SOFT);c.text_limit(r.x+52,r.y+54,if self.search_len>0{core::str::from_utf8(&self.search[..self.search_len]).unwrap_or("")}else{"Search installed apps"},SOFT,false,r.x+r.w-24);
            for(i,a)in LAUNCH.iter().enumerate(){if !app_label_matches(LABELS[i],&self.search[..self.search_len]){continue;}let t=Self::menu_tile(r,i);c.round(t.x,t.y,t.w,t.h,10,[23,54,76]);self.icon(*a,t.x+(t.w-34)/2,t.y+5);let label=LABELS[i];c.text_limit(t.x+t.w.saturating_sub(c.text_width(label))/2,t.y+40,label,WHITE,false,t.x+t.w-3);}
            c.rect(r.x+18,r.y+325,r.w-36,1,RIM);
            self.button(Rect{x:r.x+20,y:r.y+338,w:208,h:38},if self.dark{"Light appearance (T)"}else{"Dark appearance (T)"},[24,58,80],WHITE);
            self.button(Rect{x:r.x+238,y:r.y+338,w:198,h:38},if self.auth.account.is_some(){"Lock workspace"}else{"Settings"},BLUE,WHITE);
        }
        if self.mode_preview.is_some(){let x=(w-456)/2;let y=(by.saturating_sub(164))/2;c.frost(x,y,456,164,18,GLASS,RIM);c.title(x+20,y+12,"Keep this display mode?",WHITE);c.text_limit(x+20,y+52,"Automatic rollback after 15 seconds.",SOFT,false,x+436);c.control(x+20,y+106,192,38,"Restore (Esc)",[24,58,80],WHITE);c.control(x+244,y+106,192,38,"Keep (Enter)",BLUE,WHITE);}
        if let Some((r,window))=self.context{c.frost(r.x,r.y,r.w,r.h,12,GLASS,RIM);let labels=if window{["Minimise","Maximise / restore","Snap left","Snap right","Close window"]}else{["Open Hi5Explorer","Display settings","Switch appearance","Change wallpaper","Running applications"]};for(i,label)in labels.iter().enumerate(){let y=r.y+7+i*30;c.round(r.x+7,y,r.w-14,28,5,[24,58,80]);c.text_limit(r.x+14,y+2,label,WHITE,false,r.x+r.w-12);}}
        if self.notifications{let r=self.notification_rect();c.frost(r.x,r.y,r.w,r.h,14,GLASS,RIM);c.title(r.x+14,r.y+8,"Notifications",WHITE);for(i,msg)in self.notices.iter().flatten().enumerate(){c.text_limit(r.x+14,r.y+44+i*29,msg,SOFT,false,r.x+r.w-10);}if self.notices[0].is_none(){c.text(r.x+14,r.y+44,"No local notifications",SOFT);}c.control(r.x+12,r.y+r.h-38,r.w-24,30,"Clear local notifications",[24,58,80],WHITE);}
        if self.task_switch{let r=self.task_rect();c.frost(r.x,r.y,r.w,r.h,14,GLASS,RIM);c.title(r.x+14,r.y+8,"Running applications",WHITE);for(i,a)in self.running_apps().iter().flatten().enumerate(){let x=r.x+6+(i%2)*(r.w/2);let y=r.y+44+(i/2)*52;let ww=r.w/2-12;c.round(x,y,ww,46,8,[24,58,80]);self.icon(*a,x+5,y+6);c.text_limit(x+43,y+12,a.title(),WHITE,false,x+ww-4);}}
        if let Some(msg)=self.shell_notice{let x=(w-456)/2;let y=(by.saturating_sub(164))/2;c.glass(x,y,456,164,18,GLASS,RIM);c.title(x+20,y+12,"Hi5OS workspace",WHITE);c.text_limit(x+20,y+58,msg,SOFT,false,x+436);c.control(x+294,y+106,142,38,"Close",BLUE,WHITE);}
    }
    fn draw_auth(&mut self){self.cursor_hide();self.damage=None;self.saved_valid=false;let c=&self.c;let q=palette(self.auth.dark);let w=c.fb.width;let h=c.fb.height;let x=(w-520)/2;let y=(h-400)/2;c.wallpaper(h,self.alternate_wall);c.frost(x,y,520,400,20,if self.auth.dark{GLASS}else{[235,244,250]},RIM);c.brand(x+24,y+16,TEAL);c.title(x+68,y+16,if self.auth.stage==7{"Welcome to Hi5OS"}else{"Set up Hi5OS"},q.ink);c.rect(x+24,y+54,472,1,q.edge);
        if h>=620{c.brand(24,22,TEAL);c.title(66,22,"Hi5OS",WHITE);c.text(24,52,"Secure local workspace",SOFT);}
        let a=&self.auth;let (heading,hint,value)=match a.stage{1=>("1 / 6  Language and region","1: English (UK)   2: English (US)",if a.region==0{"English (UK)"}else{"English (US)"}),2=>("2 / 6  Local username","Letters, digits, hyphen or underscore",core::str::from_utf8(&a.name[..a.len]).unwrap_or("")),3=>("3 / 6  Computer name","Letters, digits, hyphen or underscore",core::str::from_utf8(&a.host[..a.host_len]).unwrap_or("")),4=>("4 / 6  Create password","Use at least 12 characters (max 64)",""),5=>("5 / 6  Confirm password","Enter the same password again",""),6=>("6 / 6  Appearance","1: Light   2: Dark",if a.dark{"Dark"}else{"Light"}),7=>("Local account",a.account.as_ref().map_or("",|v|v.username()),""),_=>("Recovery required","Account data cannot be verified","")};
        c.title(x+24,y+68,heading,q.ink);c.text_limit(x+24,y+112,hint,q.muted,false,x+496);c.round(x+24,y+152,472,52,9,q.edge);c.round(x+25,y+153,470,50,8,q.sub);if a.stage==7{c.round(x+456,y+72,38,38,19,BLUE);c.round(x+469,y+79,12,12,6,WHITE);c.round(x+464,y+94,22,10,5,WHITE);}
        if matches!(a.stage,1|6){let options=if a.stage==1{["English (UK)","English (US)"]}else{["Light","Dark"]};let selected=if a.stage==1{a.region as usize}else{a.dark as usize};for(i,label)in options.iter().enumerate(){self.button(Rect{x:x+29+i*234,y:y+157,w:228,h:42},label,if i==selected{BLUE}else{q.panel},if i==selected{WHITE}else{q.ink});}}else if matches!(a.stage,4|5|7){let n=if a.stage==5{a.confirm_len}else{a.pass_len};let stars=[b'*';64];c.text_limit(x+36,y+165,core::str::from_utf8(&stars[..n]).unwrap_or(""),q.ink,false,x+488);}else{c.text_limit(x+36,y+165,value,q.ink,false,x+488);}
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
    fn lock_account(&mut self){if self.auth.account.is_none(){return;}self.context=None;self.notifications=false;self.task_switch=false;self.resize=None;self.file_search_focus=false;self.motion=None;self.alt=false;self.alt_apps=[None;11];if self.app==Some(App::WinLive){crate::trial::live_close();self.app=None;}self.menu=false;self.search_focus=false;self.search_len=0;self.shell_notice=None;self.drag=None;self.word_clip.fill((0,0));self.word_clip.clear();self.auth.clear_password();self.auth.stage=7;self.auth.msg="Enter your password";self.auth.delay=0;serial::line("H5K: ACCOUNT_LOCKED");self.draw();}
    fn settings_sidebar(&self)->bool{self.win.w>=740}
    fn content_x(&self,a:App)->usize{self.win.x+22+if a==App::System&&self.settings_sidebar(){174}else{0}}
    fn explorer_cell(&self,i:usize)->Rect{let r=self.win;let x=r.x+22;let y=r.y+146;if self.explorer_grid{let cw=(r.w-50)/2;Rect{x:x+(i%2)*cw,y:y+(i/2)*62,w:cw-5,h:56}}else{Rect{x,y:y+i*31,w:r.w-44,h:29}}}
    fn file_matches(&self,i:usize)->bool{let name=if self.explorer_tab==1{crate::office::NAMES[i]}else{NAMES[i]};app_label_matches(name,&self.file_search[..self.file_search_len])}
    fn draw_window(&self,a:App,q:Palette){
        let c=&self.c;let r=self.win;
        c.panel(r.x,r.y,r.w,r.h,14,q);
        c.round(r.x+1,r.y+1,r.w-2,42,13,q.bar);c.rect(r.x+1,r.y+35,r.w-2,8,q.bar);c.rect(r.x+1,r.y+42,r.w-2,1,q.edge);
        self.icon(a,r.x+8,r.y+4);c.text_limit(r.x+52,r.y+11,a.title(),q.ink,false,r.x+r.w-116);
        c.window_control(r.x+r.w-108,r.y+5,0,q);
        c.window_control(r.x+r.w-73,r.y+5,1,q);
        c.window_control(r.x+r.w-38,r.y+5,2,q);if !self.maximized&&a!=App::WinLive{for i in 0..3{c.rect(r.x+r.w-4-i*3,r.y+r.h-3-i*3,2,2,q.muted);}}
        let x=self.content_x(a);let y=r.y+58;let right=r.x+r.w-14;
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
                if self.settings_sidebar(){c.round(r.x+12,y,158,r.h.saturating_sub(70),10,q.sub);for(i,label)in ["Display","Personalisation","Network","Account","Security","Updates"].iter().enumerate(){self.button(Rect{x:r.x+18,y:y+8+i*37,w:146,h:32},label,if i==self.settings_tab{BLUE}else{q.sub},if i==self.settings_tab{WHITE}else{q.ink});}}
                if self.settings_tab!=0&&self.settings_sidebar(){
                    match self.settings_tab{
                    1=>{c.title(x,y,"Personalisation",q.ink);c.text(x,y+42,"Application appearance",q.muted);self.button(Rect{x,y:y+72,w:180,h:36},if self.dark{"Use light theme"}else{"Use dark theme"},BLUE,WHITE);c.text(x,y+130,"Desktop wallpaper",q.muted);self.button(Rect{x,y:y+162,w:140,h:36},"Aurora",if !self.alternate_wall{BLUE}else{q.sub},if !self.alternate_wall{WHITE}else{q.ink});self.button(Rect{x:x+150,y:y+162,w:140,h:36},"Midnight",if self.alternate_wall{BLUE}else{q.sub},if self.alternate_wall{WHITE}else{q.ink});c.text_limit(x,y+218,"Preferences saved to attached data disk.",q.muted,false,right);},
                    2=>{c.title(x,y,"Network",q.ink);c.text(x,y+44,"Open network configuration.",q.muted);self.button(Rect{x,y:y+82,w:180,h:36},"Open Network",BLUE,WHITE);},
                    3=>{c.title(x,y,"Local account",q.ink);c.text_limit(x,y+44,self.auth.account.as_ref().map_or("No account configured",|a|a.username()),q.ink,false,right);self.button(Rect{x,y:y+82,w:180,h:36},if self.auth.account.is_some(){"Lock workspace"}else{"Set up account"},BLUE,WHITE);},
                    4=>{c.title(x,y,"Security",q.ink);c.text_limit(x,y+44,"Local password verification available.",q.ink,false,right);c.text_limit(x,y+76,"Disk encryption and endpoint protection not implemented.",q.muted,false,right);},
                    _=>{c.title(x,y,"Updates",q.ink);c.text_limit(x,y+44,"Development system image upgrades via VMware.",q.ink,false,right);c.text_limit(x,y+76,"Native verified updates not available yet.",q.muted,false,right);}
                    }
                    return;
                }
                c.title(x,y,"Display & appearance",q.ink);c.text_limit(x+260,y+5,"SYSTEM / DISPLAY",q.muted,false,right);c.rect(x,y+32,right.saturating_sub(x),1,q.edge);
                c.text(x,y+38,"Current resolution:",q.muted);c.num(x+170,y+38,c.physical.width as u64,q.ink);c.text(x+224,y+38,"x",q.muted);c.num(x+242,y+38,c.physical.height as u64,q.ink);
                c.text_limit(x,y+67,crate::display::backend(c.physical),q.muted,false,right);
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
                c.title(x,y,"Hi5Explorer",q.ink);c.text_limit(x+180,y+5,if self.explorer_tab==1{"Workspace / Office"}else{"Workspace / Notes"},q.muted,false,right);
                c.rect(x,y+32,r.w-44,1,q.edge);
                if self.explorer_tab==1{self.button(Rect{x,y:y+43,w:106,h:34},"Notes",q.sub,q.ink);self.button(Rect{x:x+116,y:y+43,w:130,h:34},"Office",BLUE,WHITE);}
                else{self.button(Rect{x,y:y+43,w:106,h:34},"New note",BLUE,WHITE);self.button(Rect{x:x+116,y:y+43,w:90,h:34},"Delete",q.sub,q.ink);self.button(Rect{x:x+216,y:y+43,w:130,h:34},"Office",q.sub,q.ink);}
                self.button(Rect{x:x+356,y:y+43,w:100,h:34},if self.explorer_grid{"List view"}else{"Grid view"},q.sub,q.ink);
                for i in 0..FILE_COUNT{if !self.file_matches(i){continue;}let cell=self.explorer_cell(i);let selected=self.explorer_tab==0&&i==self.file;let name=if self.explorer_tab==1{crate::office::NAMES[i]}else{NAMES[i]};let detail=if self.explorer_tab==1{"Open in Hi5Word"}else if self.cache[i].present{"Document"}else{"Empty slot"};c.round(cell.x,cell.y,cell.w,cell.h,6,if selected{q.edge}else{q.sub});
                    if self.explorer_grid{self.icon(if self.explorer_tab==1{App::Word}else{App::Notepad},cell.x+7,cell.y+10);c.text_limit(cell.x+47,cell.y+5,name,q.ink,false,cell.x+cell.w-4);c.text_limit(cell.x+47,cell.y+29,detail,q.muted,false,cell.x+cell.w-4);}
                    else{c.text_limit(cell.x+8,cell.y+1,name,q.ink,false,cell.x+155);c.text_limit(cell.x+156,cell.y+1,detail,q.muted,false,cell.x+cell.w-6);if self.explorer_tab==0&&self.cache[i].present&&cell.w>=390{c.num(cell.x+315,cell.y+1,self.cache[i].len as u64,q.ink);c.text(cell.x+354,cell.y+1,"B",q.muted);}}
                }
                c.round(x,y+216,r.w-44,28,6,if self.file_search_focus{q.edge}else{q.sub});c.text_limit(x+8,y+218,if self.file_search_len==0{"Search slots (Ctrl+F)"}else{core::str::from_utf8(&self.file_search[..self.file_search_len]).unwrap_or("")},q.ink,false,right);
                c.text_limit(x,y+250,if self.file_search_focus{"Enter: apply   Esc: clear"}else if self.explorer_tab==1{"Native Office slots - select to open"}else if self.store.attached(){"Local data disk - four note slots"}else{"RAM workspace - not persistent"},q.muted,false,right);
            }
            App::Network=>{
                c.title(x,y,"Network status",q.ink);c.round(x,y+44,r.w-44,91,8,q.sub);c.text(x+16,y+55,crate::net::adapter_label(),q.ink);
                c.text(x+16,y+83,"Detected PCI network devices:",q.muted);c.num(x+260,y+83,self.nics as u64,q.ink);
                if let Some(n)=&self.nic{let (ip,gw,dns)=n.config();for (i,(label,value)) in [("IP address",ip),("Gateway",gw),("DNS server",dns)].iter().enumerate(){let yy=y+148+i*24;c.text(x,yy,label,q.muted);draw_ip(c,x+140,yy,*value,q.ink);}}
                self.button(Rect{x,y:y+229,w:190,h:34},"Connect / DHCP (C)",BLUE,WHITE);c.text_limit(x,y+267,self.net_status,q.muted,false,right);
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
                c.title(x,y,"Hi5Sheet - workbook",q.ink);c.text_limit(x+250,y+4,"HOME / FORMULAS",q.muted,false,right);c.round(x-5,y+28,r.w-34,39,7,q.bar);
                let name=crate::sheet::Sheet::cell_name(self.sheet_cell);let label=core::str::from_utf8(&name).unwrap_or("a1");
                c.text(x,y+36,label,q.ink);c.round(x+44,y+32,r.w-88,30,4,q.sub);
                if let Ok(t)=core::str::from_utf8(&self.sheet_edit[..self.sheet_len]){c.text_limit(x+50,y+36,t,q.ink,false,right);}
                let cw=(r.w-76)/4;
                for col in 0..4{let b=[b'A'+col as u8];c.text(x+30+col*cw+8,y+67,core::str::from_utf8(&b).unwrap_or(""),q.muted);}
                for row in 0..4{let b=[b'1'+row as u8];c.text(x,y+90+row*30,core::str::from_utf8(&b).unwrap_or(""),q.muted);
                    for col in 0..4{let i=row*4+col;let xx=x+30+col*cw;let yy=y+87+row*30;
                        c.rect(xx,yy,cw-2,28,if i==self.sheet_cell{TEAL}else{q.edge});c.rect(xx+1,yy+1,cw-4,26,q.sub);
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
    fn select(&mut self,a:App){let animate=a!=App::WinLive&&(self.app.is_none()||self.minimized||self.app!=Some(a));self.file_search_focus=false;self.context=None;self.notifications=false;self.task_switch=false;if a==App::Files&&self.app!=Some(a)&&!self.background.iter().flatten().any(|s|s.app==a){self.explorer_tab=0;self.file_search_len=0;}if a==App::System&&self.app!=Some(a)&&!self.background.iter().flatten().any(|s|s.app==a){self.settings_tab=0;}self.search_focus=false;self.search_len=0;self.shell_notice=None;if self.app!=Some(a){let previous=self.app;if previous==Some(App::WinLive){crate::trial::live_close();}else{self.park_window();}if let Some(s)=self.take_window(a){self.win=s.win;self.restore=s.restore;self.maximized=s.maximized;}else{self.win=self.restore;self.win.x=(self.win.x+24).min(self.c.fb.width.saturating_sub(self.win.w));self.win.y=(self.win.y+18).min(self.bar_y().saturating_sub(self.win.h));self.restore=self.win;self.maximized=false;}serial::line("H5K: GUI_WINDOW_FOCUSED");}if a==App::WinLive&&!self.maximized{self.restore=self.win;self.win=self.full_window();self.maximized=true;}if a==App::WinDemo{self.store_msg=if crate::trial::run_gui_fixture(){"PE/DLL and Win32 drawing fixture passed"}else{"PE GUI fixture failed"};}if a==App::Store{let b=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};self.package_installed=self.store.load_7zip(b).is_some();}if a==App::Notepad&&!self.dirty{self.load_note();}if a==App::Word&&!self.word_loaded{self.word_open(0);self.word_loaded=true;}self.app=Some(a);self.minimized=false;self.menu=false;serial::line(match a{App::System=>"H5K: GUI_SYSTEM_OPEN",App::Files=>"H5K: GUI_FILES_OPEN",App::Network=>"H5K: GUI_NETWORK_OPEN",App::Terminal=>"H5K: GUI_TERMINAL_OPEN",App::Notepad=>"H5K: GUI_NOTEPAD_OPEN",App::Browser=>"H5K: GUI_BROWSER_OPEN",App::Sheet=>"H5K: GUI_SHEET_OPEN",App::Store=>"H5K: GUI_STORE_OPEN",App::WinDemo=>"H5K: GUI_WINDEMO_OPEN",App::WinLive=>"H5K: GUI_WINLIVE_OPEN",App::Word=>"H5K: GUI_WORD_OPEN"});self.draw();if animate{self.animate_window(self.win,true);}}
    fn persist_appearance(&mut self){self.auth.dark=self.dark;if self.store.save_appearance(self.dark as u8|((self.alternate_wall as u8)<<1)){serial::line("H5K: APPEARANCE_SAVED");self.notify("Appearance preferences saved");}}
    fn theme(&mut self){self.dark=!self.dark;self.persist_appearance();serial::line(if self.dark{"H5K: GUI_THEME_DARK"}else{"H5K: GUI_THEME_LIGHT"});self.draw();}
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
    fn word_save(&mut self){let data=self.word.encode();if self.store.save_office(self.word_slot,false,&data){self.word.dirty=false;self.word_msg="Saved .h5doc";self.notify("Hi5Word document saved");serial::line("H5K: OFFICE_WORD_SAVED");serial::metric("H5K: OFFICE_WORD_CRC=",crate::office::crc(&data[..data.len()-4])as u64);}else{self.word_msg="Save failed - data disk required";}}
    fn word_export(&mut self){if let Some(data)=self.word.docx(){if self.store.save_office(self.word_slot,true,&data){self.word_msg="DOCX export saved in matching slot";self.notify("Word document exported to DOCX");serial::line("H5K: OFFICE_DOCX_SAVED");return;}}self.word_msg="DOCX export failed - size or disk limit";}
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
        c.text_limit(x,y,crate::office::NAMES[self.word_slot],q.ink,false,right);if self.word.dirty{c.text_limit(x+180,y,"Unsaved",q.muted,false,right);}c.round(x-6,y+18,r.w-32,68,9,q.bar);c.rect(x+182,y+25,1,22,q.edge);c.rect(x+190,y+55,1,24,q.edge);c.rect(x+334,y+55,1,24,q.edge);if r.w>=850{c.rect(x+445,y+26,1,52,q.edge);c.text_limit(x+462,y+27,"DOCUMENT",q.muted,false,right);c.text_limit(x+462,y+54,"Local editing and DOCX export",q.ink,false,right);}
        for(i,label)in ["New","Save","DOCX","Open 1","2","3","4"].iter().enumerate(){let bx=x+i*62;if bx+59<=right{self.button(Rect{x:bx,y:y+22,w:59,h:27},label,if i==1{BLUE}else{q.sub},if i==1{WHITE}else{q.ink});}}
        for(i,label)in ["B","I","U","18pt","Left","Center","Right","Undo","Redo"].iter().enumerate(){let bx=x+i*48;if bx+46<=right{let on=if i<4{self.word.style&(1<<i)!=0}else if i<7{self.word.align as usize==i-4}else{false};self.button(Rect{x:bx,y:y+53,w:46,h:28},label,if on{BLUE}else{q.sub},if on{WHITE}else{q.ink});}}
        let paper=self.word_paper();c.round(x-7,paper.y-7,r.w-30,paper.h+14,8,q.sub);c.rect(paper.x+3,paper.y+4,paper.w,paper.h,q.edge);c.rect(paper.x,paper.y,paper.w,paper.h,WHITE);
        let rows=self.word_rows();let cursor_row=self.word_row_index(&rows);let floor=paper.y+paper.h;for (ri,&(start,end,width))in rows.iter().enumerate().skip(self.word_scroll){let py=paper.y+10+(ri-self.word_scroll)*34;if py+32>floor{break;}let mut px=self.word_row_x(paper,width);for i in start..end{if self.word.selection().is_some_and(|(a,b)|a<=i&&i<b){c.rect(px,py+1,self.word_advance(i).max(1),32,[192,218,247]);}if ri==cursor_row&&i==self.word.cursor{c.rect(px,py+3,1,25,BLUE);}let(b,s)=self.word.at(i);c.office_glyph(px,py,b,s,[27,43,61],paper.x+paper.w-10,floor);px+=self.word_advance(i);}if ri==cursor_row&&self.word.cursor==end{c.rect(px,py+3,1,25,BLUE);}}
        let sy=r.y+r.h-27;c.rect(x,sy-5,r.w-44,1,q.edge);c.text_limit(x,sy,self.word_msg,q.muted,false,right.saturating_sub(105));c.num(right.saturating_sub(100),sy,self.word.word_count()as u64,q.ink);c.text(right.saturating_sub(62),sy,"words",q.muted);
    }
    fn word_click(&mut self,x:usize,y:usize){let r=self.win;let bx=r.x+22;let yy=r.y+58;if x>=bx&&y>=yy+22&&y<yy+49{match(x-bx)/62{0=>self.word.clear(),1=>self.word_save(),2=>self.word_export(),3..=6=>self.word_open((x-bx)/62-3),_=>{}}}else if x>=bx&&y>=yy+53&&y<yy+81{match(x-bx)/48{0..=3=>self.word.toggle_style(1<<((x-bx)/48)),4..=6=>self.word.alignment(((x-bx)/48-4)as u8),7=>self.word.undo(),8=>self.word.redo(),_=>{}}}else{let p=self.word_paper();if p.hit(x,y){let rows=self.word_rows();let ri=(self.word_scroll+y.saturating_sub(p.y+10)/34).min(rows.len()-1);let(start,end,width)=rows[ri];let mut px=self.word_row_x(p,width);let mut pos=end;for i in start..end{let adv=self.word_advance(i);if x<px+adv/2{pos=i;break;}px+=adv;}self.word.move_to(pos,self.term.shift);}}self.word_follow();self.draw();}

    fn load_note(&mut self){self.note.fill(0);self.note_len=0;if let Some(b)=self.files.read(){self.note[..b.len()].copy_from_slice(b);self.note_len=b.len();}}
    fn save_note(&mut self){if self.files.write(&self.note[..self.note_len]){
        self.cache_current();if self.store.save(true,&self.note[..self.note_len]){self.save_status=1;self.dirty=false;serial::line("H5K: GUI_NOTE_SAVED");self.notify("Notepad document saved");}
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
    fn close(&mut self){let animate=self.app.is_some()&&!self.minimized&&self.app!=Some(App::WinLive);let old=self.win;self.file_search_focus=false;self.context=None;self.notifications=false;self.task_switch=false;if self.app==Some(App::WinLive){crate::trial::live_close();}self.app=None;self.minimized=false;self.menu=false;self.search_focus=false;self.search_len=0;self.shell_notice=None;self.drag=None;serial::line("H5K: GUI_HOME");self.draw();if animate{self.animate_window(old,false);}}
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
    fn connect_net(&mut self){
        self.net_auto_done=true;self.net_status="Connecting via DHCP - Esc to cancel";self.draw();
        self.browser_busy_begin();
        if self.nic.is_none(){self.nic=Nic::open();}
        self.net_status=if let Some(n)=&mut self.nic{match n.configure(){Ok(())=>if n.link_up(){"IPv4 configured - link up"}else{"IPv4 configured - link down"},Err(e)=>{serial::line("H5K: DHCP_CONNECT_FAILED");serial::line(e);e}}}else{if crate::net::adapter_id()==Some(0x100e8086){"Intel E1000 initialisation failed"}else{"Unsupported or missing network adapter"}};
        if self.browser_busy_end(){self.net_status="Network connection cancelled";}self.notify(self.net_status);self.draw();
    }
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
        match key{0x38=>{if !self.alt{self.alt_apps=self.running_apps();self.alt_index=0;}self.alt=true;return;},0xb8=>{self.alt=false;return;},0x1d=>{self.ctrl=true;return;},0x9d=>{self.ctrl=false;return;},0x2a|0x36=>{self.term.shift=true;return;},0xaa|0xb6=>{self.term.shift=false;return;},_=>{}}
        if key&0x80!=0{return;}
        if self.auth.stage!=0{self.auth_key(key);return;}
        self.cancel_motion();
        if self.resize.is_some(){self.finish_resize(false);return;}if self.drag.is_some(){self.finish_drag();}
        if self.mode_preview.is_some(){if key==1||key==0x1c{self.finish_mode(key==0x1c);}return;}
        if self.shell_notice.is_some(){if key==1||key==0x1c{self.shell_notice=None;self.draw();}return;}
        if self.alt&&key==0x0f{self.cycle_task();return;}
        if self.ctrl&&self.alt&&key==0x14{self.toggle_tasks();return;}
        if self.ctrl&&self.alt&&key==0x31{self.menu=false;self.context=None;self.task_switch=false;self.notifications=!self.notifications;serial::line("H5K: GUI_NOTIFICATION_CENTER");self.draw();return;}
        if self.context.is_some()||self.notifications||self.task_switch{if key==1{self.context=None;self.notifications=false;self.task_switch=false;self.draw();}return;}
        if self.app==Some(App::Files)&&!self.minimized&&!self.menu{if self.ctrl&&key==0x21{self.file_search_focus=true;self.draw();return;}if self.file_search_focus{if key==1{self.file_search_len=0;self.file_search_focus=false;}else if key==0x1c{self.file_search_focus=false;}else if key==0x0e{self.file_search_len=self.file_search_len.saturating_sub(1);}else if !self.ctrl{if let Some(b)=ascii(key,self.term.shift){if self.file_search_len<32{self.file_search[self.file_search_len]=b;self.file_search_len+=1;}}}self.draw();return;}}
        if self.menu&&self.search_focus{if key==1{self.menu=false;self.search_focus=false;self.search_len=0;}else if key==0x1c{if let Some(i)=LABELS.iter().position(|s|app_label_matches(s,&self.search[..self.search_len])){self.select(LAUNCH[i]);return;}}else if key==0x0e{self.search_len=self.search_len.saturating_sub(1);}else if let Some(b)=ascii(key,self.term.shift){if self.search_len<32{self.search[self.search_len]=b;self.search_len+=1;}}self.draw();return;}
        if self.ctrl&&self.term.shift&&key==0x26{if self.auth.account.is_some(){self.lock_account();}else if self.app==Some(App::System)&&self.store.attached(){self.auth.stage=1;self.auth.dark=true;self.auth.msg="Choose regional preferences";self.draw();}return;}
        if self.app==Some(App::System)&&self.ctrl&&(0x02..=0x07).contains(&key){self.settings_tab=0;if self.term.shift{if key<=6{self.apply_scale([100,125,150,175,200][(key-2)as usize]);}}else{self.apply_display((key-2)as usize);}return;}
        if self.app==Some(App::WinLive){if key==1&&self.ctrl{self.close();return;}let ch=ascii(key,self.term.shift);let vk=match key{0x01=>27,0x0e=>8,0x0f=>9,0x1c=>13,0x3b..=0x44=>112+(key-0x3b)as u32,_=>ch.map_or(0,|v|v.to_ascii_uppercase()as u32)};if vk!=0{crate::trial::live_key(vk,ch);if !crate::trial::live_active(){self.close();}else{self.draw();}}return;}
        if key==0x01{if self.app==Some(App::Browser)&&self.browser_settings{self.browser_settings_toggle();}else if self.menu{self.menu=false;self.search_focus=false;self.search_len=0;self.draw();}else{self.close();}return;}
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
            0x02..=0x08=>self.select(APPS[(key-2)as usize]),0x32=>self.toggle_menu(false),
            0x2e if self.app==Some(App::Network)=>self.connect_net(),0x14=>self.theme(),0x2d if self.app.is_some()=>self.maximize(),
            0x12 if self.app==Some(App::Files)=>self.select(App::Notepad),0x31 if self.app==Some(App::Files)=>self.new_file(),0x20 if self.app==Some(App::Files)=>self.delete_file(),_=>{}
        }
    }
    fn launch_registered(&mut self){let buf=unsafe{&mut *ptr::addr_of_mut!(APP_BUFFER)};if self.store.load_7zip(buf).is_none(){self.store_msg="Install the verified package first";self.draw();return;}if crate::trial::original_gui_live(self.c.fb.width.saturating_sub(48),self.bar_y().saturating_sub(78)){self.select(App::WinLive);serial::line("H5K: STORE_GUI_LAUNCHED");}else{self.store_msg="Original GUI startup blocked";self.draw();}}
    fn package_action(&mut self,install:bool){let ok=if install{self.store.install_7zip(crate::trial::BUNDLED)}else{self.store.uninstall_7zip()};if ok{self.package_installed=install;}self.store_msg=if ok{if install{"Portable package installed"}else{"Package disabled; user files retained"}}else{"Package transaction failed"};serial::line(if ok{if install{"H5K: STORE_INSTALL_OK"}else{"H5K: STORE_UNINSTALL_OK"}}else{"H5K: STORE_ACTION_FAILED"});self.draw();}
    fn click(&mut self){
        let x=self.mx;let y=self.my;if self.auth.stage!=0{let ox=(self.c.fb.width-520)/2;let oy=(self.c.fb.height-400)/2;if x>=ox+29&&x<ox+491&&y>=oy+157&&y<oy+199&&matches!(self.auth.stage,1|6){let second=x>=ox+263;if self.auth.stage==1{self.auth.region=second as u8;serial::metric("H5K: SETUP_REGION_SELECTED=",second as u64);}else{self.auth.dark=second;serial::metric("H5K: SETUP_THEME_SELECTED=",second as u64);}self.draw();}else if x>=ox+24&&x<ox+496&&y>=oy+308&&y<oy+356{self.auth_enter();}return;}let by=self.bar_y();let dock=self.dock_x();
        if self.mode_preview.is_some(){let nx=(self.c.fb.width-456)/2;let ny=by.saturating_sub(164)/2;if y>=ny+106&&y<ny+144{if x>=nx+20&&x<nx+212{self.finish_mode(false);}else if x>=nx+244&&x<nx+436{self.finish_mode(true);}}return;}
        if self.shell_notice.is_some(){let nx=(self.c.fb.width-456)/2;let ny=by.saturating_sub(164)/2;if x>=nx+294&&x<nx+436&&y>=ny+106&&y<ny+144{self.shell_notice=None;self.draw();}return;}
        if let Some((r,window))=self.context{if r.hit(x,y)&&y>=r.y+7&&y<r.y+157{self.context_action((y-r.y-7)/30,window);}else{self.context=None;self.draw();}return;}
        if self.notifications{let r=self.notification_rect();if r.hit(x,y)&&y>=r.y+r.h-40{self.notices=[None;6];self.draw();}else{self.notifications=false;self.draw();}return;}
        if self.task_switch{let r=self.task_rect();if r.hit(x,y)&&y>=r.y+44{let apps=self.running_apps();let col=(x-r.x)/ (r.w/2);let row=(y-r.y-44)/52;if col<2&&row<6{if let Some(a)=apps[row*2+col]{self.focus_task(a);return;}}}self.task_switch=false;self.draw();return;}
        if y>=by&&y<by+60{if x<65{self.toggle_menu(false);return;}if self.search_rect().hit(x,y){self.toggle_menu(true);return;}if x>=dock&&x<dock+300{let a=PINNED[(x-dock)/50];if self.app==Some(a)&&!self.minimized{self.minimize_window();}else{self.select(a);}return;}return;}
        if y>=by{return;}
        if self.menu{let m=self.menu_rect();if m.hit(x,y){if y>=m.y+48&&y<m.y+82{self.search_focus=true;self.draw();return;}for(i,a)in LAUNCH.iter().enumerate(){if Self::menu_tile(m,i).hit(x,y)&&app_label_matches(LABELS[i],&self.search[..self.search_len]){self.select(*a);return;}}if y>=m.y+338&&y<m.y+376{if x>=m.x+20&&x<m.x+228{self.menu=false;self.search_focus=false;self.search_len=0;self.theme();}else if x>=m.x+238&&x<m.x+436{self.menu=false;if self.auth.account.is_some(){self.lock_account();}else{self.select(App::System);}}}return;}self.menu=false;self.search_focus=false;self.search_len=0;self.draw();}
        if let Some(a)=self.app{let r=self.win;if !self.minimized&&r.hit(x,y){
            if !self.maximized&&a!=App::WinLive{let edges=(if x<r.x+5{LEFT}else{0})|(if x>=r.x+r.w-5{RIGHT}else{0})|(if y<r.y+4{TOP}else{0})|(if y>=r.y+r.h-5{BOTTOM}else{0});if edges!=0{self.resize=Some(Resize{old:r,x,y,edges});self.resize_preview();return;}}
            if y<r.y+42{if x>=r.x+r.w-38{self.close();}else if x>=r.x+r.w-73{self.maximize();}else if x>=r.x+r.w-108{self.minimize_window();}else if !self.maximized{self.begin_drag(x-r.x,y-r.y);}return;}
            let bx=self.content_x(a);let yy=r.y+58;
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
            if a==App::Files{
                if x>=bx&&x<r.x+r.w-14&&y>=yy+216&&y<yy+244{self.file_search_focus=true;self.draw();return;}
                if x>=bx&&y>=yy+43&&y<yy+77{if x>=bx+356&&x<bx+456{self.explorer_grid=!self.explorer_grid;serial::line("H5K: GUI_EXPLORER_VIEW");self.draw();return;}
                    if self.explorer_tab==1{if x<bx+106{self.explorer_tab=0;self.file_search_len=0;self.draw();}}else if x<bx+106{self.new_file();}else if x>=bx+116&&x<bx+206{self.delete_file();}else if x>=bx+216&&x<bx+346{self.explorer_tab=1;self.file_search_len=0;self.draw();}return;
                }
                for i in 0..FILE_COUNT{if self.explorer_cell(i).hit(x,y)&&self.file_matches(i){if self.explorer_tab==1{self.select(App::Word);self.word_open(i);self.draw();}else if i==self.file{self.select(App::Notepad);}else{self.switch_file(i);}return;}}
                return;
            }
            if a==App::System{
                if self.settings_sidebar(){if x>=r.x+18&&x<r.x+164&&y>=yy+8&&y<yy+230{self.settings_tab=(y-yy-8)/37;self.draw();return;}
                    if self.settings_tab!=0{match self.settings_tab{1=>{if x>=bx&&x<bx+180&&y>=yy+72&&y<yy+108{self.theme();return;}if x>=bx&&x<bx+290&&y>=yy+162&&y<yy+198{self.alternate_wall=x>=bx+150;self.persist_appearance();self.draw();return;}},2=>{if x>=bx&&x<bx+180&&y>=yy+82&&y<yy+118{self.select(App::Network);return;}},3=>{if x>=bx&&x<bx+180&&y>=yy+82&&y<yy+118{if self.auth.account.is_some(){self.lock_account();}else if self.store.attached(){self.auth.stage=1;self.auth.dark=true;self.draw();}return;}},_=>{}}return;}
                }
                if x>=bx&&x<bx+426&&y>=yy+96&&y<yy+168{let row=(y-yy-96)/38;let col=(x-bx)/142;if (x-bx)%142<134&&(y-yy-96)%38<34{self.apply_display(row*3+col);return;}}
                if x>=bx&&x<bx+415&&y>=yy+228&&y<yy+260{self.apply_scale([100,125,150,175,200][(x-bx)/83]);return;}if x>=bx&&x<bx+180&&y>=yy+266&&y<yy+296{self.theme();}
            }return;
        }}
        if let Some(s)=self.background.iter().rev().flatten().find(|s|!s.minimized&&s.win.hit(x,y)).copied(){self.select(s.app);return;}
        if y<52&&x>=self.c.fb.width.saturating_sub(170){if x<self.c.fb.width-120{self.toggle_menu(true);}else if x<self.c.fb.width-70{self.notifications=true;serial::line("H5K: GUI_NOTIFICATION_CENTER");self.draw();}else if self.auth.account.is_some(){self.lock_account();}else{self.select(App::System);}return;}
        for i in 0..7{if self.rail_rect(i).hit(x,y){self.launch_rail(i);return;}}
        if cards_fit(self.c.fb.width,by)&&(self.app.is_none()||self.minimized)&&self.background.iter().all(|s|s.is_none()||s.is_some_and(|v|v.minimized)){for i in 0..4{if self.widget_rect(i).hit(x,y){match i{0=>self.show_notice("Hi5Central is not enrolled on this device."),1=>self.select(App::Network),2=>self.show_notice("Company resources require Hi5Central enrolment."),_=>self.show_notice("Company announcements are not connected.")}return;}}}
    }
    fn move_pointer(&mut self,dx:isize,dy:isize,pressed:bool,was:bool,right:bool,right_was:bool){self.pointer_event(crate::pointer::Event{x:dx as i64,y:dy as i64,absolute:false,pressed,was,right,right_was});}
    fn animate_window(&mut self,r:Rect,opening:bool){
        if !self.c.buffered||self.auth.stage!=0{return;}
        let dock=Rect{x:self.dock_x().min(self.c.fb.width-40),y:self.bar_y()-38,w:36,h:34};
        let (from,to)=if opening{(dock,r)}else{(r,dock)};
        self.motion=Some(Motion{from,to,start:p::ticks(),last:None});
    }
    fn cancel_motion(&mut self){
        if let Some(m)=self.motion.take(){self.cursor_hide();if let Some(r)=m.last{self.outline_pixels(r,true);}self.cursor_show();}
    }
    fn motion_tick(&mut self){
        let Some(mut m)=self.motion.take()else{return;};let elapsed=p::ticks().saturating_sub(m.start).min(12)as usize;
        let r=interpolate_rect(m.from,m.to,elapsed);
        if elapsed<12&&m.last==Some(r){self.motion=Some(m);return;}
        self.cursor_hide();if let Some(old)=m.last{self.outline_pixels(old,true);}
        if elapsed<12{self.outline_pixels(r,false);m.last=Some(r);self.motion=Some(m);}
        self.cursor_show();
    }
    fn outline_pixels(&self,r:Rect,erase:bool){
        if !self.c.buffered||r.w<2||r.h<2{return;}
        let right=(r.x+r.w-1).min(self.c.fb.width-1);let bottom=(r.y+r.h-1).min(self.bar_y()-1);
        let pixel=|x,y|{if erase{self.c.restore_pixel(x,y);}else{self.c.front_pixel(x,y,TEAL);}};
        for x in r.x..=right{pixel(x,r.y);pixel(x,bottom);}
        for y in r.y..=bottom{pixel(r.x,y);pixel(right,y);}
    }
    fn resize_preview(&mut self){if let Some(s)=self.resize{let r=resize_rect(s,self.mx,self.my,self.c.fb.width,self.bar_y());if self.outline!=Some(r){if let Some(old)=self.outline.take(){self.outline_pixels(old,true);}self.outline_pixels(r,false);self.outline=Some(r);}}}
    fn finish_resize(&mut self,commit:bool){if let Some(s)=self.resize.take(){self.cursor_hide();if let Some(r)=self.outline.take(){self.outline_pixels(r,true);}if commit{self.win=resize_rect(s,self.mx,self.my,self.c.fb.width,self.bar_y());self.restore=self.win;serial::metric("H5K: GUI_WINDOW_WIDTH=",self.win.w as u64);serial::metric("H5K: GUI_WINDOW_HEIGHT=",self.win.h as u64);serial::line("H5K: GUI_WINDOW_RESIZED");}self.draw();}}
    fn snap(&mut self,side:u8){if self.app.is_none()||self.minimized||self.app==Some(App::WinLive){return;}if let Some(r)=snap_rect(self.c.fb.width,self.bar_y(),side){if !self.maximized{self.restore=self.win;}self.win=r;self.maximized=side==0;serial::metric("H5K: GUI_SNAP_SIDE=",side as u64);serial::line("H5K: GUI_WINDOW_SNAPPED");self.notify("Window layout changed");self.draw();}else{self.notify("Display too narrow for side-by-side windows");self.draw();}}
    fn notify(&mut self,msg:&'static str){if self.notices[0]==Some(msg){return;}for i in (1..6).rev(){self.notices[i]=self.notices[i-1];}self.notices[0]=Some(msg);}
    fn notification_rect(&self)->Rect{let w=340.min(self.c.fb.width-20);Rect{x:self.c.fb.width-w-10,y:53,w,h:(self.bar_y()-63).min(286)}}
    fn running_apps(&self)->[Option<App>;11]{let mut v=[None;11];let mut n=0;if let Some(a)=self.app{v[n]=Some(a);n+=1;}for s in self.background.iter().rev().flatten(){if n<11{v[n]=Some(s.app);n+=1;}}v}
    fn task_rect(&self)->Rect{let w=440.min(self.c.fb.width-20);let h=390.min(self.bar_y()-20);Rect{x:(self.c.fb.width-w)/2,y:(self.bar_y()-h)/2,w,h}}
    fn toggle_tasks(&mut self){self.context=None;self.notifications=false;self.menu=false;self.task_switch=!self.task_switch;serial::line("H5K: GUI_TASK_SWITCHER");self.draw();}
    fn cycle_task(&mut self){let n=self.alt_apps.iter().flatten().count();if n==0{return;}self.alt_index=(self.alt_index+1)%n;if let Some(a)=self.alt_apps[self.alt_index]{self.focus_task(a);serial::line("H5K: GUI_ALT_TAB");}}
    fn focus_task(&mut self,a:App){self.task_switch=false;self.context=None;self.notifications=false;self.select(a);}
    fn minimize_window(&mut self){if self.app.is_some()&&!self.minimized{self.minimized=true;serial::line("H5K: GUI_MINIMIZED");let r=self.win;let animate=self.app!=Some(App::WinLive);self.draw();if animate{self.animate_window(r,false);}}}
    fn right_click(&mut self){if self.auth.stage!=0||self.mode_preview.is_some()||self.shell_notice.is_some()||self.drag.is_some()||self.resize.is_some(){return;}
        if self.app==Some(App::WinLive)&&!self.minimized{return;}
        self.menu=false;self.search_focus=false;self.notifications=false;self.task_switch=false;
        let in_window=self.app.is_some()&&!self.minimized&&self.win.hit(self.mx,self.my);
        let w=242;let h=164;
        self.context=Some((Rect{x:self.mx.min(self.c.fb.width-w-4),y:self.my.min(self.bar_y()-h-4),w,h},in_window));
        serial::line("H5K: GUI_CONTEXT_MENU");self.draw();
    }
    fn context_action(&mut self,row:usize,window:bool){self.context=None;if window{match row{0=>self.minimize_window(),1=>self.maximize(),2=>self.snap(1),3=>self.snap(2),4=>self.close(),_=>self.draw()}}else{match row{0=>self.select(App::Files),1=>self.select(App::System),2=>self.theme(),3=>{self.alternate_wall=!self.alternate_wall;self.persist_appearance();self.draw();},4=>self.toggle_tasks(),_=>self.draw()}}}
    fn begin_drag(&mut self,ox:usize,oy:usize){
        use crate::compositor::{Area,DragLayer};
        self.drag=Some((ox,oy));self.drag_target=None;self.drag_layer=None;
        let r=self.win;let by=self.bar_y();
        if self.app==Some(App::WinLive)||!self.c.buffered||self.menu||self.shell_notice.is_some()||self.mode_preview.is_some(){return;}
        let Some(window)=self.c.capture(Area{x:r.x,y:r.y,w:r.w,h:r.h})else{return;};
        self.paint_scene(false);
        let Some(backdrop)=self.c.capture(Area{x:0,y:0,w:self.c.fb.width,h:by})else{self.draw();return;};
        // Rebuild the back buffer under the existing window. The dock stays
        // outside the cached region; normal drawing resumes on release.
        self.c.compose_window(&window,r.x,r.y,by,palette(self.dark));
        self.drag_layer=Some(DragLayer{backdrop,window,frames:0,pixels:0,tick:p::ticks().saturating_sub(2)});
        serial::line("H5K: COMPOSITOR_SURFACE_READY");
    }
    fn drag_frame(&mut self,force:bool){
        use crate::compositor::Area;
        let Some((nx,ny))=self.drag_target else{return;};
        if nx==self.win.x&&ny==self.win.y{self.drag_target=None;return;}
        let now=p::ticks();
        if self.drag_layer.as_ref().is_some_and(|l|!force&&!l.due(now)){return;}
        self.drag_target=None;self.cursor_hide();
        let old=self.win;let by=self.bar_y();
        let a=Area{x:old.x,y:old.y,w:old.w,h:old.h}.shadow().union(Area{x:nx,y:ny,w:old.w,h:old.h}.shadow()).clipped(self.c.fb.width,by);
        self.win.x=nx;self.win.y=ny;
        if let Some(l)=self.drag_layer.as_mut(){
            self.c.restore_area(&l.backdrop,a);self.c.compose_window(&l.window,nx,ny,by,palette(self.dark));self.c.present_rect(a.x,a.y,a.w,a.h);
            l.tick=now;l.frames+=1;l.pixels=l.pixels.saturating_add((a.w*a.h)as u64);
            if l.frames==1{serial::line("H5K: COMPOSITOR_CACHED_FRAME");}
            self.cursor_show();
        }else{
            self.damage=None;self.draw();
        }
    }
    fn finish_drag(&mut self){
        self.drag_frame(true);
        if let Some(l)=self.drag_layer.take(){serial::metric("H5K: COMPOSITOR_DRAG_FRAMES=",l.frames);serial::metric("H5K: COMPOSITOR_DAMAGE_PIXELS=",l.pixels);}
        self.drag_target=None;self.drag=None;serial::line("H5K: GUI_WINDOW_MOVED");if self.my<=5{self.snap(0);}else if self.mx<=5&&snap_rect(self.c.fb.width,self.bar_y(),1).is_some(){self.snap(1);}else if self.mx+6>=self.c.fb.width&&snap_rect(self.c.fb.width,self.bar_y(),2).is_some(){self.snap(2);}else{self.restore=self.win;self.draw();}
    }
    fn pointer_event(&mut self,e:crate::pointer::Event){
        self.cursor_hide();self.mouse=true;let (x,y)=e.position(self.mx,self.my,self.c.fb.width,self.c.fb.height);self.mx=x;self.my=y;let (pressed,was)=(e.pressed,e.was);
        if (e.right&&!e.right_was)||(pressed&&!was){self.cancel_motion();self.cursor_hide();}
        if e.right&&!e.right_was{self.right_click();if !self.saved_valid{self.cursor_show();}return;}
        if self.resize.is_some(){if !pressed&&was{self.finish_resize(true);return;}if pressed{self.resize_preview();}if !self.saved_valid{self.cursor_show();}return;}
        if pressed&&!was{self.click();}
        if (pressed&&was)||(!pressed&&was){if let Some((ox,oy))=self.drag{let nx=self.mx.saturating_sub(ox).min(self.c.fb.width.saturating_sub(self.win.w));let ny=self.my.saturating_sub(oy).min(self.bar_y().saturating_sub(self.win.h));self.drag_target=Some((nx,ny));if pressed{self.drag_frame(false);}else{self.finish_drag();return;}}}
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
    let mut canvas=Canvas::new(fb);if let Some((mode,scale))=store.display_settings(){if let Some(actual)=(if crate::display::native_preview(fb){Some(fb)}else{crate::display::set_mode(fb,mode)}).or_else(||if Canvas::supported(fb,scale){Some(fb)}else{None}){canvas=Canvas::scaled(actual,scale);serial::line("H5K: DISPLAY_SETTINGS_RESTORED");serial::metric("H5K: DISPLAY_SCALE=",canvas.scale as u64);}}else if crate::boot::vmware()&&fb.width>=1600&&Canvas::supported(fb,125){canvas=Canvas::scaled(fb,125);}let fb=canvas.fb;
    if fb.width<640||fb.height<480{serial::line("H5K: FAIL DESKTOP_RESOLUTION");crate::kernel::halt();}
    let w=if crate::boot::vmware(){900}else{560}.min(fb.width.saturating_sub(108));let h=if crate::boot::vmware(){640}else{365}.min(dock_top(fb.height).saturating_sub(12));let win=Rect{x:(fb.width-w)/2+20,y:(dock_top(fb.height).saturating_sub(h))/2,w,h};
    let mut cache=[EMPTY_FILE;FILE_COUNT];cache[0]=FileSlot{data:files.data,len:files.len,present:files.present};
    for i in 1..FILE_COUNT{if let Some((present,data,len))=store.load(i){cache[i]=FileSlot{data,len,present};}}
    let _=store.load(0);
    let mut url=[0;128];let default=b"http://10.0.2.200:8080/hello.exe";url[..default.len()].copy_from_slice(default);
    let auth=Auth::new(&store);let prefs=store.browser_settings();let appearance=store.appearance();let mut auth=auth;if let Some(flags)=appearance{auth.dark=flags&1!=0;}let mut ui=Ui{motion:None,explorer_grid:false,file_search:[0;32],file_search_len:0,file_search_focus:false,resize:None,outline:None,context:None,notifications:false,notices:[None;6],task_switch:false,alt:false,alt_apps:[None;11],alt_index:0,drag_layer:None,drag_target:None,mode_preview:None,net_status:"Network not configured",net_auto_done:false,damage:None,alternate_wall:appearance.is_some_and(|f|f&2!=0),settings_tab:0,explorer_tab:0,search:[0;32],search_len:0,search_focus:false,shell_notice:None,last_minute:u64::MAX,word_clip:alloc::vec::Vec::new(),word:crate::office::Word::new(),word_slot:0,word_loaded:false,word_msg:"New document",word_scroll:0,auth,auto_graphics:prefs&1!=0,auto_script:false,browser_settings:false,browser_diagnostics:prefs&2!=0,tabs:[crate::http::EMPTY_LINK;4],tab_count:1,tab_index:0,page:unsafe{&mut *ptr::addr_of_mut!(crate::webpage::PAGE)},history:[crate::http::EMPTY_LINK;16],history_len:0,history_pos:0,history_replay:false,web_status:0,web_links:[crate::http::EMPTY_LINK;8],web_link_count:0,web_scroll:0,web_text:false,background:[None;10],display_msg:"Select a supported display mode",package_installed:false,store_msg:"F12 opens Hi5Store",c:canvas,app:None,minimized:false,menu:false,dark:appearance.is_some_and(|f|f&1!=0),ram,nics,files,tasks,mx:fb.width/2,my:fb.height/2,mouse:false,saved:[0;256],saved_valid:false,win,restore:win,maximized:false,drag:None,term:Terminal::new(),sheet:crate::sheet::Sheet::new(),sheet_result:0,sheet_cell:0,sheet_edit:[0;48],sheet_len:0,sheet_msg:"Select cell and type value or formula",pe_report:crate::imports::Report::default(),rebase:crate::relocate::Report::default(),store,note:[0;CAP],note_len:0,dirty:false,save_status:0,ctrl:false,file:0,cache,nic:None,url,url_len:default.len(),web:unsafe{&mut *ptr::addr_of_mut!(WEB_BUFFER)},source:[0;128],source_len:0,web_len:0,web_ok:false,web_msg:"Offline until Go is pressed"};
    crate::display::log_adapter();crate::display::log_runtime(ui.c.physical);crate::net::log_adapters();if appearance.is_some(){serial::line("H5K: APPEARANCE_RESTORED");}
    ui.mouse=init_input();serial::line(if ui.mouse{"H5K: PS2_MOUSE_ACK"}else{"H5K: PS2_MOUSE_UNAVAILABLE"});
    if crate::pointer::init(){ui.mouse=true;serial::line("H5K: ABS_POINTER_READY");}else{serial::line("H5K: REL_POINTER_READY");}
    serial::line(if ui.c.buffered{"H5K: DOUBLE_BUFFER_OK"}else{"H5K: DIRECT_RENDER_FALLBACK"});
    ui.draw();if ui.auth.stage==1{serial::line("H5K: ACCOUNT_SETUP_READY");}else if ui.auth.stage==7{serial::line("H5K: ACCOUNT_LOGIN_READY");}serial::line("H5K: FRAMEBUFFER_OK");serial::line("H5K: DESKTOP_READY");
    p::timer_init();let start=p::ticks();while p::ticks().wrapping_sub(start)<3{p::wait_tick();}
    serial::line("H5K: TIMER_IRQ_OK");if !crate::net::self_test()||!crate::http::navigation_test()||!crate::account::self_test()||!crate::webpage::self_test(){serial::line("H5K: FAIL NETWORK_FORMAT_TEST");crate::kernel::halt();}serial::line("H5K: NET_FORMAT_TEST_OK");serial::line("H5K: BROWSER_LAYOUT_SELFTEST_OK");serial::line("H5K: KERNEL_READY");
    let mut ext=false;
    loop{match ui.tasks.dispatch(){
        0=>{for _ in 0..32{let st=p::input(0x64);if st&1==0{break;}let b=p::input(0x60);if st&0x20!=0{
            if let Some((dx,dy,pressed,was,right,right_was))=mouse_byte(b){ui.move_pointer(dx,dy,pressed,was,right,right_was);}
        }else if b==0xe0{ext=true;}else{if !ext{ui.key(b);}else if ui.auth.stage!=0{ }else if ui.app==Some(App::WinLive)&&b&0x80==0{let vk=match b{0x48=>38,0x50=>40,0x4b=>37,0x4d=>39,0x47=>36,0x4f=>35,0x49=>33,0x51=>34,0x53=>46,_=>0};if vk!=0{crate::trial::live_key(vk,None);if !crate::trial::live_active(){ui.close();}else{ui.draw();}}}else if ui.app==Some(App::Word)&&b&0x80==0{ui.word_extended(b);}else if ui.app==Some(App::Browser)&&ui.web_text&&b&0x80==0{if b==0x49||b==0x48{ui.web_scroll=ui.web_scroll.saturating_sub(if b==0x49{5}else{1});ui.draw();}else if b==0x51||b==0x50{ui.web_scroll=(ui.web_scroll+if b==0x51{5}else{1}).min(ui.page.rows(ui.win.w.saturating_sub(57)).saturating_sub(1));ui.draw();}}else if ui.app==Some(App::Terminal)&&ui.term.result==24{if b==0x49{ui.term.scroll=ui.term.scroll.saturating_sub(5);ui.draw();}else if b==0x51{let n=crate::trial::output_bytes().iter().filter(|c|**c==10).count();ui.term.scroll=(ui.term.scroll+5).min(n.saturating_sub(1));ui.draw();}}ext=false;}}}
        1=>{if ui.mode_preview.is_some_and(|v|p::ticks()>=v.2){ui.finish_mode(false);}
            if ui.drag.is_some(){ui.drag_frame(false);}else if ui.resize.is_none(){ui.motion_tick();}
            for _ in 0..32{let Some(e)=crate::pointer::next()else{break;};ui.pointer_event(e);}
            if crate::boot::vmware()&&ui.auth.stage==0&&!ui.net_auto_done&&ui.mode_preview.is_none(){ui.net_auto_done=true;if crate::net::adapter_id()==Some(0x100e8086){ui.connect_net();}else{ui.net_status="No supported Intel E1000 adapter";ui.draw();}}
            if ui.auth.stage==0&&ui.drag.is_none()&&ui.resize.is_none()&&ui.app!=Some(App::WinLive)&&crate::security::now().is_some_and(|t|t/60!=ui.last_minute){ui.draw();}
            if ui.auth.stage==0&&ui.app==Some(App::WinLive){crate::trial::live_tick();if !crate::trial::live_active(){ui.close();}}else{core::hint::spin_loop();}},_=>p::wait_tick(),
    }}
}
