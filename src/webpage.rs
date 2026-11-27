//! Bounded native document/layout model. No arbitrary script evaluation or network side effects.
use crate::ui::{Canvas,Color};
use crate::http::{Link,EMPTY_LINK};
const MAX:usize=2048;
const IMAGE_LIMIT:usize=4;
const CSS_LIMIT:usize=2;
const PIX_W:usize=320;const PIX_H:usize=180;const PIX_CAP:usize=PIX_W*PIX_H*3;
#[derive(Clone,Copy)]struct Photo{w:usize,h:usize,bytes:[u8;PIX_CAP]}
const NO_PHOTO:Photo=Photo{w:0,h:0,bytes:[0;PIX_CAP]};
static mut PHOTOS:[Photo;IMAGE_LIMIT]=[NO_PHOTO;IMAGE_LIMIT];
#[link(name="hi5image",kind="static")]extern "C"{fn hi5_image_decode(data:*const u8,len:usize,out:*mut u8,cap:usize,w:*mut u32,h:*mut u32)->i32;}
#[derive(Clone,Copy)]struct Rule{selector:[u8;96],len:usize,style:Block,spec:u8}
const EMPTY_RULE:Rule=Rule{selector:[0;96],len:0,style:EMPTY,spec:0};
static mut RULES:[Rule;256]=[EMPTY_RULE;256];
const TEXT:usize=192;
#[derive(Clone,Copy)]struct Block{buf:[u8;TEXT],len:usize,kind:u8,color:Option<Color>,bg:Option<Color>,id:[u8;32],id_len:usize,link:Link,image:u8}
const EMPTY:Block=Block{buf:[0;TEXT],len:0,kind:0,color:None,bg:None,id:[0;32],id_len:0,link:EMPTY_LINK,image:0};
#[derive(Clone,Copy)]pub struct Page{blocks:[Block;MAX],pub count:usize,pub styled:usize,pub images:usize,pub scripts:usize,pub stylesheets:[Link;CSS_LIMIT],pub stylesheet_count:usize,pub image_urls:[Link;IMAGE_LIMIT],pub image_count:usize,pub loaded_images:usize,title:[u8;96],title_len:usize}
pub static mut PAGE:Page=Page::new();
fn trim(b:&[u8])->&[u8]{let mut a=0;let mut z=b.len();while a<z&&b[a].is_ascii_whitespace(){a+=1;}while z>a&&b[z-1].is_ascii_whitespace(){z-=1;}&b[a..z]}
fn attr<'a>(mut a:&'a[u8],want:&[u8])->Option<&'a[u8]>{
    while !a.is_empty(){a=trim(a);if a.is_empty(){break;}let n=a.iter().position(|b|b.is_ascii_whitespace()||*b==b'=').unwrap_or(a.len());if n==0{return None;}let k=&a[..n];a=trim(&a[n..]);if a.first()!=Some(&b'='){continue;}a=trim(&a[1..]);let q=*a.first()?;let(v,rest)=if q==b'\''||q==b'"'{let e=a[1..].iter().position(|b|*b==q)?+1;(&a[1..e],&a[e+1..])}else{let e=a.iter().position(|b|b.is_ascii_whitespace()).unwrap_or(a.len());(&a[..e],&a[e..])};if k.eq_ignore_ascii_case(want){return Some(v);}a=rest;
    }None
}
fn rgb(s:&[u8])->Option<Color>{let s=trim(s);let named=[(b"red".as_slice(),[220,30,30]),(b"blue",[20,90,210]),(b"black",[0,0,0]),(b"white",[255,255,255]),(b"green",[0,128,0]),(b"navy",[0,0,128]),(b"gray",[128,128,128])];for(n,c)in named{if s.eq_ignore_ascii_case(n){return Some(c);}}if s.len()!=7||s[0]!=b'#'{return None;}let mut c=[0;3];for i in 0..3{let h=(s[1+i*2]as char).to_digit(16)?;let l=(s[2+i*2]as char).to_digit(16)?;c[i]=(h*16+l)as u8;}Some(c)}
fn style(b:&mut Block,s:&[u8]){for decl in s.split(|v|*v==b';'){if let Some(i)=decl.iter().position(|v|*v==b':'){let k=trim(&decl[..i]);let v=trim(&decl[i+1..]);if k.eq_ignore_ascii_case(b"color"){if let Some(c)=rgb(v){b.color=Some(c);}}else if k.eq_ignore_ascii_case(b"background-color"){if let Some(c)=rgb(v){b.bg=Some(c);}}else if k.eq_ignore_ascii_case(b"font-weight")&&matches!(v,b"bold"|b"700"){b.kind=1;}else if k.eq_ignore_ascii_case(b"font-size")&&matches!(v,b"24px"|b"28px"|b"32px"){b.kind=1;}}}}
impl Page{
    pub const fn new()->Self{Self{blocks:[EMPTY;MAX],count:0,styled:0,images:0,scripts:0,stylesheets:[EMPTY_LINK;CSS_LIMIT],stylesheet_count:0,image_urls:[EMPTY_LINK;IMAGE_LIMIT],image_count:0,loaded_images:0,title:[0;96],title_len:0}}
    pub fn clear(&mut self){self.count=0;self.styled=0;self.images=0;self.scripts=0;self.title_len=0;self.stylesheet_count=0;self.image_count=0;self.loaded_images=0;unsafe{for p in (&mut *core::ptr::addr_of_mut!(PHOTOS)).iter_mut(){p.w=0;p.h=0;}}}
    pub fn title(&self)->&str{core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")}
    fn push(&mut self,b:&mut Block){if b.len>0&&self.count<MAX{if b.color.is_some()||b.bg.is_some(){self.styled+=1;}self.blocks[self.count]=*b;self.count+=1;}*b=EMPTY;}
    fn text(&mut self,b:&mut Block,t:&[u8]){
        let mut i=0;while i<t.len()&&self.count<MAX{let mut ch=t[i];let mut step=1;if ch==b'&'{for(s,v)in[(b"&amp;".as_slice(),b'&'),(b"&lt;",b'<'),(b"&gt;",b'>'),(b"&quot;",b'"'),(b"&#39;",b'\''),(b"&nbsp;",b' ')]{if t[i..].starts_with(s){ch=v;step=s.len();break;}}}
            if ch.is_ascii_whitespace(){ch=b' ';if b.len==0||b.buf[b.len-1]==b' '{i+=step;continue;}}
            if ch>=128{if ch&0xc0==0x80{i+=step;continue;}ch=b'?';}
            if ch>=32&&ch<127{if b.len==TEXT{let mut next=*b;next.len=0;next.id_len=0;self.push(b);*b=next;}b.buf[b.len]=ch;b.len+=1;}i+=step;
        }
    }
    pub fn parse(&mut self,src:&[u8]){self.parse_at(src,b"http://example.test/",&[]);}
    pub fn parse_at(&mut self,src:&[u8],base:&[u8],css:&[u8]){
        self.clear();let rules=unsafe{&mut *core::ptr::addr_of_mut!(RULES)};let mut nr=0;css_rules(css,rules,&mut nr);
        // Collect embedded stylesheet blocks and external stylesheet references.
        let mut scan=0;while scan<src.len(){
            let Some(e)=src[scan..].iter().position(|v|*v==b'<')else{break;};scan+=e;
            let Some(e)=tag_end(&src[scan..])else{break;};let raw=trim(&src[scan+1..scan+e]);scan+=e+1;
            let n=raw.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(raw.len());let t=&raw[..n];let a=&raw[n..];
            if raw.starts_with(b"!--"){if let Some(e)=src[scan..].windows(3).position(|v|v==b"-->"){scan+=e+3;}continue;}
            if t.eq_ignore_ascii_case(b"script")||t.eq_ignore_ascii_case(b"style"){
                let stop=if t.eq_ignore_ascii_case(b"style"){b"</style".as_slice()}else{b"</script".as_slice()};
                let Some(end)=src[scan..].windows(stop.len()).position(|v|v.eq_ignore_ascii_case(stop))else{break;};
                if t.eq_ignore_ascii_case(b"style"){css_rules(&src[scan..scan+end],rules,&mut nr);}scan+=end;continue;
            }
            if t.eq_ignore_ascii_case(b"link")&&self.stylesheet_count<CSS_LIMIT&&attr(a,b"rel").is_some_and(|v|v.eq_ignore_ascii_case(b"stylesheet")){
                if let Some(h)=attr(a,b"href"){if let Some(l)=url(base,h){self.stylesheets[self.stylesheet_count]=l;self.stylesheet_count+=1;}}
            }
        }
        let mut p=0;let mut b=EMPTY;let mut skip=0u8;let mut active=EMPTY_LINK;
        while p<src.len()&&self.count<MAX{
            if src[p]!=b'<'{let e=src[p..].iter().position(|v|*v==b'<').unwrap_or(src.len()-p);if skip==0{b.link=active;self.text(&mut b,&src[p..p+e]);}else if skip==4{let t=trim(&src[p..p+e]);let n=t.len().min(96);self.title[..n].copy_from_slice(&t[..n]);self.title_len=n;}p+=e;continue;}
            if src[p..].starts_with(b"<!--"){if let Some(e)=src[p+4..].windows(3).position(|v|v==b"-->"){p+=e+7;continue;}break;}
            let Some(e)=tag_end(&src[p..])else{break;};let raw=trim(&src[p+1..p+e]);p+=e+1;
            let closing=raw.first()==Some(&b'/');let t=if closing{trim(&raw[1..])}else{raw};let n=t.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(t.len());let tag=&t[..n];let a=&t[n..];
            let group=if tag.eq_ignore_ascii_case(b"script"){1}else if tag.eq_ignore_ascii_case(b"style"){2}else if tag.eq_ignore_ascii_case(b"head"){3}else if tag.eq_ignore_ascii_case(b"title"){4}else{0};
            if matches!(group,1|2)&&!closing{if group==1&&skip==0{self.scripts+=1;}let stop=if group==1{b"</script".as_slice()}else{b"</style".as_slice()};if let Some(e)=src[p..].windows(stop.len()).position(|v|v.eq_ignore_ascii_case(stop)){let pos=p+e;if let Some(end)=tag_end(&src[pos..]){p=pos+end+1;continue;}}break;}
            if skip==3&&group==4&&!closing{skip=4;continue;}if skip==4&&group==4&&closing{skip=3;continue;}if skip!=0{if group==skip&&closing{skip=0;}continue;}if group!=0&&!closing{if group==1{self.scripts+=1;}skip=group;continue;}
            if tag.eq_ignore_ascii_case(b"a"){self.push(&mut b);active=if closing{EMPTY_LINK}else{attr(a,b"href").and_then(|v|url(base,v)).unwrap_or(EMPTY_LINK)};b.link=active;}
            let heading=[b"h1".as_slice(),b"h2",b"h3",b"h4"].iter().any(|v|tag.eq_ignore_ascii_case(v));
            let block=heading||[b"p".as_slice(),b"div",b"section",b"article",b"header",b"footer",b"nav",b"main",b"li",b"br",b"tr",b"pre",b"button"].iter().any(|v|tag.eq_ignore_ascii_case(v));
            if block{self.push(&mut b);b.link=active;if !closing{if heading{b.kind=1;}if tag.eq_ignore_ascii_case(b"li"){self.text(&mut b,b"- ");}}}
            if !closing&&tag.eq_ignore_ascii_case(b"img"){self.push(&mut b);self.images+=1;b.kind=2;b.link=active;if self.image_count<IMAGE_LIMIT{if let Some(l)=attr(a,b"src").or_else(||attr(a,b"data-src")).and_then(|v|url(base,v)){self.image_urls[self.image_count]=l;self.image_count+=1;b.image=self.image_count as u8;}}self.text(&mut b,b"[Image] ");if let Some(v)=attr(a,b"alt"){self.text(&mut b,v);}self.push(&mut b);}
            if !closing&&(block||tag.eq_ignore_ascii_case(b"span")||tag.eq_ignore_ascii_case(b"strong")||tag.eq_ignore_ascii_case(b"a")){for spec in 0..3{for r in &rules[..nr]{if r.spec==spec&&selector_match(&r.selector[..r.len],tag,a){if r.style.color.is_some(){b.color=r.style.color;}if r.style.bg.is_some(){b.bg=r.style.bg;}if r.style.kind==1{b.kind=1;}}}}if let Some(v)=attr(a,b"style"){style(&mut b,v);}if let Some(v)=attr(a,b"id"){let n=v.len().min(32);b.id[..n].copy_from_slice(&v[..n]);b.id_len=n;}}
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
    pub fn load_image(&mut self,i:usize,bytes:&[u8])->bool{
        if i>=self.image_count{return false;}let photo=unsafe{&mut (*core::ptr::addr_of_mut!(PHOTOS))[i]};let mut w=0u32;let mut h=0u32;
        if unsafe{hi5_image_decode(bytes.as_ptr(),bytes.len(),photo.bytes.as_mut_ptr(),PIX_CAP,&mut w,&mut h)}!=1||w==0||h==0||w as usize>PIX_W||h as usize>PIX_H{return false;}
        if photo.w==0{self.loaded_images+=1;}photo.w=w as usize;photo.h=h as usize;true
    }
    fn cols(b:&Block,width:usize)->usize{(width/if b.kind==1{14}else{9}).clamp(8,TEXT)}
    fn photo(b:&Block)->Option<&'static Photo>{if b.image==0{return None;}let p=unsafe{&(*core::ptr::addr_of!(PHOTOS))[b.image as usize-1]};(p.w>0&&p.h>0).then_some(p)}
    fn image_size(p:&Photo,width:usize)->(usize,usize){let w=p.w.min(width.saturating_sub(8));(w,(w*p.h/p.w).max(1))}
    pub fn rows(&self,width:usize)->usize{self.blocks[..self.count].iter().map(|b|if Self::photo(b).is_some(){1}else{b.len.div_ceil(Self::cols(b,width))}).sum()}
    pub fn draw(&self,c:&Canvas,x:usize,mut y:usize,right:usize,floor:usize,scroll:usize,ink:Color,sub:Color){
        let width=right.saturating_sub(x);if width<40{return;}let mut row=0;
        for b in &self.blocks[..self.count]{
            if let Some(p)=Self::photo(b){if row>=scroll{let(w,h)=Self::image_size(p,width);let h=h.min(floor.saturating_sub(y));for j in 0..h{for i in 0..w{let at=((j*p.h/(w*p.h/p.w).max(1))*p.w+i*p.w/w)*3;c.pixel(x+3+i,y+j,[p.bytes[at],p.bytes[at+1],p.bytes[at+2]]);}}y+=h+8;if y>=floor{return;}}row+=1;continue;}
            let big=b.kind==1;let height=if big{30}else{22};
            for part in b.buf[..b.len].chunks(Self::cols(b,width)){if row>=scroll{if y+height>floor{return;}if let Some(bg)=b.bg{c.rect(x,y,width,height,bg);}else if b.kind==2{c.rect(x,y,width,height,sub);}
                if let Ok(t)=core::str::from_utf8(part){let color=b.color.unwrap_or(if b.link.len>0{[31,97,185]}else{ink});c.text_limit(x+3,y+2,t,color,big,right);if b.link.len>0{c.rect(x+3,y+height-3,c.text_width(t).min(width.saturating_sub(6)),1,color);}}y+=height;}row+=1;}
        }
    }
    pub fn hit(&self,width:usize,mut y:usize,floor:usize,scroll:usize,click:usize)->Option<Link>{let mut row=0;for b in &self.blocks[..self.count]{if let Some(p)=Self::photo(b){if row>=scroll{let(_,h)=Self::image_size(p,width);let next=y+h.min(floor.saturating_sub(y))+8;if click>=y&&click<next.min(floor){return (b.link.len>0).then_some(b.link);}y=next;}row+=1;continue;}let h=if b.kind==1{30}else{22};for _ in b.buf[..b.len].chunks(Self::cols(b,width)){if row>=scroll{if y+h>floor{return None;}if click>=y&&click<y+h{return (b.link.len>0).then_some(b.link);}y+=h;}row+=1;}}None}

}
fn url(base:&[u8],href:&[u8])->Option<Link>{let mut l=EMPTY_LINK;l.len=crate::http::resolve_url(base,href,&mut l.url)?;if base.starts_with(b"https://")&&!l.url[..l.len].starts_with(b"https://"){return None;}Some(l)}
fn tag_end(s:&[u8])->Option<usize>{let mut q=0;for(i,b)in s.iter().enumerate().skip(1){if q!=0{if *b==q{q=0;}}else if *b==b'\''||*b==b'"'{q=*b;}else if *b==b'>'{return Some(i);}}None}
fn selector_match(s:&[u8],tag:&[u8],attrs:&[u8])->bool{if s.first()==Some(&b'.'){attr(attrs,b"class").is_some_and(|v|v.split(|b|b.is_ascii_whitespace()).any(|v|v==&s[1..]))}else if s.first()==Some(&b'#'){attr(attrs,b"id")==Some(&s[1..])}else{s.eq_ignore_ascii_case(tag)}}
fn css_rules(mut css:&[u8],out:&mut[Rule;256],count:&mut usize){while !css.is_empty()&&*count<out.len(){let Some(i)=css.iter().position(|v|*v==b'{')else{break;};let Some(j)=css[i+1..].iter().position(|v|*v==b'}')else{break;};let sel=&css[..i];let body=&css[i+1..i+1+j];css=&css[i+j+2..];let mut b=EMPTY;style(&mut b,body);if b.color.is_none()&&b.bg.is_none()&&b.kind==0{continue;}for s in sel.split(|v|*v==b','){let s=trim(s);if s.is_empty()||s.len()>96||*count>=out.len(){continue;}let spec=if s[0]==b'#'{2}else if s[0]==b'.'{1}else{0};let name=if spec>0{&s[1..]}else{s};if name.is_empty()||!name.iter().all(|v|v.is_ascii_alphanumeric()||*v==b'-'||*v==b'_'){continue;}let mut r=EMPTY_RULE;r.selector[..s.len()].copy_from_slice(s);r.len=s.len();r.spec=spec;r.style=b;out[*count]=r;*count+=1;}}}
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
    p.parse_at(b"<h1 class='headline'>A</h1><p><a href='/news'>News</a></p>",b"https://example.test/",b".headline { color: #123456; }");let css_ok=p.styled==1&&p.blocks[0].color==Some([18,52,86])&&p.blocks[1].link.len>0&&p.hit(400,0,200,0,31).is_some_and(|l|&l.url[..l.len]==b"https://example.test/news");p.clear();ok&&css_ok&&Bitmap::parse(b"BM").is_none()
}
