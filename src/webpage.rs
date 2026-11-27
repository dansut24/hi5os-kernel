//! Bounded native document/layout model. No arbitrary script evaluation or network side effects.
use crate::ui::{Canvas,Color};
use crate::http::{Link,EMPTY_LINK};
use crate::hi5webcore::{self as coreweb,Form};
const MAX:usize=4096;
const IMAGE_LIMIT:usize=16;
const CSS_LIMIT:usize=8;
const PIX_W:usize=320;const PIX_H:usize=180;const PIX_CAP:usize=PIX_W*PIX_H*3;
#[derive(Clone,Copy)]struct Photo{w:usize,h:usize,bytes:[u8;PIX_CAP]}
const NO_PHOTO:Photo=Photo{w:0,h:0,bytes:[0;PIX_CAP]};
static mut PHOTOS:[Photo;IMAGE_LIMIT]=[NO_PHOTO;IMAGE_LIMIT];
#[link(name="hi5image",kind="static")]extern "C"{fn hi5_image_decode(data:*const u8,len:usize,out:*mut u8,cap:usize,w:*mut u32,h:*mut u32)->i32;}
#[derive(Clone,Copy)]struct Rule{selector:[u8;96],len:usize,style:Block,spec:u16}
const EMPTY_RULE:Rule=Rule{selector:[0;96],len:0,style:EMPTY,spec:0};
const RULE_CAP:usize=1536;
static mut RULES:[Rule;RULE_CAP]=[EMPTY_RULE;RULE_CAP];
const TEXT:usize=192;
#[derive(Clone,Copy)]struct Block{buf:[u8;TEXT],len:usize,kind:u8,color:Option<Color>,bg:Option<Color>,id:[u8;32],id_len:usize,link:Link,image:u8,align:u8,padding:u8,margin:u8,hidden:bool,display:Option<bool>,align_set:bool,layout:u8,columns:u8,gap:u8,group:u16,item:u16,group_cols:u8,group_gap:u8,flow:u16,node:u16,width_px:u16,width_pct:u8,max_px:u16,min_h:u16,auto_margin:bool,list_none:bool,abs_pos:bool,clip_hidden:bool}
const EMPTY:Block=Block{buf:[0;TEXT],len:0,kind:0,color:None,bg:None,id:[0;32],id_len:0,link:EMPTY_LINK,image:0,align:0,padding:0,margin:0,hidden:false,display:None,align_set:false,layout:0,columns:0,gap:0,group:0,item:0,group_cols:0,group_gap:0,flow:0,node:0,width_px:0,width_pct:0,max_px:0,min_h:0,auto_margin:false,list_none:false,abs_pos:false,clip_hidden:false};
#[derive(Clone,Copy)]pub struct Page{blocks:[Block;MAX],pub count:usize,pub styled:usize,pub images:usize,pub scripts:usize,pub stylesheets:[Link;CSS_LIMIT],pub stylesheet_count:usize,pub image_urls:[Link;IMAGE_LIMIT],pub image_count:usize,pub loaded_images:usize,pub dom_nodes:usize,pub requires_js:bool,pub empty_document:bool,pub truncated:bool,pub form:Form,pub form_focus:bool,title:[u8;96],title_len:usize}
pub static mut PAGE:Page=Page::new();
static mut TAB_PAGES:[Page;4]=[Page::new();4];
static mut TAB_PHOTOS:[[Photo;IMAGE_LIMIT];4]=[[NO_PHOTO;IMAGE_LIMIT];4];
fn trim(b:&[u8])->&[u8]{let mut a=0;let mut z=b.len();while a<z&&b[a].is_ascii_whitespace(){a+=1;}while z>a&&b[z-1].is_ascii_whitespace(){z-=1;}&b[a..z]}
fn attr<'a>(mut a:&'a[u8],want:&[u8])->Option<&'a[u8]>{
    while !a.is_empty(){a=trim(a);if a.is_empty(){break;}let n=a.iter().position(|b|b.is_ascii_whitespace()||*b==b'=').unwrap_or(a.len());if n==0{return None;}let k=&a[..n];a=trim(&a[n..]);if a.first()!=Some(&b'='){if k.eq_ignore_ascii_case(want){return Some(&[]);}continue;}a=trim(&a[1..]);let q=*a.first()?;let(v,rest)=if q==b'\''||q==b'"'{let e=a[1..].iter().position(|b|*b==q)?+1;(&a[1..e],&a[e+1..])}else{let e=a.iter().position(|b|b.is_ascii_whitespace()).unwrap_or(a.len());(&a[..e],&a[e..])};if k.eq_ignore_ascii_case(want){return Some(v);}a=rest;
    }None
}
fn rgb(s:&[u8])->Option<Color>{let s=trim(s);if let Some(c)=rgb_function(s){return Some(c);}let named=[(b"red".as_slice(),[220,30,30]),(b"blue",[20,90,210]),(b"black",[0,0,0]),(b"white",[255,255,255]),(b"green",[0,128,0]),(b"navy",[0,0,128]),(b"gray",[128,128,128])];for(n,c)in named{if s.eq_ignore_ascii_case(n){return Some(c);}}if s.len()==4&&s[0]==b'#'{let mut c=[0;3];for i in 0..3{c[i]=((s[i+1]as char).to_digit(16)?*17)as u8;}return Some(c);}if s.len()!=7||s[0]!=b'#'{return None;}let mut c=[0;3];for i in 0..3{let h=(s[1+i*2]as char).to_digit(16)?;let l=(s[2+i*2]as char).to_digit(16)?;c[i]=(h*16+l)as u8;}Some(c)}
fn rgb_function(s:&[u8])->Option<Color>{
    let inner=s.strip_prefix(b"rgb(").or_else(||s.strip_prefix(b"rgba("))?.strip_suffix(b")")?;
    let mut parts=inner.split(|v|v.is_ascii_whitespace()||*v==b','||*v==b'/').filter(|v|!v.is_empty());let mut c=[0u8;3];
    for v in &mut c{let p=parts.next()?;let percent=p.ends_with(b"%");let p=p.strip_suffix(b"%").unwrap_or(p);if p.is_empty()||p.len()>3||!p.iter().all(u8::is_ascii_digit){return None;}let mut n=0u16;for b in p{n=n*10+(b-48)as u16;}*v=if percent{(n.min(100)*255/100)as u8}else{n.min(255)as u8};}
    // Only opaque colors are returned; alpha compositing is not implemented.
    if let Some(a)=parts.next(){if !matches!(a,b"1"|b"1.0"|b"100%"){return None;}}if parts.next().is_some(){return None;}Some(c)
}
fn background(s:&[u8])->Option<Color>{if let Some(c)=rgb(s){return Some(c);}let s=trim(s);if s.starts_with(b"linear-gradient(")||s.starts_with(b"radial-gradient("){for token in s.split(|v|v.is_ascii_whitespace()||matches!(*v,b','|b'(' |b')')){if token.starts_with(b"#"){if let Some(c)=rgb(token){return Some(c);}}}}None}
fn px(s:&[u8])->Option<u8>{let s=s.strip_suffix(b"px").unwrap_or(s);if s.is_empty()||s.len()>3||!s.iter().all(u8::is_ascii_digit){return None;}let mut n=0u16;for v in s{n=n*10+(v-48)as u16;}Some(n.min(40)as u8)}
fn dim(s:&[u8])->Option<u16>{let s=s.strip_suffix(b"px").unwrap_or(s);if s.is_empty()||s.len()>4||!s.iter().all(u8::is_ascii_digit){return None;}let mut n=0u16;for v in s{n=n*10+(v-48)as u16;}Some(n.min(2048))}
fn style(b:&mut Block,s:&[u8]){for decl in s.split(|v|*v==b';'){if let Some(i)=decl.iter().position(|v|*v==b':'){let k=trim(&decl[..i]);let v=trim(&decl[i+1..]);let v=trim(v.strip_suffix(b"!important").unwrap_or(v));if k.eq_ignore_ascii_case(b"color"){if let Some(c)=rgb(v){b.color=Some(c);}}else if k.eq_ignore_ascii_case(b"background-color")||k.eq_ignore_ascii_case(b"background"){if let Some(c)=background(v){b.bg=Some(c);}}else if k.eq_ignore_ascii_case(b"font-weight")&&matches!(v,b"bold"|b"700"){b.kind=1;}else if k.eq_ignore_ascii_case(b"font-size")&&matches!(v,b"24px"|b"28px"|b"32px"){b.kind=1;}else if k.eq_ignore_ascii_case(b"text-align"){b.align_set=true;b.align=if v.eq_ignore_ascii_case(b"center"){1}else if v.eq_ignore_ascii_case(b"right"){2}else{0};}else if k.eq_ignore_ascii_case(b"position"){b.abs_pos=matches!(v,b"absolute"|b"fixed");}else if k.eq_ignore_ascii_case(b"clip")||k.eq_ignore_ascii_case(b"clip-path"){if v.starts_with(b"rect(0")||v.starts_with(b"inset(50%"){b.clip_hidden=true;}}else if k.eq_ignore_ascii_case(b"width"){if let Some(n)=v.strip_suffix(b"%").and_then(dim){b.width_pct=n.min(100)as u8;b.width_px=0;}else if let Some(n)=dim(v){b.width_px=n;b.width_pct=0;}}else if k.eq_ignore_ascii_case(b"max-width"){if let Some(n)=dim(v){b.max_px=n;}}else if k.eq_ignore_ascii_case(b"height")||k.eq_ignore_ascii_case(b"min-height"){if let Some(n)=dim(v){b.min_h=n.min(600);}}else if k.eq_ignore_ascii_case(b"list-style")||k.eq_ignore_ascii_case(b"list-style-type"){if v==b"none"{b.list_none=true;}}else if k.eq_ignore_ascii_case(b"padding"){if let Some(n)=px(v){b.padding=n;}}else if k.eq_ignore_ascii_case(b"margin"){if let Some(n)=px(v){b.margin=n;}else if v==b"auto"||v.ends_with(b" auto"){b.auto_margin=true;}}else if k.eq_ignore_ascii_case(b"gap"){if let Some(n)=px(v){b.gap=n.min(24);}}else if k.eq_ignore_ascii_case(b"grid-template-columns"){let count=v.split(|b|b.is_ascii_whitespace()).filter(|x|!x.is_empty()).count();if (2..=4).contains(&count)&&v.split(|b|b.is_ascii_whitespace()).filter(|x|!x.is_empty()).all(|x|x==b"1fr"){b.columns=count as u8;}else if let Some(inner)=v.strip_prefix(b"repeat(").and_then(|x|x.strip_suffix(b")")){if let Some(i)=inner.iter().position(|x|*x==b','){if trim(&inner[i+1..])==b"1fr"{if let Some(n)=dim(trim(&inner[..i])){if (2..=4).contains(&n){b.columns=n as u8;}}}}}}else if k.eq_ignore_ascii_case(b"flex-direction")&&v.eq_ignore_ascii_case(b"column"){b.layout=3;}else if k.eq_ignore_ascii_case(b"display"){if v.eq_ignore_ascii_case(b"none"){b.display=Some(false);}else if [b"block".as_slice(),b"inline",b"inline-block",b"flex",b"grid",b"table",b"list-item",b"contents"].iter().any(|x|v.eq_ignore_ascii_case(x)){b.display=Some(true);if v.eq_ignore_ascii_case(b"flex"){if b.layout!=3{b.layout=1;}}else if v.eq_ignore_ascii_case(b"grid"){b.layout=2;}else{b.layout=0;}}}}}}
#[derive(Clone,Copy)]struct Cell{index:usize,start:usize,end:usize,x:usize,w:usize,last:usize}
struct LayoutRow{cells:[Cell;4],len:usize,h:usize}
#[derive(Default)]pub struct JsReport{pub executed:usize,pub skipped:usize,pub errors:usize,pub external:usize,pub changes:usize,pub modules:usize,pub module_errors:usize}
impl Page{
    pub const fn new()->Self{Self{blocks:[EMPTY;MAX],count:0,styled:0,images:0,scripts:0,stylesheets:[EMPTY_LINK;CSS_LIMIT],stylesheet_count:0,image_urls:[EMPTY_LINK;IMAGE_LIMIT],image_count:0,loaded_images:0,dom_nodes:0,requires_js:false,empty_document:false,truncated:false,form:Form::new(),form_focus:false,title:[0;96],title_len:0}}
    pub fn clear(&mut self){self.count=0;self.styled=0;self.images=0;self.scripts=0;self.title_len=0;self.stylesheet_count=0;self.image_count=0;self.loaded_images=0;self.dom_nodes=0;self.requires_js=false;self.empty_document=false;self.truncated=false;self.form=Form::new();self.form_focus=false;unsafe{for p in (&mut *core::ptr::addr_of_mut!(PHOTOS)).iter_mut(){p.w=0;p.h=0;}}}
    pub fn cache_save(&self,slot:usize){if slot>=4{return;}unsafe{core::ptr::copy_nonoverlapping(self,core::ptr::addr_of_mut!(TAB_PAGES).cast::<Page>().add(slot),1);core::ptr::copy_nonoverlapping(core::ptr::addr_of!(PHOTOS),core::ptr::addr_of_mut!(TAB_PHOTOS).cast::<[Photo;IMAGE_LIMIT]>().add(slot),1);}}
    pub fn cache_load(&mut self,slot:usize){if slot>=4{return;}unsafe{core::ptr::copy_nonoverlapping(core::ptr::addr_of!(TAB_PAGES).cast::<Page>().add(slot),self,1);core::ptr::copy_nonoverlapping(core::ptr::addr_of!(TAB_PHOTOS).cast::<[Photo;IMAGE_LIMIT]>().add(slot),core::ptr::addr_of_mut!(PHOTOS),1);}}
    pub fn cache_move(from:usize,to:usize){if from>=4||to>=4||from==to{return;}unsafe{core::ptr::copy_nonoverlapping(core::ptr::addr_of!(TAB_PAGES).cast::<Page>().add(from),core::ptr::addr_of_mut!(TAB_PAGES).cast::<Page>().add(to),1);core::ptr::copy_nonoverlapping(core::ptr::addr_of!(TAB_PHOTOS).cast::<[Photo;IMAGE_LIMIT]>().add(from),core::ptr::addr_of_mut!(TAB_PHOTOS).cast::<[Photo;IMAGE_LIMIT]>().add(to),1);}}
    pub fn title(&self)->&str{core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")}
    fn push(&mut self,b:&mut Block){if b.len>0&&trim(&b.buf[..b.len]).len()>0&&!b.hidden&&self.count<MAX{if b.color.is_some()||b.bg.is_some(){self.styled+=1;}self.blocks[self.count]=*b;self.count+=1;}*b=EMPTY;}
    fn text(&mut self,b:&mut Block,t:&[u8]){
        let mut i=0;while i<t.len()&&self.count<MAX{let mut ch=t[i];let mut step=1;if ch==b'&'{if let Some((v,n))=numeric_entity(&t[i..]){ch=v;step=n;}else{for(s,v)in[(b"&amp;".as_slice(),b'&'),(b"&lt;",b'<'),(b"&gt;",b'>'),(b"&quot;",b'"'),(b"&#39;",b'\''),(b"&nbsp;",b' '),(b"&copy;",b'c')]{if t[i..].starts_with(s){ch=v;step=s.len();break;}}}}
            if ch.is_ascii_whitespace(){ch=b' ';if b.len==0||b.buf[b.len-1]==b' '{i+=step;continue;}}
            if ch>=128{if let Some((v,n))=utf8_ascii(&t[i..]){ch=v;step=n;}else if ch&0xc0==0x80{i+=step;continue;}else{ch=b'?';}}
            if ch>=32&&ch<127{if b.len==TEXT{let mut next=*b;next.len=0;next.id_len=0;self.push(b);*b=next;}b.buf[b.len]=ch;b.len+=1;}i+=step;
        }
    }
    pub fn parse(&mut self,src:&[u8]){self.parse_at(src,b"http://example.test/",&[]);}
    pub fn parse_at(&mut self,src:&[u8],base:&[u8],css:&[u8]){self.parse_at_width(src,base,css,1024);}
    pub fn parse_at_width(&mut self,src:&[u8],base:&[u8],css:&[u8],viewport:usize){
        self.clear();self.requires_js=src.windows(b"/httpservice/retry/enablejs".len()).any(|v|v.eq_ignore_ascii_case(b"/httpservice/retry/enablejs"));let rules=unsafe{&mut *core::ptr::addr_of_mut!(RULES)};let mut nr=0;css_rules(css,rules,&mut nr,viewport,0);
        // Collect embedded stylesheet blocks and external stylesheet references.
        let mut scan=0;while scan<src.len(){crate::desktop::browser_poll();
            let Some(e)=src[scan..].iter().position(|v|*v==b'<')else{break;};scan+=e;if src[scan..].starts_with(b"<!--"){if let Some(end)=src[scan+4..].windows(3).position(|v|v==b"-->"){scan+=end+7;continue;}break;}
            let Some(e)=tag_end(&src[scan..])else{break;};let raw=trim(&src[scan+1..scan+e]);scan+=e+1;
            let n=raw.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(raw.len());let t=&raw[..n];let a=&raw[n..];
            if raw.starts_with(b"!--"){if let Some(e)=src[scan..].windows(3).position(|v|v==b"-->"){scan+=e+3;}continue;}
            if t.eq_ignore_ascii_case(b"script")||t.eq_ignore_ascii_case(b"style"){
                let stop=if t.eq_ignore_ascii_case(b"style"){b"</style".as_slice()}else{b"</script".as_slice()};
                let Some(end)=src[scan..].windows(stop.len()).position(|v|v.eq_ignore_ascii_case(stop))else{break;};
                if t.eq_ignore_ascii_case(b"style"){css_rules(&src[scan..scan+end],rules,&mut nr,viewport,0);}scan+=end;continue;
            }
            if t.eq_ignore_ascii_case(b"link")&&self.stylesheet_count<CSS_LIMIT&&attr(a,b"rel").is_some_and(|v|v.eq_ignore_ascii_case(b"stylesheet")){
                if let Some(h)=attr(a,b"href"){if let Some(l)=url(base,h){self.stylesheets[self.stylesheet_count]=l;self.stylesheet_count+=1;}}
            }
        }
        // Stable insertion sort: later equal-specificity rules retain source order.
        for i in 1..nr{let rule=rules[i];let mut j=i;while j>0&&rules[j-1].spec>rule.spec{rules[j]=rules[j-1];j-=1;}rules[j]=rule;}
        let mut p=0;let mut b=EMPTY;let mut skip=0u8;let mut active=EMPTY_LINK;
        let dom=unsafe{&mut *core::ptr::addr_of_mut!(coreweb::DOM)};dom.clear();let mut parents=[0usize;coreweb::DEPTH];let mut computed=[EMPTY;coreweb::DEPTH];let mut tags=[(0usize,0usize);coreweb::DEPTH];let mut depth=1usize;let mut overflow=0usize;let mut in_form=false;let mut submit=false;let mut placeholders=0usize;
        while p<src.len()&&self.count<MAX{crate::desktop::browser_poll();
            if src[p]!=b'<'{let e=src[p..].iter().position(|v|*v==b'<').unwrap_or(src.len()-p);if skip==0&&overflow==0{if e>0&&src[p..p+e].iter().any(|v|!v.is_ascii_whitespace()){let _=dom.append(parents[depth-1],p,e,true);}b.link=active;if !b.hidden{self.text(&mut b,&src[p..p+e]);}}else if skip==4{let t=trim(&src[p..p+e]);let n=t.len().min(96);self.title[..n].copy_from_slice(&t[..n]);self.title_len=n;}p+=e;continue;}
            if src[p..].starts_with(b"<!--"){if let Some(e)=src[p+4..].windows(3).position(|v|v==b"-->"){p+=e+7;continue;}break;}
            let Some(e)=tag_end(&src[p..])else{break;};let start=p;let raw=trim(&src[p+1..p+e]);p+=e+1;
            let closing=raw.first()==Some(&b'/');let t=if closing{trim(&raw[1..])}else{raw};let n=t.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(t.len());let tag=&t[..n];let a=&t[n..];
            let group=if tag.eq_ignore_ascii_case(b"script"){1}else if tag.eq_ignore_ascii_case(b"style"){2}else if tag.eq_ignore_ascii_case(b"head"){3}else if tag.eq_ignore_ascii_case(b"title"){4}else{0};
            if matches!(group,1|2)&&!closing{if group==1{self.scripts+=1;}let stop=if group==1{b"</script".as_slice()}else{b"</style".as_slice()};if let Some(e)=src[p..].windows(stop.len()).position(|v|v.eq_ignore_ascii_case(stop)){let pos=p+e;if let Some(end)=tag_end(&src[pos..]){p=pos+end+1;continue;}}break;}
            if skip==3&&group==4&&!closing{skip=4;continue;}if skip==4&&group==4&&closing{skip=3;continue;}if skip!=0{if group==skip&&closing{skip=0;}continue;}if group!=0&&!closing{if group==1{self.scripts+=1;}skip=group;continue;}
            if tag.is_empty()||tag[0]==b'!'{continue;}if !closing&&tag.eq_ignore_ascii_case(b"svg")&&!raw.ends_with(b"/"){if let Some(e)=src[p..].windows(5).position(|v|v.eq_ignore_ascii_case(b"</svg")){let at=p+e;if let Some(end)=tag_end(&src[at..]){p=at+end+1;continue;}}}
            let void=coreweb::void_tag(tag)||raw.ends_with(b"/");
            if overflow>0{if closing{overflow-=1;}else if !void{overflow=overflow.saturating_add(1);}continue;}
            let heading=[b"h1".as_slice(),b"h2",b"h3",b"h4"].iter().any(|v|tag.eq_ignore_ascii_case(v));
            // Each element contributes a bounded box to the current flow renderer.
            if closing{
                if tag.eq_ignore_ascii_case(b"form")&&in_form{self.push(&mut b);if self.form.ready&&!submit{let mut btn=EMPTY;btn.kind=4;btn.align=computed[depth-1].align;self.text(&mut btn,b"Search / Submit");self.push(&mut btn);}in_form=false;}
                let found=(1..depth).rev().find(|&i|{let(s,n)=tags[i];src[s..s+n].eq_ignore_ascii_case(tag)});
                if let Some(i)=found{self.push(&mut b);let node=computed[i];if placeholders<96&&node.id_len>0&&!node.hidden&&self.count<MAX&&!self.blocks[..self.count].iter().any(|v|v.id_len>0&&v.node==node.node){self.blocks[self.count]=node;self.count+=1;placeholders+=1;}depth=i;b=computed[depth-1];active=b.link;}continue;
            }
            if depth>=coreweb::DEPTH&&!void{self.push(&mut b);overflow=1;continue;}
            // Keep each source element with explicit parent/child/sibling relationships.
            let node=dom.append(parents[depth-1],start,p-start,false).unwrap_or(parents[depth-1]);
            let mut next=computed[depth-1];next.len=0;next.id_len=0;next.image=0;next.node=node as u16;next.padding=0;next.margin=0;next.display=None;next.align_set=false;next.layout=0;next.columns=0;next.gap=0;next.width_px=0;next.width_pct=0;next.min_h=0;next.abs_pos=false;next.clip_hidden=false;if !void&&!inline_tag(tag){next.flow=node as u16;}let parent=computed[depth-1];if parent.layout==1||parent.layout==2{next.group=parent.node;next.item=node as u16;next.group_cols=if parent.layout==2{parent.columns.max(2).min(4)}else{4};next.group_gap=parent.gap.max(4);}
            self.push(&mut b);
            if heading||tag.eq_ignore_ascii_case(b"strong"){next.kind=1;}
            if tag.eq_ignore_ascii_case(b"center")||attr(a,b"align").is_some_and(|v|v.eq_ignore_ascii_case(b"center")){next.align=1;}
            if tag.eq_ignore_ascii_case(b"a"){next.link=attr(a,b"href").and_then(|v|url(base,v)).unwrap_or(EMPTY_LINK);}
            // Cascade in ascending specificity, then inline declarations. Inherit text style.
            for rule in &rules[..nr]{if selector_match(&rule.selector[..rule.len],tag,a,src,dom,parents[depth-1]){let st=rule.style;if st.color.is_some(){next.color=st.color;}if st.bg.is_some(){next.bg=st.bg;}if st.kind==1{next.kind=1;}if st.align_set{next.align=st.align;}if st.padding>0{next.padding=st.padding;}if st.margin>0{next.margin=st.margin;}if st.display.is_some(){next.display=st.display;next.layout=st.layout;}if st.columns>0{next.columns=st.columns;}if st.gap>0{next.gap=st.gap;}if st.width_px>0||st.width_pct>0{next.width_px=st.width_px;next.width_pct=st.width_pct;}if st.max_px>0{next.max_px=st.max_px;}if st.min_h>0{next.min_h=st.min_h;}next.auto_margin|=st.auto_margin;next.list_none|=st.list_none;next.abs_pos|=st.abs_pos;next.clip_hidden|=st.clip_hidden;if st.layout==3{next.layout=3;}}}
            if let Some(v)=attr(a,b"style"){style(&mut next,v);}if let Some(v)=attr(a,b"id"){let n=v.len().min(32);next.id[..n].copy_from_slice(&v[..n]);next.id_len=n;}
            // aria-hidden affects accessibility, not visual display. An ancestor display:none cannot be overridden by a descendant.
            next.hidden=computed[depth-1].hidden||next.display==Some(false)||attr(a,b"hidden").is_some()||(next.abs_pos&&(next.clip_hidden||(next.width_px==1&&next.min_h==1)));
            if tag.eq_ignore_ascii_case(b"form")&&!self.form.ready&&!in_form&&!next.hidden{
                let method=attr(a,b"method").unwrap_or(b"get");let action=match attr(a,b"action"){None|Some(b"")=>url(base,base),Some(v)=>url(base,v)};
                if method.eq_ignore_ascii_case(b"get"){if let Some(l)=action{if coreweb::same_origin(base,&l.url[..l.len]){self.form=Form::new();self.form.action=l;in_form=true;submit=false;}}}
            }
            if (tag.eq_ignore_ascii_case(b"input")||tag.eq_ignore_ascii_case(b"textarea"))&&in_form&&!next.hidden&&attr(a,b"disabled").is_none()&&attr(a,b"readonly").is_none(){
                let typ=if tag.eq_ignore_ascii_case(b"textarea"){b"text".as_slice()}else{attr(a,b"type").unwrap_or(b"text")};
                if !self.form.ready&&(typ.eq_ignore_ascii_case(b"text")||typ.eq_ignore_ascii_case(b"search")){
                    if let Some(n)=attr(a,b"name"){if !n.is_empty()&&n.len()<=48&&n.iter().all(|v|v.is_ascii_alphanumeric()||matches!(*v,b'_'|b'-')){self.form.name[..n.len()].copy_from_slice(n);self.form.name_len=n.len();self.form.ready=true;let mut field=next;field.kind=3;self.text(&mut field,attr(a,b"placeholder").filter(|v|!v.is_empty()).unwrap_or(b"Enter search text"));self.push(&mut field);}}
                }else if typ.eq_ignore_ascii_case(b"submit")&&self.form.ready&&!submit{let mut btn=next;btn.kind=4;self.text(&mut btn,attr(a,b"value").filter(|v|!v.is_empty()).unwrap_or(b"Submit"));self.push(&mut btn);submit=true;}
            }
            if tag.eq_ignore_ascii_case(b"textarea"){if let Some(e)=src[p..].windows(10).position(|v|v.eq_ignore_ascii_case(b"</textarea")){let at=p+e;if let Some(end)=tag_end(&src[at..]){p=at+end+1;}}b=computed[depth-1];active=b.link;continue;}
            if tag.eq_ignore_ascii_case(b"button")&&in_form&&self.form.ready&&!submit&&!next.hidden&&attr(a,b"type").map_or(true,|v|v.eq_ignore_ascii_case(b"submit")){next.kind=4;submit=true;}
            if tag.eq_ignore_ascii_case(b"img"){
                let mut pic=next;pic.kind=2;if pic.width_px==0&&pic.width_pct==0{pic.width_px=attr(a,b"width").and_then(dim).unwrap_or(0); }if !pic.hidden{self.images+=1;if self.image_count<IMAGE_LIMIT{if let Some(l)=attr(a,b"src").or_else(||attr(a,b"data-src")).and_then(|v|url(base,v)){self.image_urls[self.image_count]=l;self.image_count+=1;pic.image=self.image_count as u8;}}self.text(&mut pic,b"[Image] ");if let Some(v)=attr(a,b"alt"){self.text(&mut pic,v);}self.push(&mut pic);}
            }
            if !void{
                tags[depth]=(tag.as_ptr() as usize-src.as_ptr() as usize,tag.len());parents[depth]=node;computed[depth]=next;depth+=1;b=next;active=b.link;
                if tag.eq_ignore_ascii_case(b"li")&&!next.list_none{self.text(&mut b,b"- ");}
            }else{b=computed[depth-1];active=b.link;}

        }
        self.push(&mut b);self.dom_nodes=dom.len.saturating_sub(1);self.truncated=dom.truncated||p<src.len();
        self.empty_document=self.count==0;
        if self.empty_document||self.requires_js{let mut notice=EMPTY;notice.kind=1;self.text(&mut notice,if self.requires_js{b"Website requires JavaScript".as_slice()}else{b"No visible content in this response".as_slice()});self.push(&mut notice);let mut detail=EMPTY;self.text(&mut detail,if self.requires_js{b"The server returned a JavaScript interstitial, not search results. Website browser APIs may be incomplete.".as_slice()}else{b"HTML received, but no supported visible content was found. Scripts are disabled.".as_slice()});self.push(&mut detail);}
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
            let Some(rest)=trim(rest).strip_prefix(b"=")else{continue;};let mut value=[0u8;TEXT];let Some(n)=string_expression(trim(rest),&mut value)else{continue;};if id.len()>32||n==0{continue;}
            if let Some(b)=self.blocks[..self.count].iter_mut().find(|b|b.id_len==id.len()&&&b.id[..b.id_len]==id){b.buf[..n].copy_from_slice(&value[..n]);b.len=n;changed+=1;}
        }changed
    }

