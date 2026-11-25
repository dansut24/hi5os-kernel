//! Bounded single-thread Win32 object/message foundation; no fabricated GUI rendering.
#[derive(Clone,Copy)]struct Class{atom:u16,proc:u64,name:[u8;64],len:usize}
const CLASS:Class=Class{atom:0,proc:0,name:[0;64],len:0};
#[derive(Clone,Copy)]struct Window{handle:u64,atom:u16,parent:u64,rect:[i32;4],visible:bool}
const WINDOW:Window=Window{handle:0,atom:0,parent:0,rect:[0;4],visible:false};
#[derive(Clone,Copy,PartialEq,Debug)]pub struct Message{pub hwnd:u64,pub id:u32,pub wparam:u64,pub lparam:u64}
const MSG:Message=Message{hwnd:0,id:0,wparam:0,lparam:0};
pub struct Desktop{classes:[Class;16],windows:[Window;32],q:[Message;128],head:usize,len:usize,next:u64}
impl Desktop{
    pub const fn new()->Self{Self{classes:[CLASS;16],windows:[WINDOW;32],q:[MSG;128],head:0,len:0,next:1}}
    pub fn register(&mut self,name:&[u8],proc:u64)->Option<u16>{
        if name.is_empty()||name.len()>64||!name.iter().all(|c|(32..127).contains(c))||proc==0||self.classes.iter().any(|c|c.atom!=0&&c.name[..c.len].eq_ignore_ascii_case(name)){return None;}
        let i=self.classes.iter().position(|c|c.atom==0)?;let atom=(i+1)as u16;let c=&mut self.classes[i];c.atom=atom;c.proc=proc;c.len=name.len();c.name[..name.len()].copy_from_slice(name);Some(atom)
    }
    fn window(&self,h:u64)->Option<usize>{if h==0{return None;}self.windows.iter().position(|w|w.handle==h)}
    pub fn create(&mut self,atom:u16,parent:u64,rect:[i32;4])->Option<u64>{
        if atom==0||!self.classes.iter().any(|c|c.atom==atom)||parent!=0&&self.window(parent).is_none()||rect[2]<=0||rect[3]<=0||rect[2]>16384||rect[3]>16384||self.len==128{return None;}
        let i=self.windows.iter().position(|w|w.handle==0)?;let h=self.next;self.next=self.next.checked_add(1)?;
        self.windows[i]=Window{handle:h,atom,parent,rect,visible:false};self.post(Message{hwnd:h,id:1,wparam:0,lparam:0});Some(h)
    }
    pub fn procedure(&self,h:u64)->Option<u64>{let w=self.windows[self.window(h)?];self.classes.iter().find(|c|c.atom==w.atom).map(|c|c.proc)}
    pub fn show(&mut self,h:u64,visible:bool)->bool{
        let Some(i)=self.window(h)else{return false;};if self.len==128{return false;}
        self.windows[i].visible=visible;self.post(Message{hwnd:h,id:0x18,wparam:visible as u64,lparam:0})
    }
    pub fn rect(&self,h:u64)->Option<[i32;4]>{Some(self.windows[self.window(h)?].rect)}
    pub fn post(&mut self,m:Message)->bool{
        if self.len==128||m.hwnd!=0&&self.window(m.hwnd).is_none(){return false;}
        self.q[(self.head+self.len)%128]=m;self.len+=1;true
    }
    pub fn peek(&mut self,remove:bool)->Option<Message>{if self.len==0{return None;}let m=self.q[self.head];if remove{self.head=(self.head+1)%128;self.len-=1;}Some(m)}
    pub fn destroy(&mut self,h:u64)->bool{
        let Some(i)=self.window(h)else{return false;};if self.windows.iter().any(|w|w.handle!=0&&w.parent==h){return false;}
        // Do not deliver stale messages for destroyed handles; no callback dispatched here.
        let count=self.len;for _ in 0..count{let m=self.peek(true).unwrap();if m.hwnd!=h{self.post(m);}}
        self.windows[i]=WINDOW;true
    }
}
pub fn self_test()->bool{
    let mut d=Desktop::new();let Some(a)=d.register(b"Hi5Window",0x1000)else{return false;};
    if d.register(b"hi5window",0x2000).is_some(){return false;}
    let Some(h)=d.create(a,0,[0,0,320,240])else{return false;};
    let Some(child)=d.create(a,h,[1,1,20,20])else{return false;};
    if d.procedure(h)!=Some(0x1000)||d.rect(h)!=Some([0,0,320,240])||d.destroy(h)||!d.show(h,true){return false;}
    let Some(first)=d.peek(false)else{return false;};if first.id!=1||d.peek(true)!=Some(first){return false;}
    if !d.destroy(child)||!d.destroy(h)||d.peek(true).is_some()||d.post(Message{hwnd:h,id:0,wparam:0,lparam:0}){return false;}
    for i in 0..128{if !d.post(Message{hwnd:0,id:i,wparam:0,lparam:0}){return false;}}
    !d.post(MSG)&&(0..128).all(|i|d.peek(true).is_some_and(|m|m.id==i))&&d.peek(true).is_none()
}
