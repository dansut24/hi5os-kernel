//! Bounded native document/layout model. No arbitrary script evaluation or network side effects.
use crate::ui::{Canvas,Color};
use crate::http::{Link,EMPTY_LINK};
use crate::hi5webcore::{self as coreweb,Form};
const MAX:usize=2048;
const IMAGE_LIMIT:usize=4;
const CSS_LIMIT:usize=2;
const PIX_W:usize=320;const PIX_H:usize=180;const PIX_CAP:usize=PIX_W*PIX_H*3;
#[derive(Clone,Copy)]struct Photo{w:usize,h:usize,bytes:[u8;PIX_CAP]}
const NO_PHOTO:Photo=Photo{w:0,h:0,bytes:[0;PIX_CAP]};
static mut PHOTOS:[Photo;IMAGE_LIMIT]=[NO_PHOTO;IMAGE_LIMIT];
#[link(name="hi5image",kind="static")]extern "C"{fn hi5_image_decode(data:*const u8,len:usize,out:*mut u8,cap:usize,w:*mut u32,h:*mut u32)->i32;}
#[derive(Clone,Copy)]struct Rule{selector:[u8;96],len:usize,style:Block,spec:u16}
const EMPTY_RULE:Rule=Rule{selector:[0;96],len:0,style:EMPTY,spec:0};
static mut RULES:[Rule;256]=[EMPTY_RULE;256];
const TEXT:usize=192;
#[derive(Clone,Copy)]struct Block{buf:[u8;TEXT],len:usize,kind:u8,color:Option<Color>,bg:Option<Color>,id:[u8;32],id_len:usize,link:Link,image:u8,align:u8,padding:u8,margin:u8,hidden:bool,display:Option<bool>,align_set:bool,node:u16}
const EMPTY:Block=Block{buf:[0;TEXT],len:0,kind:0,color:None,bg:None,id:[0;32],id_len:0,link:EMPTY_LINK,image:0,align:0,padding:0,margin:0,hidden:false,display:None,align_set:false,node:0};
#[derive(Clone,Copy)]pub struct Page{blocks:[Block;MAX],pub count:usize,pub styled:usize,pub images:usize,pub scripts:usize,pub stylesheets:[Link;CSS_LIMIT],pub stylesheet_count:usize,pub image_urls:[Link;IMAGE_LIMIT],pub image_count:usize,pub loaded_images:usize,pub dom_nodes:usize,pub requires_js:bool,pub empty_document:bool,pub truncated:bool,pub form:Form,pub form_focus:bool,title:[u8;96],title_len:usize}
pub static mut PAGE:Page=Page::new();
fn trim(b:&[u8])->&[u8]{let mut a=0;let mut z=b.len();while a<z&&b[a].is_ascii_whitespace(){a+=1;}while z>a&&b[z-1].is_ascii_whitespace(){z-=1;}&b[a..z]}
fn attr<'a>(mut a:&'a[u8],want:&[u8])->Option<&'a[u8]>{
    while !a.is_empty(){a=trim(a);if a.is_empty(){break;}let n=a.iter().position(|b|b.is_ascii_whitespace()||*b==b'=').unwrap_or(a.len());if n==0{return None;}let k=&a[..n];a=trim(&a[n..]);if a.first()!=Some(&b'='){if k.eq_ignore_ascii_case(want){return Some(&[]);}continue;}a=trim(&a[1..]);let q=*a.first()?;let(v,rest)=if q==b'\''||q==b'"'{let e=a[1..].iter().position(|b|*b==q)?+1;(&a[1..e],&a[e+1..])}else{let e=a.iter().position(|b|b.is_ascii_whitespace()).unwrap_or(a.len());(&a[..e],&a[e..])};if k.eq_ignore_ascii_case(want){return Some(v);}a=rest;
    }None
}
fn rgb(s:&[u8])->Option<Color>{let s=trim(s);let named=[(b"red".as_slice(),[220,30,30]),(b"blue",[20,90,210]),(b"black",[0,0,0]),(b"white",[255,255,255]),(b"green",[0,128,0]),(b"navy",[0,0,128]),(b"gray",[128,128,128])];for(n,c)in named{if s.eq_ignore_ascii_case(n){return Some(c);}}if s.len()!=7||s[0]!=b'#'{return None;}let mut c=[0;3];for i in 0..3{let h=(s[1+i*2]as char).to_digit(16)?;let l=(s[2+i*2]as char).to_digit(16)?;c[i]=(h*16+l)as u8;}Some(c)}
fn px(s:&[u8])->Option<u8>{let s=s.strip_suffix(b"px").unwrap_or(s);if s.is_empty()||s.len()>3||!s.iter().all(u8::is_ascii_digit){return None;}let mut n=0u16;for v in s{n=n*10+(v-48)as u16;}Some(n.min(40)as u8)}
fn style(b:&mut Block,s:&[u8]){for decl in s.split(|v|*v==b';'){if let Some(i)=decl.iter().position(|v|*v==b':'){let k=trim(&decl[..i]);let v=trim(&decl[i+1..]);if k.eq_ignore_ascii_case(b"color"){if let Some(c)=rgb(v){b.color=Some(c);}}else if k.eq_ignore_ascii_case(b"background-color"){if let Some(c)=rgb(v){b.bg=Some(c);}}else if k.eq_ignore_ascii_case(b"font-weight")&&matches!(v,b"bold"|b"700"){b.kind=1;}else if k.eq_ignore_ascii_case(b"font-size")&&matches!(v,b"24px"|b"28px"|b"32px"){b.kind=1;}else if k.eq_ignore_ascii_case(b"text-align"){b.align_set=true;b.align=if v.eq_ignore_ascii_case(b"center"){1}else if v.eq_ignore_ascii_case(b"right"){2}else{0};}else if k.eq_ignore_ascii_case(b"padding"){if let Some(n)=px(v){b.padding=n;}}else if k.eq_ignore_ascii_case(b"margin"){if let Some(n)=px(v){b.margin=n;}}else if k.eq_ignore_ascii_case(b"display"){if v.eq_ignore_ascii_case(b"none"){b.display=Some(false);}else if [b"block".as_slice(),b"inline",b"inline-block",b"flex",b"grid",b"table",b"list-item",b"contents"].iter().any(|x|v.eq_ignore_ascii_case(x)){b.display=Some(true);}}}}}
impl Page{
    pub const fn new()->Self{Self{blocks:[EMPTY;MAX],count:0,styled:0,images:0,scripts:0,stylesheets:[EMPTY_LINK;CSS_LIMIT],stylesheet_count:0,image_urls:[EMPTY_LINK;IMAGE_LIMIT],image_count:0,loaded_images:0,dom_nodes:0,requires_js:false,empty_document:false,truncated:false,form:Form::new(),form_focus:false,title:[0;96],title_len:0}}
    pub fn clear(&mut self){self.count=0;self.styled=0;self.images=0;self.scripts=0;self.title_len=0;self.stylesheet_count=0;self.image_count=0;self.loaded_images=0;self.dom_nodes=0;self.requires_js=false;self.empty_document=false;self.truncated=false;self.form=Form::new();self.form_focus=false;unsafe{for p in (&mut *core::ptr::addr_of_mut!(PHOTOS)).iter_mut(){p.w=0;p.h=0;}}}
    pub fn title(&self)->&str{core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")}
    fn push(&mut self,b:&mut Block){if b.len>0&&!b.hidden&&self.count<MAX{if b.color.is_some()||b.bg.is_some(){self.styled+=1;}self.blocks[self.count]=*b;self.count+=1;}*b=EMPTY;}
    fn text(&mut self,b:&mut Block,t:&[u8]){
        let mut i=0;while i<t.len()&&self.count<MAX{let mut ch=t[i];let mut step=1;if ch==b'&'{for(s,v)in[(b"&amp;".as_slice(),b'&'),(b"&lt;",b'<'),(b"&gt;",b'>'),(b"&quot;",b'"'),(b"&#39;",b'\''),(b"&nbsp;",b' '),(b"&copy;",b'c')]{if t[i..].starts_with(s){ch=v;step=s.len();break;}}}
            if ch.is_ascii_whitespace(){ch=b' ';if b.len==0||b.buf[b.len-1]==b' '{i+=step;continue;}}
            if ch>=128{if ch&0xc0==0x80{i+=step;continue;}ch=b'?';}
            if ch>=32&&ch<127{if b.len==TEXT{let mut next=*b;next.len=0;next.id_len=0;self.push(b);*b=next;}b.buf[b.len]=ch;b.len+=1;}i+=step;
        }
    }
    pub fn parse(&mut self,src:&[u8]){self.parse_at(src,b"http://example.test/",&[]);}
    pub fn parse_at(&mut self,src:&[u8],base:&[u8],css:&[u8]){
        self.clear();self.requires_js=src.windows(b"/httpservice/retry/enablejs".len()).any(|v|v.eq_ignore_ascii_case(b"/httpservice/retry/enablejs"));let rules=unsafe{&mut *core::ptr::addr_of_mut!(RULES)};let mut nr=0;css_rules(css,rules,&mut nr);
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
        // Stable insertion sort: later equal-specificity rules retain source order.
        for i in 1..nr{let rule=rules[i];let mut j=i;while j>0&&rules[j-1].spec>rule.spec{rules[j]=rules[j-1];j-=1;}rules[j]=rule;}
        let mut p=0;let mut b=EMPTY;let mut skip=0u8;let mut active=EMPTY_LINK;
        let dom=unsafe{&mut *core::ptr::addr_of_mut!(coreweb::DOM)};dom.clear();let mut parents=[0usize;coreweb::DEPTH];let mut computed=[EMPTY;coreweb::DEPTH];let mut tags=[(0usize,0usize);coreweb::DEPTH];let mut depth=1usize;let mut overflow=0usize;let mut in_form=false;let mut submit=false;
        while p<src.len()&&self.count<MAX{
            if src[p]!=b'<'{let e=src[p..].iter().position(|v|*v==b'<').unwrap_or(src.len()-p);if skip==0&&overflow==0{if e>0&&src[p..p+e].iter().any(|v|!v.is_ascii_whitespace()){let _=dom.append(parents[depth-1],p,e,true);}b.link=active;if !b.hidden{self.text(&mut b,&src[p..p+e]);}}else if skip==4{let t=trim(&src[p..p+e]);let n=t.len().min(96);self.title[..n].copy_from_slice(&t[..n]);self.title_len=n;}p+=e;continue;}
            if src[p..].starts_with(b"<!--"){if let Some(e)=src[p+4..].windows(3).position(|v|v==b"-->"){p+=e+7;continue;}break;}
            let Some(e)=tag_end(&src[p..])else{break;};let start=p;let raw=trim(&src[p+1..p+e]);p+=e+1;
            let closing=raw.first()==Some(&b'/');let t=if closing{trim(&raw[1..])}else{raw};let n=t.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(t.len());let tag=&t[..n];let a=&t[n..];
            let group=if tag.eq_ignore_ascii_case(b"script"){1}else if tag.eq_ignore_ascii_case(b"style"){2}else if tag.eq_ignore_ascii_case(b"head"){3}else if tag.eq_ignore_ascii_case(b"title"){4}else{0};
            if matches!(group,1|2)&&!closing{if group==1{self.scripts+=1;}let stop=if group==1{b"</script".as_slice()}else{b"</style".as_slice()};if let Some(e)=src[p..].windows(stop.len()).position(|v|v.eq_ignore_ascii_case(stop)){let pos=p+e;if let Some(end)=tag_end(&src[pos..]){p=pos+end+1;continue;}}break;}
            if skip==3&&group==4&&!closing{skip=4;continue;}if skip==4&&group==4&&closing{skip=3;continue;}if skip!=0{if group==skip&&closing{skip=0;}continue;}if group!=0&&!closing{if group==1{self.scripts+=1;}skip=group;continue;}
            if tag.is_empty()||tag[0]==b'!'{continue;}
            let void=coreweb::void_tag(tag)||raw.ends_with(b"/");
            if overflow>0{if closing{overflow-=1;}else if !void{overflow=overflow.saturating_add(1);}continue;}
            let heading=[b"h1".as_slice(),b"h2",b"h3",b"h4"].iter().any(|v|tag.eq_ignore_ascii_case(v));
            // Each element contributes a bounded box to the current flow renderer.
            if closing{
                if tag.eq_ignore_ascii_case(b"form")&&in_form{self.push(&mut b);if self.form.ready&&!submit{let mut btn=EMPTY;btn.kind=4;btn.align=computed[depth-1].align;self.text(&mut btn,b"Search / Submit");self.push(&mut btn);}in_form=false;}
                let found=(1..depth).rev().find(|&i|{let(s,n)=tags[i];src[s..s+n].eq_ignore_ascii_case(tag)});
                if let Some(i)=found{self.push(&mut b);depth=i;b=computed[depth-1];active=b.link;}continue;
            }
            if depth>=coreweb::DEPTH&&!void{self.push(&mut b);overflow=1;continue;}
            // Keep each source element with explicit parent/child/sibling relationships.
            let node=dom.append(parents[depth-1],start,p-start,false).unwrap_or(parents[depth-1]);
            let mut next=computed[depth-1];next.len=0;next.id_len=0;next.image=0;next.node=node as u16;next.padding=0;next.margin=0;next.display=None;next.align_set=false;
            self.push(&mut b);
            if heading||tag.eq_ignore_ascii_case(b"strong"){next.kind=1;}
            if tag.eq_ignore_ascii_case(b"center")||attr(a,b"align").is_some_and(|v|v.eq_ignore_ascii_case(b"center")){next.align=1;}
            if tag.eq_ignore_ascii_case(b"a"){next.link=attr(a,b"href").and_then(|v|url(base,v)).unwrap_or(EMPTY_LINK);}
            // Cascade in ascending specificity, then inline declarations. Inherit text style.
            for rule in &rules[..nr]{if selector_match(&rule.selector[..rule.len],tag,a,src,dom,parents[depth-1]){let st=rule.style;if st.color.is_some(){next.color=st.color;}if st.bg.is_some(){next.bg=st.bg;}if st.kind==1{next.kind=1;}if st.align_set{next.align=st.align;}if st.padding>0{next.padding=st.padding;}if st.margin>0{next.margin=st.margin;}if st.display.is_some(){next.display=st.display;}}}
            if let Some(v)=attr(a,b"style"){style(&mut next,v);}if let Some(v)=attr(a,b"id"){let n=v.len().min(32);next.id[..n].copy_from_slice(&v[..n]);next.id_len=n;}
            // aria-hidden affects accessibility, not visual display. An ancestor display:none cannot be overridden by a descendant.
            next.hidden=computed[depth-1].hidden||next.display==Some(false)||attr(a,b"hidden").is_some();
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
                let mut pic=next;pic.kind=2;if !pic.hidden{self.images+=1;if self.image_count<IMAGE_LIMIT{if let Some(l)=attr(a,b"src").or_else(||attr(a,b"data-src")).and_then(|v|url(base,v)){self.image_urls[self.image_count]=l;self.image_count+=1;pic.image=self.image_count as u8;}}self.text(&mut pic,b"[Image] ");if let Some(v)=attr(a,b"alt"){self.text(&mut pic,v);}self.push(&mut pic);}
            }
            if !void{
                tags[depth]=(tag.as_ptr() as usize-src.as_ptr() as usize,tag.len());parents[depth]=node;computed[depth]=next;depth+=1;b=next;active=b.link;
                if tag.eq_ignore_ascii_case(b"li"){self.text(&mut b,b"- ");}
            }else{b=computed[depth-1];active=b.link;}

        }
        self.push(&mut b);self.dom_nodes=dom.len.saturating_sub(1);self.truncated=dom.truncated||p<src.len();
        self.empty_document=self.count==0;
        if self.empty_document||self.requires_js{let mut notice=EMPTY;notice.kind=1;self.text(&mut notice,if self.requires_js{b"Website requires JavaScript".as_slice()}else{b"No visible content in this response".as_slice()});self.push(&mut notice);let mut detail=EMPTY;self.text(&mut detail,if self.requires_js{b"The server returned a JavaScript interstitial, not search results. Hi5WebCore does not execute general website scripts.".as_slice()}else{b"HTML received, but no supported visible content was found. Scripts are disabled.".as_slice()});self.push(&mut detail);}
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
    fn cols(b:&Block,width:usize)->usize{if matches!(b.kind,3|4){return TEXT;}(width.saturating_sub((b.padding as usize+b.margin as usize)*2)/if b.kind==1{14}else{9}).clamp(8,TEXT)}
    fn height(b:&Block)->usize{(if b.kind==1{30}else if matches!(b.kind,3|4){38}else{22})+2*(b.padding as usize+b.margin as usize)}
    pub fn control_hit(&self,width:usize,mut y:usize,floor:usize,scroll:usize,click:usize)->u8{let mut row=0;for b in &self.blocks[..self.count]{let n=if Self::photo(b).is_some(){1}else{b.len.div_ceil(Self::cols(b,width))};for _ in 0..n{if row>=scroll{let h=Self::photo(b).map_or(Self::height(b),|p|Self::image_size(p,width).1+8);if click>=y&&click<y+h&&click<floor{return if matches!(b.kind,3|4){b.kind}else{0};}y+=h;if y>=floor{return 0;}}row+=1;}}0}
    fn photo(b:&Block)->Option<&'static Photo>{if b.image==0{return None;}let p=unsafe{&(*core::ptr::addr_of!(PHOTOS))[b.image as usize-1]};(p.w>0&&p.h>0).then_some(p)}
    fn image_size(p:&Photo,width:usize)->(usize,usize){let w=p.w.min(width.saturating_sub(8));(w,(w*p.h/p.w).max(1))}
    pub fn rows(&self,width:usize)->usize{self.blocks[..self.count].iter().map(|b|if Self::photo(b).is_some(){1}else{b.len.div_ceil(Self::cols(b,width))}).sum()}
    pub fn draw(&self,c:&Canvas,x:usize,mut y:usize,right:usize,floor:usize,scroll:usize,ink:Color,sub:Color){
        let width=right.saturating_sub(x);if width<40{return;}let mut row=0;
        for b in &self.blocks[..self.count]{
            if let Some(p)=Self::photo(b){if row>=scroll{let(w,h)=Self::image_size(p,width);let h=h.min(floor.saturating_sub(y));let ox=if b.align==1{x+(width-w)/2}else if b.align==2{x+width-w}else{x+3};for j in 0..h{for i in 0..w{let at=((j*p.h/(w*p.h/p.w).max(1))*p.w+i*p.w/w)*3;c.pixel(ox+i,y+j,[p.bytes[at],p.bytes[at+1],p.bytes[at+2]]);}}y+=h+8;if y>=floor{return;}}row+=1;continue;}
            let big=b.kind==1;let height=Self::height(b);
            for part in b.buf[..b.len].chunks(Self::cols(b,width)){if row>=scroll{if y+height>floor{return;}if let Some(bg)=b.bg{c.rect(x,y,width,height,bg);}else if b.kind==2{c.rect(x,y,width,height,sub);}
                if let Ok(t)=core::str::from_utf8(part){let color=b.color.unwrap_or(if b.link.len>0{[31,97,185]}else{ink});let inset=b.padding as usize+b.margin as usize+3;let tw=c.text_width(t)*(if big{2}else{1});let tx=if b.align==1{x+width.saturating_sub(tw)/2}else if b.align==2{right.saturating_sub(tw+inset).max(x)}else{x+inset};if matches!(b.kind,3|4){let cw=if b.kind==3{width.min(420)}else{width.min(180)};let ox=if b.align==1{x+(width-cw)/2}else{x};c.round(ox+2,y+2,cw.saturating_sub(4),height-4,5,if b.kind==3&&self.form_focus{[207,224,247]}else{[227,231,237]});let val=if b.kind==3&&self.form.len>0{core::str::from_utf8(&self.form.value[..self.form.len]).unwrap_or("")}else{t};c.text_limit(ox+10,y+10,val,[30,34,40],false,ox+cw.saturating_sub(5));}else{c.text_limit(tx,y+inset,t,color,big,right);if b.link.len>0{c.rect(tx,y+height-3,c.text_width(t).min(right.saturating_sub(tx)),1,color);}}}y+=height;}row+=1;}
        }
    }
    pub fn hit(&self,width:usize,mut y:usize,floor:usize,scroll:usize,click:usize)->Option<Link>{let mut row=0;for b in &self.blocks[..self.count]{if let Some(p)=Self::photo(b){if row>=scroll{let(_,h)=Self::image_size(p,width);let next=y+h.min(floor.saturating_sub(y))+8;if click>=y&&click<next.min(floor){return (b.link.len>0).then_some(b.link);}y=next;}row+=1;continue;}let h=Self::height(b);for _ in b.buf[..b.len].chunks(Self::cols(b,width)){if row>=scroll{if y+h>floor{return None;}if click>=y&&click<y+h{return (b.link.len>0).then_some(b.link);}y+=h;}row+=1;}}None}

}
fn url(base:&[u8],href:&[u8])->Option<Link>{let mut decoded=[0u8;128];let(mut i,mut n)=(0,0);while i<href.len(){if n==decoded.len(){return None;}let mut consumed=1;let mut b=href[i];for ent in [b"&amp;".as_slice(),b"&#38;",b"&#x26;"]{if href[i..].starts_with(ent){b=b'&';consumed=ent.len();break;}}decoded[n]=b;n+=1;i+=consumed;}let mut l=EMPTY_LINK;l.len=crate::http::resolve_url(base,&decoded[..n],&mut l.url)?;if base.starts_with(b"https://")&&!l.url[..l.len].starts_with(b"https://"){return None;}Some(l)}
fn tag_end(s:&[u8])->Option<usize>{let mut q=0;for(i,b)in s.iter().enumerate().skip(1){if q!=0{if *b==q{q=0;}}else if *b==b'\''||*b==b'"'{q=*b;}else if *b==b'>'{return Some(i);}}None}
// Bounded compound and descendant selectors; unsupported syntax is rejected rather than partially matched.
fn ident(b:u8)->bool{b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')}
fn compound(s:&[u8],tag:&[u8],attrs:&[u8])->bool{
    let mut i=0;if s.first()==Some(&b'*'){i=1;}else if s.first().is_some_and(|b|ident(*b)){i=s.iter().position(|b|!ident(*b)).unwrap_or(s.len());if !tag.eq_ignore_ascii_case(&s[..i]){return false;}}
    while i<s.len(){let k=s[i];if !matches!(k,b'.'|b'#'){return false;}i+=1;let e=s[i..].iter().position(|b|!ident(*b)).map_or(s.len(),|n|i+n);if e==i{return false;}let v=&s[i..e];if k==b'.'{if !attr(attrs,b"class").is_some_and(|a|a.split(|b|b.is_ascii_whitespace()).any(|x|x==v)){return false;}}else if attr(attrs,b"id")!=Some(v){return false;}i=e;}!s.is_empty()
}
fn selector_spec(s:&[u8])->Option<u16>{
    let mut score=0u16;let mut parts=0;
    for part in s.split(|b|b.is_ascii_whitespace()).filter(|p|!p.is_empty()){
        parts+=1;if parts>8{return None;}let mut i=0;
        if part.first()==Some(&b'*'){i=1;}else if part.first().is_some_and(|b|ident(*b)){i=part.iter().position(|b|!ident(*b)).unwrap_or(part.len());score+=1;}
        while i<part.len(){let k=part[i];if !matches!(k,b'.'|b'#'){return None;}score+=if k==b'#'{256}else{16};i+=1;let e=part[i..].iter().position(|b|!ident(*b)).map_or(part.len(),|n|i+n);if e==i{return None;}i=e;}
    }(parts>0).then_some(score)
}
fn selector_match(s:&[u8],tag:&[u8],attrs:&[u8],src:&[u8],dom:&coreweb::Tree,mut parent:usize)->bool{
    let mut parts=s.rsplit(|b|b.is_ascii_whitespace()).filter(|p|!p.is_empty());
    let Some(last)=parts.next()else{return false;};if !compound(last,tag,attrs){return false;}
    for part in parts{let mut found=false;while parent>0&&parent<dom.len{let node=dom.nodes[parent];parent=node.parent as usize;
        let Some(raw)=src.get(node.start+1..node.start+node.len.saturating_sub(1))else{continue;};let raw=trim(raw);let n=raw.iter().position(|v|v.is_ascii_whitespace()||*v==b'/').unwrap_or(raw.len());
        if compound(part,&raw[..n],&raw[n..]){found=true;break;}}
        if !found{return false;}
    }true
}
fn css_rules(mut css:&[u8],out:&mut[Rule;256],count:&mut usize){
    while !css.is_empty()&&*count<out.len(){
        css=trim(css);if css.starts_with(b"/*"){let Some(e)=css[2..].windows(2).position(|v|v==b"*/")else{break;};css=&css[e+4..];continue;}
        let Some(i)=css.iter().position(|v|*v==b'{')else{break;};
        // Skip balanced nested rules (@media/@supports etc.) rather than treating inner selectors as global.
        let mut depth=1usize;let mut end=None;let mut quote=0u8;let mut escaped=false;
        for (j,&v) in css[i+1..].iter().enumerate(){if quote!=0{if escaped{escaped=false;}else if v==b'\\'{escaped=true;}else if v==quote{quote=0;}continue;}if matches!(v,b'\''|b'"'){quote=v;continue;}if v==b'{'{depth+=1;}else if v==b'}'{depth-=1;if depth==0{end=Some(i+1+j);break;}}}
        let Some(j)=end else{break;};let sel=trim(&css[..i]);let body=&css[i+1..j];css=&css[j+1..];if sel.starts_with(b"@"){continue;}
        let mut b=EMPTY;style(&mut b,body);if b.color.is_none()&&b.bg.is_none()&&b.kind==0&&!b.align_set&&b.padding==0&&b.margin==0&&b.display.is_none(){continue;}
        for s in sel.split(|v|*v==b','){let s=trim(s);if s.len()>96||*count>=out.len(){continue;}let Some(spec)=selector_spec(s)else{continue;};let mut r=EMPTY_RULE;r.selector[..s.len()].copy_from_slice(s);r.len=s.len();r.spec=spec;r.style=b;out[*count]=r;*count+=1;}
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
    p.parse_at(b"<h1 class='headline'>A</h1><p><a href='/news'>News</a></p>",b"https://example.test/",b".headline { color: #123456; }");let css_ok=p.styled==1&&p.blocks[0].color==Some([18,52,86])&&p.blocks[1].link.len>0&&p.hit(400,0,200,0,31).is_some_and(|l|&l.url[..l.len]==b"https://example.test/news");p.parse_at(b"<center><div style='color:#123456'><span>Nested</span></div><form action='/search'><input name='q'><input type='submit' value='Go'></form></center>",b"https://example.test/",&[]);let form_ok=p.dom_nodes>=7&&p.blocks[0].align==1&&p.blocks[0].color==Some([18,52,86])&&p.form.ready&&p.blocks[..p.count].iter().any(|v|v.kind==3)&&p.blocks[..p.count].iter().any(|v|v.kind==4);p.parse_at(b"<head><title>Search</title></head><body><style>div{display:none}.result{display:block}.result a.link{color:#123456} @media print {a{display:none}}</style><div class='result'><a class='link' href='/article?a=1&amp;b=2' aria-hidden='true'>Real result</a></div><div>Hidden</div></body>",b"https://example.test/",&[]);let result_ok=p.count==1&&p.blocks[0].color==Some([18,52,86])&&&p.blocks[0].buf[..p.blocks[0].len]==b"Real result"&&p.hit(400,0,200,0,10).is_some_and(|l|&l.url[..l.len]==b"https://example.test/article?a=1&b=2");
    p.parse(b"<html><head><title>Google Search</title></head><body><noscript><style>div{display:none}</style><div style='display:block'>Enable JavaScript <a href='/httpservice/retry/enablejs'>here</a></div></noscript><script>void(0)</script></body></html>");let interstitial_ok=p.requires_js&&p.count>=3&&!p.empty_document;
    p.parse(b"<html><body><div hidden>Not visible</div><script>void(0)</script></body></html>");let empty_ok=p.empty_document&&p.count==2;
    p.clear();ok&&css_ok&&form_ok&&result_ok&&interstitial_ok&&empty_ok&&coreweb::self_test()&&Bitmap::parse(b"BM").is_none()
}
