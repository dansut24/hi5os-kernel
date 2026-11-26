//! Bounded native control state; no claims of on-screen rendering yet.
pub const TOOLBAR_PROC:u64=u64::MAX;
#[derive(Clone,Copy)]pub struct Button{pub image:i32,pub command:i32,pub state:u8,pub style:u8,pub data:u64,pub len:usize,pub text:[u16;96]}
pub const BUTTON:Button=Button{image:0,command:0,state:0,style:0,data:0,len:0,text:[0;96]};
#[derive(Clone,Copy)]struct Bar{hwnd:u64,images:u64,abi:bool,len:usize,buttons:[Button;32]}
const BAR:Bar=Bar{hwnd:0,images:0,abi:false,len:0,buttons:[BUTTON;32]};
#[derive(Clone,Copy)]pub struct Image{pub offset:usize,pub len:usize,pub mask:u32}
const IMAGE:Image=Image{offset:0,len:0,mask:0};
#[derive(Clone,Copy)]struct List{handle:u64,w:u32,h:u32,len:usize,images:[Image;64]}
const LIST:List=List{handle:0,w:0,h:0,len:0,images:[IMAGE;64]};
pub struct Controls{bars:[Bar;8],lists:[List;8],next:u64}
impl Controls{
    pub const fn new()->Self{Self{bars:[BAR;8],lists:[LIST;8],next:0x53000000}}
    pub fn reset(&mut self){*self=Self::new();}
    pub fn add_bar(&mut self,h:u64)->bool{if h==0||self.bar(h).is_some(){return false;}let Some(b)=self.bars.iter_mut().find(|b|b.hwnd==0)else{return false;};*b=Bar{hwnd:h,..BAR};true}
    fn bar(&self,h:u64)->Option<usize>{if h==0{return None;}self.bars.iter().position(|b|b.hwnd==h)}
    pub fn is_bar(&self,h:u64)->bool{self.bar(h).is_some()}
    pub fn abi(&mut self,h:u64,size:u64)->bool{let Some(i)=self.bar(h)else{return false;};if size!=32{return false;}self.bars[i].abi=true;true}
    pub fn set_images(&mut self,h:u64,list:u64)->Option<u64>{let i=self.bar(h)?;if list!=0&&self.list(list).is_none(){return None;}let old=self.bars[i].images;self.bars[i].images=list;Some(old)}
    pub fn add_buttons(&mut self,h:u64,b:&[Button])->bool{let Some(i)=self.bar(h)else{return false;};let v=&mut self.bars[i];if !v.abi||b.len()>32-v.len||b.iter().any(|x|x.len>=96){return false;}v.buttons[v.len..v.len+b.len()].copy_from_slice(b);v.len+=b.len();true}
    pub fn button(&self,h:u64,n:usize)->Option<(Button,Option<Image>)>{let b=&self.bars[self.bar(h)?];if n>=b.len{return None;}let v=b.buttons[n];let image=if v.image>=0{self.list(b.images).and_then(|i|{let l=&self.lists[i];((v.image as usize)<l.len).then(||l.images[v.image as usize])})}else{None};Some((v,image))}
    pub fn bar_at(&self,n:usize)->Option<u64>{self.bars.get(n).map(|b|b.hwnd).filter(|h|*h!=0)}
    pub fn enable(&mut self,h:u64,id:i32,enabled:bool)->bool{let Some(i)=self.bar(h)else{return false;};let b=&mut self.bars[i];let Some(v)=b.buttons[..b.len].iter_mut().find(|v|v.command==id)else{return false;};if enabled{v.state|=4;}else{v.state&=!4;}true}
    pub fn button_count(&self,h:u64)->Option<usize>{Some(self.bars[self.bar(h)?].len)}
    pub fn dimensions(&self,h:u64)->Option<(i32,i32)>{let b=&self.bars[self.bar(h)?];let (w,h)=if let Some(i)=self.list(b.images){(self.lists[i].w,self.lists[i].h)}else{(24,24)};Some(((w as usize+12).checked_mul(b.len)? as i32,h as i32+12))}
    fn list(&self,h:u64)->Option<usize>{if h==0{return None;}self.lists.iter().position(|l|l.handle==h)}
    pub fn create_list(&mut self,w:u32,h:u32)->Option<u64>{if w==0||h==0||w>128||h>128{return None;}let i=self.lists.iter().position(|l|l.handle==0)?;let handle=self.next;self.next=self.next.checked_add(1)?;self.lists[i]=List{handle,w,h,..LIST};Some(handle)}
    pub fn count(&self,h:u64)->Option<usize>{Some(self.lists[self.list(h)?].len)}
    pub fn add_image(&mut self,h:u64,image:Image,w:u32,height:u32)->Option<usize>{let i=self.list(h)?;let l=&mut self.lists[i];if w!=l.w||height!=l.h||l.len==64||image.len==0{return None;}let n=l.len;l.images[n]=image;l.len+=1;Some(n)}
    pub fn destroy_list(&mut self,h:u64)->bool{let Some(i)=self.list(h)else{return false;};if self.bars.iter().any(|b|b.hwnd!=0&&b.images==h){return false;}self.lists[i]=LIST;true}
}
pub fn dib_size(data:&[u8])->Option<(u32,u32)>{
    let d=|at:usize|Some(u32::from_le_bytes(data.get(at..at+4)?.try_into().ok()?));
    let header=d(0)? as usize;if header<40||header>data.len(){return None;}let w=d(4)? as i32;let h=d(8)? as i32;
    let planes=u16::from_le_bytes(data.get(12..14)?.try_into().ok()?);let bpp=u16::from_le_bytes(data.get(14..16)?.try_into().ok()?);
    if w<=0||h==0||w>128||h.unsigned_abs()>128||planes!=1||!matches!(bpp,1|4|8|16|24|32)||d(16)?!=0{return None;}
    let colors=if bpp<=8{let used=d(32)?;if used==0{1usize<<bpp}else{if used>1<<bpp{return None;}used as usize}}else{d(32)? as usize};
    let stride=(w as usize*bpp as usize).div_ceil(32)*4;let end=header.checked_add(colors.checked_mul(4)?)?.checked_add(stride.checked_mul(h.unsigned_abs()as usize)?)?;
    if end>data.len(){return None;}Some((w as u32,h.unsigned_abs()))
}
pub fn dib_pixel(data:&[u8],x:usize,y:usize)->Option<[u8;3]>{
    let (w,h)=dib_size(data)?;if x>=w as usize||y>=h as usize{return None;}
    let d=|at:usize|Some(u32::from_le_bytes(data.get(at..at+4)?.try_into().ok()?));
    let header=d(0)?as usize;let bpp=u16::from_le_bytes(data.get(14..16)?.try_into().ok()?)as usize;
    let used=d(32)?as usize;let colors=if bpp<=8&&used==0{1usize<<bpp}else{used};
    let row=if (d(8)?as i32)<0{y}else{h as usize-1-y};let stride=(w as usize*bpp).div_ceil(32)*4;
    let start=header+colors*4+row*stride;
    if bpp<=8{let v=*data.get(start+x*bpp/8)?;let shift=8-bpp-(x*bpp%8);let i=(v as usize>>shift)&((1<<bpp)-1);if i>=colors{return None;}let p=data.get(header+i*4..header+i*4+3)?;return Some([p[2],p[1],p[0]]);}
    if bpp==16{let i=start+x*2;let v=u16::from_le_bytes(data.get(i..i+2)?.try_into().ok()?);return Some([(((v>>10)&31)*255/31)as u8,(((v>>5)&31)*255/31)as u8,((v&31)*255/31)as u8]);}
    let i=start+x*(bpp/8);let p=data.get(i..i+3)?;Some([p[2],p[1],p[0]])
}
pub fn self_test(c:&mut Controls)->bool{
    c.reset();if !c.add_bar(1)||c.add_bar(1)||c.add_buttons(1,&[BUTTON])||!c.abi(1,32){return false;}
    let Some(l)=c.create_list(24,24)else{return false;};
    if c.set_images(1,l)!=Some(0)||c.add_image(l,Image{offset:2,len:4,mask:0xff00ff},24,24)!=Some(0)||c.count(l)!=Some(1)||!c.add_buttons(1,&[Button{command:42,..BUTTON}])||c.button_count(1)!=Some(1)||c.dimensions(1)!=Some((36,36))||c.destroy_list(l)||c.set_images(1,0)!=Some(l)||!c.destroy_list(l){return false;}
    let mut dib=[0u8;44];dib[0]=40;dib[4]=1;dib[8]=1;dib[12]=1;dib[14]=24;
    let ok=dib_size(&dib)==Some((1,1))&&dib_size(&dib[..43]).is_none();c.reset();ok
}
