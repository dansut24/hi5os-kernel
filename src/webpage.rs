//! Bounded native document/layout model. No arbitrary script evaluation or network side effects.
use crate::ui::{Canvas,Color};
const MAX:usize=128;
const TEXT:usize=192;
#[derive(Clone,Copy)]struct Block{buf:[u8;TEXT],len:usize,kind:u8,color:Option<Color>,bg:Option<Color>,id:[u8;32],id_len:usize}
const EMPTY:Block=Block{buf:[0;TEXT],len:0,kind:0,color:None,bg:None,id:[0;32],id_len:0};
#[derive(Clone,Copy)]pub struct Page{blocks:[Block;MAX],pub count:usize,pub styled:usize,pub images:usize,pub scripts:usize,title:[u8;96],title_len:usize}
pub static mut PAGE:Page=Page::new();
fn trim(b:&[u8])->&[u8]{let mut a=0;let mut z=b.len();while a<z&&b[a].is_ascii_whitespace(){a+=1;}while z>a&&b[z-1].is_ascii_whitespace(){z-=1;}&b[a..z]}
fn attr<'a>(mut a:&'a[u8],want:&[u8])->Option<&'a[u8]>{
    while !a.is_empty(){a=trim(a);if a.is_empty(){break;}let n=a.iter().position(|b|b.is_ascii_whitespace()||*b==b'=').unwrap_or(a.len());if n==0{return None;}let k=&a[..n];a=trim(&a[n..]);if a.first()!=Some(&b'='){continue;}a=trim(&a[1..]);let q=*a.first()?;let(v,rest)=if q==b'\''||q==b'"'{let e=a[1..].iter().position(|b|*b==q)?+1;(&a[1..e],&a[e+1..])}else{let e=a.iter().position(|b|b.is_ascii_whitespace()).unwrap_or(a.len());(&a[..e],&a[e..])};if k.eq_ignore_ascii_case(want){return Some(v);}a=rest;
    }None
}
fn rgb(s:&[u8])->Option<Color>{let s=trim(s);let named=[(b"red".as_slice(),[220,30,30]),(b"blue",[20,90,210]),(b"black",[0,0,0]),(b"white",[255,255,255]),(b"green",[0,128,0]),(b"navy",[0,0,128]),(b"gray",[128,128,128])];for(n,c)in named{if s.eq_ignore_ascii_case(n){return Some(c);}}if s.len()!=7||s[0]!=b'#'{return None;}let mut c=[0;3];for i in 0..3{let h=(s[1+i*2]as char).to_digit(16)?;let l=(s[2+i*2]as char).to_digit(16)?;c[i]=(h*16+l)as u8;}Some(c)}
fn style(b:&mut Block,s:&[u8]){for decl in s.split(|v|*v==b';'){if let Some(i)=decl.iter().position(|v|*v==b':'){let k=trim(&decl[..i]);let v=trim(&decl[i+1..]);if k.eq_ignore_ascii_case(b"color"){b.color=rgb(v);}else if k.eq_ignore_ascii_case(b"background-color"){b.bg=rgb(v);}else if k.eq_ignore_ascii_case(b"font-weight")&&matches!(v,b"bold"|b"700"){b.kind=1;}else if k.eq_ignore_ascii_case(b"font-size")&&matches!(v,b"24px"|b"28px"|b"32px"){b.kind=1;}}}}
impl Page{
    pub const fn new()->Self{Self{blocks:[EMPTY;MAX],count:0,styled:0,images:0,scripts:0,title:[0;96],title_len:0}}
    pub fn clear(&mut self){self.count=0;self.styled=0;self.images=0;self.scripts=0;self.title_len=0;}
    pub fn title(&self)->&str{core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")}
    fn push(&mut self,b:&mut Block){if b.len>0&&self.count<MAX{if b.color.is_some()||b.bg.is_some(){self.styled+=1;}self.blocks[self.count]=*b;self.count+=1;}*b=EMPTY;}
    fn text(&mut self,b:&mut Block,t:&[u8]){
        let mut i=0;while i<t.len()&&self.count<MAX{let mut ch=t[i];let mut step=1;if ch==b'&'{for(s,v)in[(b"&amp;".as_slice(),b'&'),(b"&lt;",b'<'),(b"&gt;",b'>'),(b"&quot;",b'"'),(b"&#39;",b'\''),(b"&nbsp;",b' ')]{if t[i..].starts_with(s){ch=v;step=s.len();break;}}}
            if ch.is_ascii_whitespace(){ch=b' ';if b.len==0||b.buf[b.len-1]==b' '{i+=step;continue;}}
            if ch>=128{if ch&0xc0==0x80{i+=step;continue;}ch=b'?';}
            if ch>=32&&ch<127{if b.len==TEXT{let mut next=*b;next.len=0;next.id_len=0;self.push(b);*b=next;}b.buf[b.len]=ch;b.len+=1;}i+=step;
        }
    }
    pub fn parse(&mut self,src:&[u8]){
        self.clear();let mut p=0;let mut b=EMPTY;let mut skip=0u8;
        while p<src.len()&&self.count<MAX{
            if src[p]!=b'<'{let e=src[p..].iter().position(|v|*v==b'<').unwrap_or(src.len()-p);if skip==0{self.text(&mut b,&src[p..p+e]);}else if skip==4{let t=trim(&src[p..p+e]);let n=t.len().min(96);self.title[..n].copy_from_slice(&t[..n]);self.title_len=n;}p+=e;continue;}
            if src[p..].starts_with(b"<!--"){if let Some(e)=src[p+4..].windows(3).position(|v|v==b"-->"){p+=e+7;continue;}break;}
            let Some(e)=src[p..].iter().position(|v|*v==b'>')else{break;};let raw=trim(&src[p+1..p+e]);p+=e+1;
            let closing=raw.first()==Some(&b'/');let t=if closing{trim(&raw[1..])}else{raw};let n=t.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(t.len());let tag=&t[..n];let a=&t[n..];
            let group=if tag.eq_ignore_ascii_case(b"script"){1}else if tag.eq_ignore_ascii_case(b"style"){2}else if tag.eq_ignore_ascii_case(b"head"){3}else if tag.eq_ignore_ascii_case(b"title"){4}else{0};
            if skip==3&&group==4&&!closing{skip=4;continue;}if skip==4&&group==4&&closing{skip=3;continue;}if skip!=0{if group==skip&&closing{skip=0;}continue;}if group!=0&&!closing{if group==1{self.scripts+=1;}skip=group;continue;}
            let heading=[b"h1".as_slice(),b"h2",b"h3",b"h4"].iter().any(|v|tag.eq_ignore_ascii_case(v));
            let block=heading||[b"p".as_slice(),b"div",b"section",b"article",b"header",b"footer",b"nav",b"main",b"li",b"br",b"tr",b"pre",b"button"].iter().any(|v|tag.eq_ignore_ascii_case(v));
            if block{self.push(&mut b);if !closing{if heading{b.kind=1;}if tag.eq_ignore_ascii_case(b"li"){self.text(&mut b,b"- ");}}}
            if !closing&&tag.eq_ignore_ascii_case(b"img"){self.push(&mut b);self.images+=1;b.kind=2;self.text(&mut b,b"[Image] ");if let Some(v)=attr(a,b"alt"){self.text(&mut b,v);}self.push(&mut b);}
            if !closing&&(block||tag.eq_ignore_ascii_case(b"span")||tag.eq_ignore_ascii_case(b"strong")){if let Some(v)=attr(a,b"style"){style(&mut b,v);}if let Some(v)=attr(a,b"id"){let n=v.len().min(32);b.id[..n].copy_from_slice(&v[..n]);b.id_len=n;}}
        }
        self.push(&mut b);
    }
    // Deliberately tiny, opt-in textContent assignment subset. No eval, loops,
    // functions, networking, native calls, HTML injection or external scripts.
    pub fn literal_update(&mut self,src:&[u8])->usize{
        let mut p=0;let mut changed=0;
        while p<src.len()&&changed<16{
            let Some(i)=src[p..].windows(8).position(|v|v.eq_ignore_ascii_case(b"<script>"))else{break;};p+=i+8;
            let Some(e)=src[p..].windows(9).position(|v|v.eq_ignore_ascii_case(b"</script>"))else{break;};
            let s=trim(&src[p..p+e]);p+=e+9;
            let Some(s)=s.strip_prefix(b"document.getElementById(")else{continue;};
            let Some((id,rest))=literal(s)else{continue;};let Some(rest)=trim(rest).strip_prefix(b").textContent")else{continue;};
            let Some(rest)=trim(rest).strip_prefix(b"=")else{continue;};let Some((value,rest))=literal(trim(rest))else{continue;};if !matches!(trim(rest),b""|b";")||value.len()>TEXT||id.len()>32{continue;}
            if let Some(b)=self.blocks[..self.count].iter_mut().find(|b|b.id_len==id.len()&&&b.id[..b.id_len]==id){b.buf[..value.len()].copy_from_slice(value);b.len=value.len();changed+=1;}
        }changed
    }
    pub fn draw(&self,c:&Canvas,x:usize,mut y:usize,right:usize,floor:usize,scroll:usize,ink:Color,sub:Color){
        let width=right.saturating_sub(x);if width<40{return;}let mut row=0;
        for b in &self.blocks[..self.count]{let big=b.kind==1;let height=if big{30}else{22};let cols=(width/if big{14}else{9}).clamp(8,TEXT);
            for part in b.buf[..b.len].chunks(cols){if row>=scroll{if y+height>floor{return;}if let Some(bg)=b.bg{c.rect(x,y,width,height,bg);}else if b.kind==2{c.rect(x,y,width,height,sub);}
                if let Ok(t)=core::str::from_utf8(part){c.text_limit(x+3,y+2,t,b.color.unwrap_or(ink),big,right);}y+=height;}row+=1;}
        }
    }
}
fn literal(s:&[u8])->Option<(&[u8],&[u8])>{let q=*s.first()?;if q!=b'\''&&q!=b'"'{return None;}let e=s[1..].iter().position(|v|*v==q)?+1;let a=&s[1..e];if a.iter().any(|v|*v<32||*v>=127||*v==b'\\'){return None;}Some((a,&s[e+1..]))}
// Uncompressed 24-bit BMP only; bounded dimensions and checked offsets.
pub struct Bitmap<'a>{bytes:&'a[u8],offset:usize,stride:usize,pub w:usize,pub h:usize,bottom:bool}
fn u32at(b:&[u8],n:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(n..n+4)?.try_into().ok()?))}
impl<'a> Bitmap<'a>{pub fn parse(b:&'a[u8])->Option<Self>{if b.get(..2)?!=b"BM"||u32at(b,14)?<40||b.get(26..30)?!=[1,0,24,0]||u32at(b,30)?!=0{return None;}let w=u32at(b,18)?as i32;let h=u32at(b,22)?as i32;if !(1..=2048).contains(&w)||h==0||h==i32::MIN||h.abs()>2048{return None;}let(w,hgt)=(w as usize,h.unsigned_abs()as usize);let stride=(w*3+3)&!3;let offset=u32at(b,10)?as usize;if offset<54||offset.checked_add(stride*hgt)?>b.len(){return None;}Some(Self{bytes:b,offset,stride,w,h:hgt,bottom:h>0})}
    pub fn draw(&self,c:&Canvas,x:usize,y:usize,maxw:usize,maxh:usize){if maxw==0||maxh==0{return;}let w=self.w.min(maxw).min(maxh*self.w/self.h);if w==0{return;}let h=(w*self.h/self.w).max(1);for j in 0..h{let sy=j*self.h/h;let sy=if self.bottom{self.h-1-sy}else{sy};for i in 0..w{let at=self.offset+sy*self.stride+(i*self.w/w)*3;c.pixel(x+i,y+j,[self.bytes[at+2],self.bytes[at+1],self.bytes[at]]);}}}
}
pub fn self_test()->bool{
    let p=unsafe{&mut *core::ptr::addr_of_mut!(PAGE)};
    let s=b"<html><head><title>Hi5 test</title></head><body><h1 style='color:#112233'>Headline</h1><p id='demo'>Before</p><img alt='photo'><script>document.getElementById('demo').textContent = 'After';</script></body></html>";
    p.parse(s);let ok=p.count==3&&p.title()=="Hi5 test"&&p.styled==1&&p.images==1&&p.scripts==1&&p.blocks[0].kind==1&&p.literal_update(s)==1&&&p.blocks[1].buf[..p.blocks[1].len]==b"After"&&p.literal_update(b"<script>document.getElementById('demo').textContent = 'no';alert(1)</script>")==0;
    p.clear();ok&&Bitmap::parse(b"BM").is_none()
}