    pub fn run_js(&mut self,src:&[u8],base:&[u8],ram_mib:u64,mut fetch:impl FnMut(&Link)->Option<alloc::vec::Vec<u8>>)->Result<JsReport,&'static str>{
        use alloc::vec::Vec;
        let mut snippets=[(0usize,0usize,EMPTY_LINK,false);48];let mut script_count=0usize;let mut pos=0usize;let mut report=JsReport::default();
        while pos<src.len()&&script_count<snippets.len(){
            let Some(i)=src[pos..].iter().position(|b|*b==b'<')else{break;};pos+=i;
            if src[pos..].starts_with(b"<!--"){if let Some(end)=src[pos+4..].windows(3).position(|v|v==b"-->"){pos+=end+7;continue;}break;}
            let Some(e)=tag_end(&src[pos..])else{break;};let raw=trim(&src[pos+1..pos+e]);pos+=e+1;
            let k=raw.iter().position(|b|b.is_ascii_whitespace()||*b==b'/').unwrap_or(raw.len());let tag=&raw[..k];
            if tag.eq_ignore_ascii_case(b"style"){if let Some(end)=src[pos..].windows(7).position(|v|v.eq_ignore_ascii_case(b"</style")){pos+=end;continue;}break;}
            if !tag.eq_ignore_ascii_case(b"script"){continue;}
            let Some(end)=src[pos..].windows(8).position(|v|v.eq_ignore_ascii_case(b"</script"))else{break;};
            let a=&raw[k..];let module=attr(a,b"type").is_some_and(|v|v.eq_ignore_ascii_case(b"module"));let classic=attr(a,b"type").map_or(true,|v|[b"".as_slice(),b"text/javascript",b"application/javascript"].iter().any(|t|v.eq_ignore_ascii_case(t)));
            if classic||module{
                if classic&&attr(a,b"nomodule").is_some(){report.skipped+=1;}
                else if let Some(href)=attr(a,b"src"){
                    // Integrity-bearing resources require an SRI verifier; credentials are never sent.
                    if attr(a,b"integrity").is_some()||attr(a,b"crossorigin").is_some_and(|v|v.eq_ignore_ascii_case(b"use-credentials")){report.skipped+=1;}
                    else if let Some(l)=url(base,href).filter(|l|coreweb::script_allowed(base,&l.url[..l.len])){snippets[script_count]=(0,0,l,module);script_count+=1;}else{report.skipped+=1;}
                }else if end>0{if end>crate::webjs::SOURCE_CAP{report.skipped+=1;}else{snippets[script_count]=(pos,end,EMPTY_LINK,module);script_count+=1;}}
            }
            pos+=end;
        }
        if script_count==snippets.len()&&pos<src.len(){report.skipped+=1;}if script_count==0{return Ok(report);}
        let mut session=crate::webjs::Session::new(ram_mib)?;
        let mut bootstrap=Vec::with_capacity(8192);
        bootstrap.extend_from_slice(b"globalThis.window=globalThis;globalThis.self=globalThis;const __h5nodes=Object.create(null),__h5added=[],__h5events=[]; const __h5on=(t,f)=>{if((t==='DOMContentLoaded'||t==='load')&&typeof f==='function'&&__h5events.length<16)__h5events.push(f)}; globalThis.document={readyState:'loading',getElementById:id=>__h5nodes[String(id)]||null,querySelector:s=>typeof s==='string'&&s[0]==='#'?__h5nodes[s.slice(1)]||null:null,createElement:t=>({tagName:String(t).toUpperCase(),textContent:''}),body:{appendChild(n){if(n&&__h5added.length<8)__h5added.push(n);return n}},addEventListener:__h5on};globalThis.addEventListener=__h5on;globalThis.console={log(){},warn(){},error(){}};");
        bootstrap.extend_from_slice(b"document.title=");js_quote(&mut bootstrap,&self.title[..self.title_len]);bootstrap.extend_from_slice(b";globalThis.location={href:");js_quote(&mut bootstrap,base);bootstrap.extend_from_slice(b"};");
        let mut targets=[0usize;96];let mut target_count=0;
        for i in 0..self.count{let b=&self.blocks[i];if b.id_len==0||b.hidden||b.kind>=2||target_count==targets.len(){continue;}
            let id=&b.id[..b.id_len];if !id.iter().all(|v|v.is_ascii_alphanumeric()||matches!(*v,b'-'|b'_'|b'.'|b':'))||targets[..target_count].iter().any(|&j|&self.blocks[j].id[..self.blocks[j].id_len]==id){continue;}
            targets[target_count]=i;target_count+=1;bootstrap.extend_from_slice(b"__h5nodes[");js_quote(&mut bootstrap,id);bootstrap.extend_from_slice(b"]={id:");js_quote(&mut bootstrap,id);bootstrap.extend_from_slice(b",textContent:");js_quote(&mut bootstrap,&b.buf[..b.len]);bootstrap.extend_from_slice(b"};");
        }
        if bootstrap.len()>crate::webjs::SOURCE_CAP{return Err("DOM bootstrap exceeds budget");}
        session.exec(&bootstrap)?;
        let mut total=0usize;let mut external=0usize;let mut module_keys:Vec<Vec<u8>>=Vec::new();
        // Preload only the modules explicitly named by this document. No recursive network fetches or dynamic network import.
        for (ordinal,&(start,len,link,module)) in snippets[..script_count].iter().enumerate(){if crate::desktop::browser_poll(){return Err("JavaScript cancelled");}
            if !module{continue;}
            let mut key=Vec::new();let source=if link.len>0{
                if external>=16{report.skipped+=1;continue;}external+=1;
                key.extend_from_slice(&link.url[..link.len]);match fetch(&link){Some(v)=>{report.external+=1;v},None=>{report.skipped+=1;continue;}}
            }else{key.extend_from_slice(base.split(|v|*v==b'#').next().unwrap_or(base));key.extend_from_slice(b"#h5inline");key.push(b'0'+(ordinal/10)as u8);key.push(b'0'+(ordinal%10)as u8);src[start..start+len].to_vec()};
            if source.len()>crate::webjs::SOURCE_CAP||source.len()>3*1024*1024-total{report.skipped+=1;continue;}total+=source.len();
            if session.add_module(&key,source){module_keys.push(key);}else{report.skipped+=1;}
        }
        for &(start,len,link,module) in &snippets[..script_count]{if crate::desktop::browser_poll(){return Err("JavaScript cancelled");}
            if module{continue;}let downloaded;
            let source=if link.len>0{
                if external>=16{report.skipped+=1;continue;}external+=1;
                downloaded=match fetch(&link){Some(v)=>v,None=>{report.skipped+=1;continue;}};
                report.external+=1;downloaded.as_slice()
            }else{&src[start..start+len]};
            if source.len()>crate::webjs::SOURCE_CAP||source.len()>3*1024*1024-total||core::str::from_utf8(source).is_err(){report.skipped+=1;continue;}total+=source.len();
            match session.exec(source){Ok(())=>{report.executed+=1;session.drain_jobs()?;},Err("JavaScript exception - unsupported API or syntax")=>report.errors+=1,Err(e)=>return Err(e)}
        }
        session.exec(b"document.readyState='interactive'")?;
        for key in &module_keys{match session.exec_module(key){Ok(())=>{report.executed+=1;report.modules+=1;},Err("JavaScript exception - unsupported API or syntax")=>{report.errors+=1;report.module_errors+=1;},Err(e)=>return Err(e)}}
        match session.exec(b"document.readyState='complete';for(const f of __h5events){try{f.call(window)}catch(e){}};"){Ok(())=>session.drain_jobs()?,Err("JavaScript exception - unsupported API or syntax")=>report.errors+=1,Err(e)=>return Err(e)}
        let mut edits=Vec::with_capacity(target_count);let mut query=Vec::with_capacity(8192);
        query.extend_from_slice(b"[");
        for (n,&i) in targets[..target_count].iter().enumerate(){if n>0{query.push(b',');}let b=&self.blocks[i];query.extend_from_slice(b"String(__h5nodes[");js_quote(&mut query,&b.id[..b.id_len]);query.extend_from_slice(b"]?.textContent??'').slice(0,192)");}
        for i in 0..8{if i>0||target_count>0{query.push(b',');}query.extend_from_slice(b"String(__h5added[");query.push(b'0'+i);query.extend_from_slice(b"]?.textContent??'').slice(0,192)");}
        query.extend_from_slice(b"].map(v=>v.replace(/\\u001f/g,' ')).join('\\u001f')");
        let mut output=Vec::new();let cap=(target_count+8)*769;output.try_reserve_exact(cap).map_err(|_|"DOM result buffer allocation")?;output.resize(cap,0);
        let n=session.eval(&query,&mut output)?;let mut values=output[..n].split(|b|*b==31);
        for &i in &targets[..target_count]{let value=values.next().ok_or("Incomplete DOM result")?;let mut buf=[0u8;TEXT];let len=js_text(value,&mut buf);let b=&self.blocks[i];if &b.buf[..b.len]!=&buf[..len]{edits.push((i,buf,len));}}
        let mut added=Vec::new();for _ in 0..8{let value=values.next().ok_or("Incomplete DOM result")?;if !value.is_empty(){let mut buf=[0u8;TEXT];let len=js_text(value,&mut buf);added.push((buf,len));}}
        if values.next().is_some(){return Err("Invalid DOM result");}
        report.errors+=session.job_errors();
        let mut changed=edits.len();for (i,buf,len) in edits{self.blocks[i].buf=buf;self.blocks[i].len=len;}
        for (buf,len) in added{if len==0||self.count>=MAX{continue;}let mut block=EMPTY;block.buf=buf;block.len=len;self.push(&mut block);changed+=1;}
        report.changes=changed;Ok(report)
    }

    pub fn load_image(&mut self,i:usize,bytes:&[u8])->bool{
        if i>=self.image_count{return false;}let photo=unsafe{&mut (*core::ptr::addr_of_mut!(PHOTOS))[i]};let mut w=0u32;let mut h=0u32;
        if unsafe{hi5_image_decode(bytes.as_ptr(),bytes.len(),photo.bytes.as_mut_ptr(),PIX_CAP,&mut w,&mut h)}!=1||w==0||h==0||w as usize>PIX_W||h as usize>PIX_H{return false;}
        if photo.w==0{self.loaded_images+=1;}photo.w=w as usize;photo.h=h as usize;true
    }
    fn cols(b:&Block,width:usize)->usize{if matches!(b.kind,3|4){return TEXT;}(width.saturating_sub((b.padding as usize+b.margin as usize)*2)/if b.kind==1{14}else{9}).clamp(8,TEXT)}
    fn height(b:&Block)->usize{(if b.kind==1{30}else if matches!(b.kind,3|4){38}else{22})+2*(b.padding as usize+b.margin as usize)}
    fn box_width(b:&Block,w:usize)->usize{let mut n=w;if b.width_px>0{n=n.min(b.width_px as usize);}else if b.width_pct>0{n=n.min(w*b.width_pct as usize/100);}if b.max_px>0{n=n.min(b.max_px as usize);}n.max(1)}
    fn photo(b:&Block)->Option<&'static Photo>{if b.image==0{return None;}let p=unsafe{&(*core::ptr::addr_of!(PHOTOS))[b.image as usize-1]};(p.w>0&&p.h>0).then_some(p)}
    fn image_size(p:&Photo,width:usize)->(usize,usize){let w=p.w.min(width.saturating_sub(8)).max(1);(w,(w*p.h/p.w).max(1))}
    fn cell_height(b:&Block,width:usize)->usize{Self::photo(b).map_or(Self::height(b),|p|Self::image_size(p,width).1+8)}
    fn line_end(b:&Block,width:usize,off:usize)->usize{
        if Self::photo(b).is_some()||matches!(b.kind,3|4){return b.len;}
        let avail=width.saturating_sub(2*(b.padding as usize+b.margin as usize+3));let mut end=off;
        while end<b.len{if end>off&&Canvas::measure_text(&b.buf[off..=end],b.kind==1)>avail{break;}end+=1;}
        if end<b.len{b.buf[off..end].iter().rposition(|v|*v==b' ').filter(|n|*n>0).map_or(end,|n|off+n+1)}else{end}
    }
    fn card_height(&self,cell:&Cell)->usize{let mut h=0;for b in &self.blocks[cell.index..cell.last]{if b.len==0{continue;}let bw=Self::box_width(b,cell.w);let mut off=0;let mut bh=0;while off<b.len{off=Self::line_end(b,bw,off);bh+=Self::cell_height(b,bw);}h+=bh.max((b.min_h as usize).min(180));}h}
    // A single row iterator is used by rendering, scrolling and hit-testing.
    // Horizontal layout is deliberately limited to simple consecutive one-block siblings.
    fn next_row(&self,width:usize,index:&mut usize,offset:&mut usize)->Option<LayoutRow>{
        while *index<self.count&&self.blocks[*index].len==0{*index+=1;*offset=0;}if *index>=self.count{return None;}let b=&self.blocks[*index];let mut row=LayoutRow{cells:[Cell{index:*index,start:*offset,end:b.len,x:0,w:width,last:0};4],len:1,h:Self::cell_height(b,width)};
        if *offset==0&&b.group!=0&&b.group_cols>=2&&width>=220{
            let limit=b.group_cols.min(4)as usize;let mut starts=[0usize;4];let mut ends=[0usize;4];let mut count=0;let mut at=*index;
            while count<limit&&at<self.count{let v=&self.blocks[at];if v.group!=b.group||v.item==0{break;}let item=v.item;let begin=at;while at<self.count&&self.blocks[at].group==b.group&&self.blocks[at].item==item{at+=1;}starts[count]=begin;ends[count]=at;count+=1;}
            let gap=b.group_gap as usize;let tracks=if b.group_cols<4{b.group_cols as usize}else{count};let cw=width.saturating_sub(gap*(tracks.saturating_sub(1)))/tracks.max(1);
            if count>1&&cw>=80{
                row.len=count;row.h=0;for i in 0..count{let v=&self.blocks[starts[i]];let cell=Cell{index:starts[i],start:0,end:v.len,x:i*(cw+gap),w:cw,last:ends[i]};
                    row.h=row.h.max(self.card_height(&cell));row.cells[i]=cell;
                }if row.h<=200{*index=at;return Some(row);}row=LayoutRow{cells:[Cell{index:*index,start:*offset,end:b.len,x:0,w:width,last:0};4],len:1,h:Self::cell_height(b,width)};
            }
        }
        if *offset==0&&b.width_px==0&&b.width_pct==0&&b.max_px==0&&b.flow!=0&&b.group==0&&b.kind==0&&b.align==0&&b.bg.is_none()&&b.padding==0&&b.margin==0{
            let mut count=0;let mut used=0usize;
            while count<4&&*index+count<self.count{let v=&self.blocks[*index+count];if v.flow!=b.flow||v.width_px!=0||v.width_pct!=0||v.max_px!=0||v.group!=0||v.kind!=0||v.align!=0||v.bg.is_some()||v.padding!=0||v.margin!=0||v.len==0{break;}
                let cw=Canvas::measure_text(&v.buf[..v.len],false)+8;if used+cw>width{break;}row.cells[count]=Cell{index:*index+count,start:0,end:v.len,x:used,w:cw,last:0};used+=cw;count+=1;
            }if count>1{row.len=count;row.h=22;*index+=count;return Some(row);}row.cells[0]=Cell{index:*index,start:*offset,end:b.len,x:0,w:width,last:0};
        }
        let bw=Self::box_width(b,width);let bx=if b.auto_margin||(b.align==1&&matches!(b.kind,2|3|4)){width.saturating_sub(bw)/2}else{0};row.cells[0].x=bx;row.cells[0].w=bw;row.h=Self::cell_height(b,bw);
        let end=Self::line_end(b,bw,*offset);if *offset==0&&end==b.len{row.h=row.h.max((b.min_h as usize).min(180));}
        row.cells[0].end=end;if end>=b.len{*index+=1;*offset=0;}else{*offset=end;}Some(row)
    }
    pub fn rows(&self,width:usize)->usize{let(mut i,mut off,mut n)=(0,0,0);while self.next_row(width,&mut i,&mut off).is_some(){n+=1;}n}
    pub fn control_hit_xy(&self,width:usize,y:usize,floor:usize,scroll:usize,x:usize,click:usize)->u8{self.cell_hit(width,y,floor,scroll,x,click).map_or(0,|b|if matches!(b.kind,3|4){b.kind}else{0})}
    pub fn hit_xy(&self,width:usize,y:usize,floor:usize,scroll:usize,x:usize,click:usize)->Option<Link>{self.cell_hit(width,y,floor,scroll,x,click).and_then(|b|(b.link.len>0).then_some(b.link))}
    pub fn hit(&self,width:usize,y:usize,floor:usize,scroll:usize,click:usize)->Option<Link>{self.hit_xy(width,y,floor,scroll,0,click)}
    fn cell_hit(&self,width:usize,mut y:usize,floor:usize,scroll:usize,x:usize,click:usize)->Option<&Block>{
        let(mut i,mut off,mut row)=(0,0,0);
        while let Some(r)=self.next_row(width,&mut i,&mut off){if row>=scroll{if y>=floor{return None;}for cell in &r.cells[..r.len]{
            if x<cell.x||x>=cell.x+cell.w||click<y||click>=floor{continue;}
            if cell.last==0{let b=&self.blocks[cell.index];if click<y+Self::cell_height(b,cell.w){return Some(b);}}
            else{let mut cy=y;for b in &self.blocks[cell.index..cell.last]{if b.len==0{continue;}let bw=Self::box_width(b,cell.w);let bx=cell.x+if b.auto_margin{(cell.w-bw)/2}else{0};let start=cy;let mut off=0;while off<b.len{off=Self::line_end(b,bw,off);let h=Self::cell_height(b,bw);if x>=bx&&x<bx+bw&&click>=cy&&click<cy+h{return Some(b);}cy+=h;}cy=cy.max(start+(b.min_h as usize).min(180));}}
        }y+=r.h;}row+=1;}None
    }
    fn draw_line(&self,c:&Canvas,b:&Block,x:usize,y:usize,width:usize,start:usize,end:usize,floor:usize,ink:Color,sub:Color){
        if y>=floor{return;}let right=x+width;let height=Self::cell_height(b,width);let visible=height.min(floor-y);
        if let Some(p)=Self::photo(b){let(w,h)=Self::image_size(p,width);let ox=if b.align==1{x+(width-w)/2}else if b.align==2{x+width-w}else{x+3};for j in 0..h.min(floor-y){for k in 0..w{let at=((j*p.h/h)*p.w+k*p.w/w)*3;c.pixel(ox+k,y+j,[p.bytes[at],p.bytes[at+1],p.bytes[at+2]]);}}return;}
        if let Some(bg)=b.bg{c.rect(x,y,width,visible,bg);}else if b.kind==2{c.rect(x,y,width,visible,sub);}
        if y+height>floor{return;}
        if let Ok(t)=core::str::from_utf8(trim(&b.buf[start..end])){let big=b.kind==1;let mut color=b.color.unwrap_or(if b.link.len>0{[31,97,185]}else{ink});let bg=b.bg.unwrap_or(if b.kind==2{sub}else{[255,255,255]});let lum=|c:Color| (299*c[0]as i32+587*c[1]as i32+114*c[2]as i32)/1000;if (lum(color)-lum(bg)).abs()<45{color=if lum(bg)>128{[30,34,40]}else{[240,243,247]};}let inset=b.padding as usize+b.margin as usize+3;let tw=Canvas::measure_text(t.as_bytes(),big);let tx=if b.align==1{x+width.saturating_sub(tw)/2}else if b.align==2{right.saturating_sub(tw+inset).max(x)}else{x+inset};
            if matches!(b.kind,3|4){let cw=if b.kind==3{width.min(420)}else{width.min(180)};let ox=if b.align==1{x+(width-cw)/2}else{x};if cw<14{return;}c.round(ox+2,y+2,cw-4,height-4,5,if b.kind==3&&self.form_focus{[207,224,247]}else{[227,231,237]});let val=if b.kind==3&&self.form.len>0{core::str::from_utf8(&self.form.value[..self.form.len]).unwrap_or("")}else{t};c.text_limit(ox+10,y+10,val,[30,34,40],false,ox+cw-5);}
            else{c.text_limit(tx,y+inset,t,color,big,right);if b.link.len>0&&tx<right{c.rect(tx,y+height-3,tw.min(right-tx),1,color);}}
        }
    }
    pub fn draw(&self,c:&Canvas,x:usize,mut y:usize,right:usize,floor:usize,scroll:usize,ink:Color,sub:Color){
        let width=right.saturating_sub(x);if width<40{return;}let(mut i,mut off,mut row)=(0,0,0);
        while let Some(r)=self.next_row(width,&mut i,&mut off){if row>=scroll{if y>=floor{return;}for cell in &r.cells[..r.len]{
            if cell.last==0{self.draw_line(c,&self.blocks[cell.index],x+cell.x,y,cell.w,cell.start,cell.end,floor,ink,sub);}
            else{let mut cy=y;for b in &self.blocks[cell.index..cell.last]{if b.len==0{continue;}let bw=Self::box_width(b,cell.w);let bx=x+cell.x+if b.auto_margin{(cell.w-bw)/2}else{0};let start=cy;let mut off=0;while off<b.len&&cy<floor{let end=Self::line_end(b,bw,off);self.draw_line(c,b,bx,cy,bw,off,end,floor,ink,sub);cy+=Self::cell_height(b,bw);off=end;}cy=cy.max(start+(b.min_h as usize).min(180));if cy>=floor{break;}}}
        }y+=r.h;}row+=1;}
    }

}
fn ascii_codepoint(v:u32)->u8{match v{32..=126=>v as u8,160=>b' ',0x2018|0x2019|0x2032=>b'\'',0x201c|0x201d|0x2033=>b'"',0x2010..=0x2015|0x2212=>b'-',0x2022=>b'*',0x2026=>b'.',0xa9=>b'c',0xae=>b'r',_=>b'?'}}
fn numeric_entity(s:&[u8])->Option<(u8,usize)>{
    let tail=s.strip_prefix(b"&#")?;let (radix,skip)=if tail.first().is_some_and(|b|matches!(*b,b'x'|b'X')){(16,1)}else{(10,0)};
    let n=tail[skip..].iter().take(9).position(|v|*v==b';')?;if n==0{return None;}let mut cp=0u32;for b in &tail[skip..skip+n]{cp=cp.checked_mul(radix)?.checked_add((*b as char).to_digit(radix)?)?;}if char::from_u32(cp).is_none(){return None;}Some((ascii_codepoint(cp),2+skip+n+1))
}
fn utf8_ascii(s:&[u8])->Option<(u8,usize)>{let first=*s.first()?;let n=match first{0xc2..=0xdf=>2,0xe0..=0xef=>3,0xf0..=0xf4=>4,_=>return None};let c=core::str::from_utf8(s.get(..n)?).ok()?.chars().next()?;Some((ascii_codepoint(c as u32),n))}
fn js_quote(out:&mut alloc::vec::Vec<u8>,s:&[u8]){out.push(b'"');const H:&[u8;16]=b"0123456789abcdef";for &b in s{match b{b'"'|b'\\'=>{out.push(b'\\');out.push(b);},32..=126=>out.push(b),_=>{out.extend_from_slice(b"\\u00");out.push(H[(b>>4)as usize]);out.push(H[(b&15)as usize]);}}}out.push(b'"');}
fn js_text(s:&[u8],out:&mut[u8;TEXT])->usize{let mut n=0;for &b in s{if n==TEXT{break;}if b&0xc0==0x80{continue;}out[n]=if b.is_ascii_whitespace(){b' '}else if (32..127).contains(&b){b}else{b'?'};n+=1;}n}
fn url(base:&[u8],href:&[u8])->Option<Link>{let mut decoded=[0u8;128];let(mut i,mut n)=(0,0);while i<href.len(){if n==decoded.len(){return None;}let mut consumed=1;let mut b=href[i];for ent in [b"&amp;".as_slice(),b"&#38;",b"&#x26;"]{if href[i..].starts_with(ent){b=b'&';consumed=ent.len();break;}}decoded[n]=b;n+=1;i+=consumed;}let mut l=EMPTY_LINK;l.len=crate::http::resolve_url(base,&decoded[..n],&mut l.url)?;if base.starts_with(b"https://")&&!l.url[..l.len].starts_with(b"https://"){return None;}Some(l)}
fn tag_end(s:&[u8])->Option<usize>{let mut q=0;for(i,b)in s.iter().enumerate().skip(1){if q!=0{if *b==q{q=0;}}else if *b==b'\''||*b==b'"'{q=*b;}else if *b==b'>'{return Some(i);}}None}
// Bounded compound and descendant selectors; unsupported syntax is rejected rather than partially matched.
fn ident(b:u8)->bool{b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')}
fn compound(s:&[u8],tag:&[u8],attrs:&[u8])->bool{
    let mut i=0;if s.first()==Some(&b'*'){i=1;}else if s.first().is_some_and(|b|ident(*b)){i=s.iter().position(|b|!ident(*b)).unwrap_or(s.len());if !tag.eq_ignore_ascii_case(&s[..i]){return false;}}
    while i<s.len(){let k=s[i];if !matches!(k,b'.'|b'#'){return false;}i+=1;let e=s[i..].iter().position(|b|!ident(*b)).map_or(s.len(),|n|i+n);if e==i{return false;}let v=&s[i..e];if k==b'.'{if !attr(attrs,b"class").is_some_and(|a|a.split(|b|b.is_ascii_whitespace()).any(|x|x==v)){return false;}}else if attr(attrs,b"id")!=Some(v){return false;}i=e;}!s.is_empty()
}
fn selector_parts(s:&[u8])->Option<([&[u8];8],[bool;8],usize)>{
    let mut parts=[b"".as_slice();8];let mut direct=[false;8];let mut n=0;let mut p=0;let mut child=false;
    while p<s.len(){while p<s.len()&&s[p].is_ascii_whitespace(){p+=1;}if p==s.len(){break;}
        if s[p]==b'>'{if n==0||child{return None;}child=true;p+=1;continue;}
        if n==parts.len(){return None;}let start=p;while p<s.len()&&!s[p].is_ascii_whitespace()&&s[p]!=b'>'{p+=1;}
        parts[n]=&s[start..p];direct[n]=child;n+=1;child=false;
    }if n==0||child{None}else{Some((parts,direct,n))}
}
fn selector_spec(s:&[u8])->Option<u16>{
    let (parts,_,n)=selector_parts(s)?;let mut score=0u16;
    for part in &parts[..n]{let mut i=0;
        if part.first()==Some(&b'*'){i=1;}else if part.first().is_some_and(|b|ident(*b)){i=part.iter().position(|b|!ident(*b)).unwrap_or(part.len());score+=1;}
        while i<part.len(){let k=part[i];if !matches!(k,b'.'|b'#'){return None;}score+=if k==b'#'{256}else{16};i+=1;let e=part[i..].iter().position(|b|!ident(*b)).map_or(part.len(),|n|i+n);if e==i{return None;}i=e;}
    }Some(score)
}
fn selector_match(s:&[u8],tag:&[u8],attrs:&[u8],src:&[u8],dom:&coreweb::Tree,mut parent:usize)->bool{
    let Some((parts,direct,n))=selector_parts(s)else{return false;};if !compound(parts[n-1],tag,attrs){return false;}
    for i in (0..n-1).rev(){let mut found=false;while parent>0&&parent<dom.len{let node=dom.nodes[parent];parent=node.parent as usize;
        if let Some(raw)=src.get(node.start+1..node.start+node.len.saturating_sub(1)){let raw=trim(raw);let n=raw.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(raw.len());if compound(parts[i],&raw[..n],&raw[n..]){found=true;break;}}
        if direct[i+1]{break;}
    }if !found{return false;}}true
}
fn inline_tag(t:&[u8])->bool{[b"a".as_slice(),b"span",b"em",b"strong",b"b",b"i",b"small",b"label",b"abbr",b"code",b"u",b"time"].iter().any(|v|t.eq_ignore_ascii_case(v))}
fn media_applies(s:&[u8],width:usize)->bool{
    let mut q=trim(s);if q.starts_with(b"screen"){q=trim(&q[6..]);if q.is_empty(){return true;}let Some(v)=q.strip_prefix(b"and")else{return false;};q=trim(v);}
    if q==b"all"{return true;}let Some(q)=q.strip_prefix(b"(").and_then(|v|v.strip_suffix(b")"))else{return false;};
    let Some(i)=q.iter().position(|b|*b==b':')else{return false;};let key=trim(&q[..i]);let Some(v)=trim(&q[i+1..]).strip_suffix(b"px")else{return false;};if v.is_empty()||v.len()>5||!v.iter().all(u8::is_ascii_digit){return false;}let mut n=0usize;for b in v{n=n*10+(b-b'0')as usize;}match key{b"min-width"=>width>=n,b"max-width"=>width<=n,_=>false}
}
fn css_rules(mut css:&[u8],out:&mut[Rule;RULE_CAP],count:&mut usize,viewport:usize,level:usize){
    while !css.is_empty()&&*count<out.len(){crate::desktop::browser_poll();
        css=trim(css);if css.starts_with(b"/*"){let Some(e)=css[2..].windows(2).position(|v|v==b"*/")else{break;};css=&css[e+4..];continue;}
        let Some(i)=css.iter().position(|v|*v==b'{')else{break;};
        // Skip balanced nested rules (@media/@supports etc.) rather than treating inner selectors as global.
        let mut depth=1usize;let mut end=None;let mut quote=0u8;let mut escaped=false;
        for (j,&v) in css[i+1..].iter().enumerate(){if quote!=0{if escaped{escaped=false;}else if v==b'\\'{escaped=true;}else if v==quote{quote=0;}continue;}if matches!(v,b'\''|b'"'){quote=v;continue;}if v==b'{'{depth+=1;}else if v==b'}'{depth-=1;if depth==0{end=Some(i+1+j);break;}}}
        let Some(j)=end else{break;};let sel=trim(&css[..i]);let body=&css[i+1..j];css=&css[j+1..];if sel.starts_with(b"@"){if level<4{if let Some(q)=sel.strip_prefix(b"@media"){if media_applies(q,viewport){css_rules(body,out,count,viewport,level+1);}}}continue;}
        let mut b=EMPTY;style(&mut b,body);if b.color.is_none()&&b.bg.is_none()&&b.kind==0&&!b.align_set&&b.padding==0&&b.margin==0&&b.display.is_none()&&b.columns==0&&b.gap==0&&b.layout==0&&b.width_px==0&&b.width_pct==0&&b.max_px==0&&b.min_h==0&&!b.auto_margin&&!b.list_none&&!b.abs_pos&&!b.clip_hidden{continue;}
        for s in sel.split(|v|*v==b','){let s=trim(s);if s.len()>96||*count>=out.len(){continue;}let Some(spec)=selector_spec(s)else{continue;};let mut r=EMPTY_RULE;r.selector[..s.len()].copy_from_slice(s);r.len=s.len();r.spec=spec;r.style=b;out[*count]=r;*count+=1;}
    }
}
// Only a bounded string-literal concatenation expression; no arbitrary JS, loops or host calls.
fn string_expression(mut s:&[u8],out:&mut[u8;TEXT])->Option<usize>{let mut n=0;for _ in 0..8{let (v,rest)=literal(trim(s))?;if v.len()>out.len()-n{return None;}out[n..n+v.len()].copy_from_slice(v);n+=v.len();let rest=trim(rest);if matches!(rest,b""|b";"){return Some(n);}s=rest.strip_prefix(b"+")?;}None}
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
    p.parse_at(b"<h1 class='headline'>A</h1><p><a href='/news'>News</a></p>",b"https://example.test/",b".headline { color: #123456; }");let css_ok=p.styled==1&&p.blocks[0].color==Some([18,52,86])&&p.blocks[1].link.len>0&&p.hit(400,0,200,0,31).is_some_and(|l|&l.url[..l.len]==b"https://example.test/news");p.parse_at(b"<center><div style='color:#123456'><span>Nested</span></div><form action='/search'><input name='q'><input type='submit' value='Go'></form></center>",b"https://example.test/",&[]);let form_ok=p.dom_nodes>=7&&p.blocks[0].align==1&&p.blocks[0].color==Some([18,52,86])&&p.form.ready&&p.blocks[..p.count].iter().any(|v|v.kind==3)&&p.blocks[..p.count].iter().any(|v|v.kind==4);p.parse_at(b"<head><title>Search</title></head><body><style>div{display:none}.result{display:block}.result a.link{color:#123456} @media print {a{display:none}}</style><div class='result'><a class='link' href='/article?a=1&amp;b=2' aria-hidden='true'>Real result</a></div><div>Hidden</div></body>",b"https://example.test/",&[]);let result_ok=p.count==1&&p.blocks[0].color==Some([18,52,86])&&&p.blocks[0].buf[..p.blocks[0].len]==b"Real result"&&p.hit(400,0,200,0,10).is_some_and(|l|&l.url[..l.len]==b"https://example.test/article?a=1&b=2");
    p.parse(b"<html><head><title>Google Search</title></head><body><noscript><style>div{display:none}</style><div style='display:block'>Enable JavaScript <a href='/httpservice/retry/enablejs'>here</a></div></noscript><script>void(0)</script></body></html>");let interstitial_ok=p.requires_js&&p.count>=3&&!p.empty_document;
    p.parse(b"<html><body><div hidden>Not visible</div><script>void(0)</script></body></html>");let empty_ok=p.empty_document&&p.count==2;
    p.parse_at(b"<div style='display:grid;grid-template-columns:1fr 1fr;gap:8px'><a href='/one'>One</a><a href='/two'>Two</a></div>",b"https://example.test/",&[]);let layout_ok=p.count==2&&p.rows(400)==1&&p.hit_xy(400,0,200,0,10,5).is_some_and(|l|&l.url[..l.len]==b"https://example.test/one")&&p.hit_xy(400,0,200,0,300,5).is_some_and(|l|&l.url[..l.len]==b"https://example.test/two");
    p.parse(b"<p id='demo'>Before</p>");let script_ok=p.literal_update(b"<script>document.getElementById('demo').textContent = 'Hi5' + 'Browser';</script>")==1&&&p.blocks[0].buf[..p.blocks[0].len]==b"Hi5Browser"&&p.literal_update(b"<script>document.getElementById('demo').textContent = 'a' + evil();</script>")==0;
    p.parse_at_width(b"<p>Hello <a href='/next'>world</a> again</p><style>@media (max-width: 500px){p{color:#123456}}</style>",b"https://example.test/",&[],400);let responsive_ok=p.count==3&&p.rows(400)==1&&p.blocks[0].color==Some([18,52,86]);
    p.parse_at_width(b"<p>Hello</p><style>@media (max-width: 500px){p{color:#123456}}</style>",b"https://example.test/",&[],900);let wide_ok=p.blocks[0].color.is_none();
    p.parse_at(b"<ul style='list-style:none'><li>News</li></ul><p style='width:50%;margin:auto'>Centered</p>",b"https://example.test/",&[]);let box_ok=p.count==2&&&p.blocks[0].buf[..p.blocks[0].len]==b"News"&&{let(mut i,mut off)=(1,0);p.next_row(400,&mut i,&mut off).is_some_and(|r|r.cells[0].x==100&&r.cells[0].w==200)};
    p.parse_at(b"<div class='nav'><a href='/yes'>Direct</a><span><a href='/no'>Nested</a></span></div><style>.nav>a{color:#123456}</style>",b"https://example.test/",&[]);let child_ok=p.count==2&&p.blocks[0].color==Some([18,52,86])&&p.blocks[1].color.is_none();
    p.parse_at(b"<div style='display:grid;grid-template-columns:repeat(2, 1fr)'><article><h3>First</h3><a href='/one'>One</a></article><article><h3>Second</h3><a href='/two'>Two</a></article></div>",b"https://example.test/",&[]);let grid_ok=p.count==4&&p.rows(600)==1&&p.hit_xy(600,0,200,0,10,31).is_some_and(|l|&l.url[..l.len]==b"https://example.test/one")&&p.hit_xy(600,0,200,0,350,31).is_some_and(|l|&l.url[..l.len]==b"https://example.test/two");
    p.parse(b"<p>It&#x27;s &#39;fine&#39; &amp; &#x2014; OK</p>");let entity_ok=p.count==1&&&p.blocks[0].buf[..p.blocks[0].len]==b"It\'s \'fine\' & - OK";
    let rgb_ok=rgb(b"rgb(18, 52, 86)")==Some([18,52,86])&&rgb(b"rgb(100% 0% 0%)")==Some([255,0,0])&&background(b"linear-gradient(120deg, #112233, #445566)")==Some([17,34,51])&&rgb(b"rgba(1,2,3,0.5)").is_none();
    p.clear();ok&&css_ok&&form_ok&&result_ok&&interstitial_ok&&empty_ok&&layout_ok&&script_ok&&responsive_ok&&wide_ok&&box_ok&&child_ok&&grid_ok&&entity_ok&&rgb_ok&&coreweb::self_test()&&Bitmap::parse(b"BM").is_none()
}
