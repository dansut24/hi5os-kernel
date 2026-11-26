//! Bounded Win32 standard menu template and mutable menu tree. No rendering or owner-draw.
pub const LABEL:usize=128;
const MAX_MENUS:usize=48;
const MAX_ITEMS:usize=48;
#[derive(Clone,Copy)]pub struct Item{pub typ:u32,pub state:u32,pub id:u32,pub sub:u64,pub len:usize,pub text:[u16;LABEL]}
pub const EMPTY:Item=Item{typ:0,state:0,id:0,sub:0,len:0,text:[0;LABEL]};
struct Menu{handle:u64,len:usize,items:[Item;MAX_ITEMS]}
const MENU:Menu=Menu{handle:0,len:0,items:[EMPTY;MAX_ITEMS]};
pub struct Menus{slots:[Menu;MAX_MENUS],next:u64}
impl Menus{
    pub const fn new()->Self{Self{slots:[MENU;MAX_MENUS],next:0x52000000}}
    pub fn reset(&mut self){for m in &mut self.slots{m.handle=0;m.len=0;}self.next=0x52000000;}
    fn index(&self,h:u64)->Option<usize>{if h==0{return None;}self.slots.iter().position(|m|m.handle==h)}
    pub fn valid(&self,h:u64)->bool{self.index(h).is_some()}
    pub fn create(&mut self)->Option<u64>{let i=self.slots.iter().position(|m|m.handle==0)?;let h=self.next;self.next=self.next.checked_add(1)?;self.slots[i].handle=h;self.slots[i].len=0;Some(h)}
    pub fn count(&self,h:u64)->Option<usize>{Some(self.slots[self.index(h)?].len)}
    fn pos(&self,h:u64,key:u32,by_pos:bool)->Option<usize>{let m=&self.slots[self.index(h)?];if by_pos{((key as usize)<m.len).then_some(key as usize)}else{m.items[..m.len].iter().position(|v|v.id==key&&v.sub==0)}}
    pub fn get(&self,h:u64,key:u32,by_pos:bool)->Option<Item>{let p=self.pos(h,key,by_pos)?;Some(self.slots[self.index(h)?].items[p])}
    fn attached(&self,h:u64)->bool{self.slots.iter().any(|m|m.handle!=0&&m.items[..m.len].iter().any(|i|i.sub==h))}
    fn reaches(&self,h:u64,target:u64,depth:usize)->bool{if h==target||depth>=MAX_MENUS{return true;}let Some(i)=self.index(h)else{return false;};self.slots[i].items[..self.slots[i].len].iter().any(|v|v.sub!=0&&self.reaches(v.sub,target,depth+1))}
    fn acceptable(&self,h:u64,item:Item,old:u64)->bool{item.len<LABEL&&item.typ&!0x0b60==0&&item.state&!0x108b==0&&(item.sub==0||self.valid(item.sub)&&!self.reaches(item.sub,h,0)&&(item.sub==old||!self.attached(item.sub)))}
    pub fn insert(&mut self,h:u64,key:u32,by_pos:bool,item:Item)->bool{
        let Some(i)=self.index(h)else{return false;};let n=self.slots[i].len;if n==MAX_ITEMS||!self.acceptable(h,item,0){return false;}
        let p=if by_pos{(key as usize).min(n)}else{let Some(p)=self.pos(h,key,false)else{return false;};p};
        let m=&mut self.slots[i];m.items.copy_within(p..n,p+1);m.items[p]=item;m.len+=1;true
    }
    pub fn set(&mut self,h:u64,key:u32,by_pos:bool,item:Item)->bool{let Some(i)=self.index(h)else{return false;};let Some(p)=self.pos(h,key,by_pos)else{return false;};if !self.acceptable(h,item,self.slots[i].items[p].sub){return false;}self.slots[i].items[p]=item;true}
    pub fn remove(&mut self,h:u64,key:u32,by_pos:bool,destroy:bool)->bool{let Some(i)=self.index(h)else{return false;};let Some(p)=self.pos(h,key,by_pos)else{return false;};let m=&mut self.slots[i];let sub=m.items[p].sub;m.items.copy_within(p+1..m.len,p);m.len-=1;if destroy&&sub!=0{self.destroy(sub);}true}
    pub fn destroy(&mut self,h:u64)->bool{if self.attached(h){return false;}self.destroy_inner(h)}
    fn destroy_inner(&mut self,h:u64)->bool{let Some(i)=self.index(h)else{return false;};let n=self.slots[i].len;self.slots[i].handle=0;self.slots[i].len=0;for p in 0..n{let sub=self.slots[i].items[p].sub;if sub!=0{self.destroy_inner(sub);}}true}
    pub fn load(&mut self,data:&[u8])->Option<u64>{
        let version=word(data,0)?;let mut pos=4usize.checked_add(word(data,2)? as usize)?;let start=self.next;
        let result=match version{0=>self.parse(data,&mut pos,0),1 if pos>=8&&pos%4==0=>self.parse_ex(data,&mut pos,0),_=>None};
        if result.is_none(){for m in &mut self.slots{if m.handle>=start{m.handle=0;m.len=0;}}}
        result
    }
    // MENUEX_TEMPLATE_HEADER (version 1), followed by aligned MENUEX_TEMPLATE_ITEMs.
    fn parse_ex(&mut self,data:&[u8],p:&mut usize,depth:usize)->Option<u64>{
        if depth>=8{return None;}let h=self.create()?;
        loop{
            *p=align4(*p)?;let typ=dword(data,*p)?;let state=dword(data,(*p).checked_add(4)?)?;let id=dword(data,(*p).checked_add(8)?)?;
            *p=(*p).checked_add(12)?;let flags=take(data,p)?;if flags&!0x81!=0{return None;}
            let mut item=Item{typ,state,id,..EMPTY};
            loop{let c=take(data,p)?;if c==0{break;}if item.len==LABEL-1{return None;}item.text[item.len]=c;item.len+=1;}
            *p=align4(*p)?;
            if flags&1!=0{let _help=dword(data,*p)?;*p=(*p).checked_add(4)?;item.sub=self.parse_ex(data,p,depth+1)?;}
            if !self.insert(h,u32::MAX,true,item){return None;}if flags&0x80!=0{break;}
        }Some(h)
    }
    fn parse(&mut self,data:&[u8],p:&mut usize,depth:usize)->Option<u64>{
        if depth>=8{return None;}let h=self.create()?;
        loop{let flags=take(data,p)? as u32;let popup=flags&0x10!=0;let id=if popup{0}else{take(data,p)? as u32};let mut item=Item{typ:flags&0x0b60,state:flags&0x0b,id,..EMPTY};
            loop{let c=take(data,p)?;if c==0{break;}if item.len==LABEL-1{return None;}item.text[item.len]=c;item.len+=1;}
            if !popup&&id==0&&item.len==0{item.typ|=0x800;}
            if popup{item.sub=self.parse(data,p,depth+1)?;}
            if !self.insert(h,u32::MAX,true,item){return None;}if flags&0x80!=0{break;}
        }Some(h)
    }
}
fn align4(p:usize)->Option<usize>{Some(p.checked_add(3)?&!3)}
fn dword(b:&[u8],p:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(p..p.checked_add(4)?)?.try_into().ok()?))}
fn word(b:&[u8],p:usize)->Option<u16>{Some(u16::from_le_bytes(b.get(p..p.checked_add(2)?)?.try_into().ok()?))}
fn take(b:&[u8],p:&mut usize)->Option<u16>{let v=word(b,*p)?;*p=(*p).checked_add(2)?;Some(v)}
pub fn self_test(m:&mut Menus)->bool{
    m.reset();
    // Standard MENU template: File popup containing Open and a separator.
    let data:[u16;18]=[0,0,0x90,70,105,108,101,0,0,42,79,112,101,110,0,0x80,0,0];
    let b=unsafe{core::slice::from_raw_parts(data.as_ptr().cast::<u8>(),data.len()*2)};
    let Some(h)=m.load(b)else{return false;};let Some(file)=m.get(h,0,true)else{return false;};
    if m.count(h)!=Some(1)||m.count(file.sub)!=Some(2)||m.get(file.sub,42,false).map(|i|i.len)!=Some(4)||m.get(file.sub,1,true).map(|i|i.typ)!=Some(0x800){return false;}
    let Some(copy)=m.create()else{return false;};let mut v=m.get(file.sub,0,true).unwrap();v.state=8;
    if !m.insert(copy,0,true,v)||m.get(copy,42,false).map(|i|i.state)!=Some(8)||m.insert(file.sub,0,true,Item{sub:h,..EMPTY})||m.destroy(file.sub)||!m.remove(copy,0,true,false)||m.count(copy)!=Some(0){return false;}
    if m.load(&b[..b.len()-2]).is_some()||!m.destroy(h)||m.valid(file.sub)||!m.destroy(copy){return false;}
    // MENUEX with popup id=500, child id=42, checked state and UTF16 text.
    let ex:[u16;26]=[1,4,0,0, 0,0,0,0,500,0,0x81,70,0,0, 0,0, 0,0,8,0,42,0,0x80,79,0,0];
    let b=unsafe{core::slice::from_raw_parts(ex.as_ptr().cast::<u8>(),ex.len()*2)};
    let Some(h)=m.load(b)else{return false;};let Some(v)=m.get(h,0,true)else{return false;};
    if v.id!=500||m.get(v.sub,0,true).map(|i|(i.id,i.state,i.text[0]))!=Some((42,8,79))||m.load(&b[..b.len()-4]).is_some()||!m.destroy(h){return false;}m.reset();true
}
