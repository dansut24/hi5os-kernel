//! Bounded single-thread Win32 object/message foundation; no fabricated GUI rendering.
#[derive(Clone,Copy)]struct Class{atom:u16,proc:u64,name:[u8;64],len:usize}
const CLASS:Class=Class{atom:0,proc:0,name:[0;64],len:0};
#[derive(Clone,Copy)]struct Window{handle:u64,atom:u16,parent:u64,rect:[i32;4],visible:bool,menu:u64,id:u64,user:u64,style:u32,ex:u32}
const WINDOW:Window=Window{handle:0,atom:0,parent:0,rect:[0;4],visible:false,menu:0,id:0,user:0,style:0,ex:0};
#[derive(Clone,Copy,PartialEq,Debug)]pub struct Message{pub hwnd:u64,pub id:u32,pub wparam:u64,pub lparam:u64}
const MSG:Message=Message{hwnd:0,id:0,wparam:0,lparam:0};
pub struct Desktop{classes:[Class;16],windows:[Window;32],q:[Message;128],head:usize,len:usize,next:u64,focus:u64}
impl Desktop{
    pub const fn new()->Self{Self{classes:[CLASS;16],windows:[WINDOW;32],q:[MSG;128],head:0,len:0,next:1,focus:0}}
    pub fn register(&mut self,name:&[u8],proc:u64)->Option<u16>{
        if name.is_empty()||name.len()>64||!name.iter().all(|c|(32..127).contains(c))||proc==0||self.classes.iter().any(|c|c.atom!=0&&c.name[..c.len].eq_ignore_ascii_case(name)){return None;}
        let i=self.classes.iter().position(|c|c.atom==0)?;let atom=(i+1)as u16;let c=&mut self.classes[i];c.atom=atom;c.proc=proc;c.len=name.len();c.name[..name.len()].copy_from_slice(name);Some(atom)
    }
    pub fn atom(&self,name:&[u8])->Option<u16>{self.classes.iter().find(|c|c.atom!=0&&c.name[..c.len].eq_ignore_ascii_case(name)).map(|c|c.atom)}
    pub fn visible(&self,h:u64)->bool{self.window(h).is_some_and(|i|self.windows[i].visible)}
    fn window(&self,h:u64)->Option<usize>{if h==0{return None;}self.windows.iter().position(|w|w.handle==h)}
    pub fn create(&mut self,atom:u16,parent:u64,rect:[i32;4])->Option<u64>{
        self.create_mode(atom,parent,rect,true)
    }
    pub fn create_sync(&mut self,atom:u16,parent:u64,rect:[i32;4])->Option<u64>{
        self.create_mode(atom,parent,rect,false)
    }
    fn create_mode(&mut self,atom:u16,parent:u64,rect:[i32;4],queued:bool)->Option<u64>{
        if atom==0||!self.classes.iter().any(|c|c.atom==atom)||parent!=0&&self.window(parent).is_none()||rect[2]<0||rect[3]<0||parent==0&&(rect[2]==0||rect[3]==0)||rect[2]>16384||rect[3]>16384||self.len==128{return None;}
        let i=self.windows.iter().position(|w|w.handle==0)?;let h=self.next;self.next=self.next.checked_add(1)?;
        self.windows[i]=Window{handle:h,atom,parent,rect,visible:false,..WINDOW};if queued{self.post(Message{hwnd:h,id:1,wparam:0,lparam:0});}Some(h)
    }
    pub fn procedure(&self,h:u64)->Option<u64>{let w=self.windows[self.window(h)?];self.classes.iter().find(|c|c.atom==w.atom).map(|c|c.proc)}
    pub fn show(&mut self,h:u64,visible:bool)->bool{
        let Some(i)=self.window(h)else{return false;};if self.len==128{return false;}
        self.windows[i].visible=visible;self.post(Message{hwnd:h,id:0x18,wparam:visible as u64,lparam:0})
    }
    pub fn rect(&self,h:u64)->Option<[i32;4]>{Some(self.windows[self.window(h)?].rect)}
    pub fn configure(&mut self,h:u64,style:u32,ex:u32,menu_or_id:u64)->bool{
        let Some(i)=self.window(h)else{return false;};let w=&mut self.windows[i];
        w.style=style;w.ex=ex;w.visible=style&0x10000000!=0;
        if w.parent==0{w.menu=menu_or_id;}else{w.id=menu_or_id;}true
    }
    pub fn parent(&self,h:u64)->Option<u64>{Some(self.windows[self.window(h)?].parent)}
    pub fn menu(&self,h:u64)->Option<u64>{Some(self.windows[self.window(h)?].menu)}
    pub fn set_menu(&mut self,h:u64,menu:u64)->bool{let Some(i)=self.window(h)else{return false;};if self.windows[i].parent!=0{return false;}self.windows[i].menu=menu;true}
    pub fn id(&self,h:u64)->Option<u64>{Some(self.windows[self.window(h)?].id)}
    pub fn long(&self,h:u64,index:i32)->Option<u64>{let w=self.windows[self.window(h)?];match index{-21=>Some(w.user),-16=>Some(w.style as u64),-20=>Some(w.ex as u64),-12=>Some(w.id),_=>None}}
    pub fn set_user(&mut self,h:u64,value:u64)->Option<u64>{let i=self.window(h)?;let old=self.windows[i].user;self.windows[i].user=value;Some(old)}
    pub fn screen_rect(&self,h:u64)->Option<[i32;4]>{
        let w=self.windows[self.window(h)?];let mut x=w.rect[0];let mut y=w.rect[1];let mut parent=w.parent;
        for _ in 0..32{if parent==0{return Some([x,y,x.checked_add(w.rect[2])?,y.checked_add(w.rect[3])?]);}let p=self.windows[self.window(parent)?];x=x.checked_add(p.rect[0])?;y=y.checked_add(p.rect[1])?;parent=p.parent;}None
    }
    // Roll back a rejected creation subtree without claiming WM_DESTROY delivery.
    pub fn discard_tree(&mut self,h:u64)->bool{
        if self.window(h).is_none(){return false;}
        while let Some(child)=self.windows.iter().find(|w|w.handle!=0&&w.parent==h).map(|w|w.handle){if !self.discard_tree(child){return false;}}
        self.destroy(h)
    }
    // Geometry and focus state only; resize/focus notification delivery and paint remain separate.
    pub fn move_window(&mut self,h:u64,x:i32,y:i32,w:i32,height:i32)->bool{
        let Some(i)=self.window(h)else{return false;};if w<0||height<0||w>16384||height>16384{return false;}
        self.windows[i].rect=[x,y,w,height];true
    }
    pub fn focus(&self)->u64{self.focus}
    pub fn set_focus(&mut self,h:u64)->Option<u64>{if h!=0&&self.window(h).is_none(){return None;}let old=self.focus;self.focus=h;Some(old)}
    pub fn post(&mut self,m:Message)->bool{
        if self.len==128||m.hwnd!=0&&self.window(m.hwnd).is_none(){return false;}
        self.q[(self.head+self.len)%128]=m;self.len+=1;true
    }
    pub fn peek(&mut self,remove:bool)->Option<Message>{if self.len==0{return None;}let m=self.q[self.head];if remove{self.head=(self.head+1)%128;self.len-=1;}Some(m)}
    pub fn destroy(&mut self,h:u64)->bool{
        let Some(i)=self.window(h)else{return false;};if self.windows.iter().any(|w|w.handle!=0&&w.parent==h){return false;}
        // Do not deliver stale messages for destroyed handles; no callback dispatched here.
        let count=self.len;for _ in 0..count{let m=self.peek(true).unwrap();if m.hwnd!=h{self.post(m);}}
        if self.focus==h{self.focus=0;}self.windows[i]=WINDOW;true
    }
}
pub fn self_test()->bool{
    let mut d=Desktop::new();let Some(a)=d.register(b"Hi5Window",0x1000)else{return false;};
    if d.register(b"hi5window",0x2000).is_some(){return false;}
    let Some(h)=d.create(a,0,[0,0,320,240])else{return false;};
    let Some(child)=d.create(a,h,[1,1,20,20])else{return false;};
    if d.procedure(h)!=Some(0x1000)||d.rect(h)!=Some([0,0,320,240])||d.destroy(h)||!d.show(h,true){return false;}
    if !d.configure(child,0x50000000,0,77)||d.parent(child)!=Some(h)||d.id(child)!=Some(77)||d.screen_rect(child)!=Some([1,1,21,21])||d.set_user(child,88)!=Some(0)||d.long(child,-21)!=Some(88){return false;}
    if d.set_focus(child)!=Some(0)||d.focus()!=child||!d.move_window(child,2,3,30,40)||d.screen_rect(child)!=Some([2,3,32,43]){return false;}
    let Some(first)=d.peek(false)else{return false;};if first.id!=1||d.peek(true)!=Some(first){return false;}
    if !d.destroy(child)||!d.destroy(h)||d.peek(true).is_some()||d.post(Message{hwnd:h,id:0,wparam:0,lparam:0}){return false;}
    for i in 0..128{if !d.post(Message{hwnd:0,id:i,wparam:0,lparam:0}){return false;}}
    !d.post(MSG)&&(0..128).all(|i|d.peek(true).is_some_and(|m|m.id==i))&&d.peek(true).is_none()
}
