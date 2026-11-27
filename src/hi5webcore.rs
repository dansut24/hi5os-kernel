//! Hi5WebCore: bounded native DOM and explicitly submitted HTML GET forms.
//! No arbitrary scripts, background requests, credential fields or process-isolation claims.
use crate::http::{Link,EMPTY_LINK};
pub const NODE_LIMIT:usize=8192;
pub const DEPTH:usize=64;
#[derive(Clone,Copy)]pub struct Node{pub parent:u16,pub first:u16,pub last:u16,pub next:u16,pub start:usize,pub len:usize,pub text:bool}
const NODE:Node=Node{parent:0,first:0,last:0,next:0,start:0,len:0,text:false};
pub struct Tree{pub nodes:[Node;NODE_LIMIT],pub len:usize,pub truncated:bool}
pub static mut DOM:Tree=Tree::new();
impl Tree{
    pub const fn new()->Self{Self{nodes:[NODE;NODE_LIMIT],len:1,truncated:false}}
    pub fn clear(&mut self){self.len=1;self.nodes[0]=NODE;self.truncated=false;}
    pub fn append(&mut self,parent:usize,start:usize,len:usize,text:bool)->Option<usize>{
        if parent>=self.len||self.len>=NODE_LIMIT{self.truncated=true;return None;}
        let i=self.len;self.len+=1;self.nodes[i]=Node{parent:parent as u16,start,len,text,..NODE};
        let old=self.nodes[parent].last as usize;if old==0{self.nodes[parent].first=i as u16;}else{self.nodes[old].next=i as u16;}self.nodes[parent].last=i as u16;Some(i)
    }
}
#[derive(Clone,Copy)]pub struct Form{pub action:Link,pub name:[u8;48],pub name_len:usize,pub value:[u8;96],pub len:usize,pub ready:bool}
impl Form{
    pub const fn new()->Self{Self{action:EMPTY_LINK,name:[0;48],name_len:0,value:[0;96],len:0,ready:false}}
    pub fn input(&mut self,b:u8){if self.len<self.value.len()&&(32..127).contains(&b){self.value[self.len]=b;self.len+=1;}}
    pub fn backspace(&mut self){if self.len>0{self.len-=1;self.value[self.len]=0;}}
    pub fn target(&self)->Option<Link>{if !self.ready||self.action.len==0||self.name_len==0{return None;}
        let mut out=EMPTY_LINK;let base=self.action.url[..self.action.len].split(|b|*b==b'?').next()?;
        if base.len()+2>128{return None;}out.url[..base.len()].copy_from_slice(base);out.len=base.len();
        append(&mut out,b'?')?;encode(&mut out,&self.name[..self.name_len])?;append(&mut out,b'=')?;encode(&mut out,&self.value[..self.len])?;Some(out)
    }
}
fn append(l:&mut Link,b:u8)->Option<()>{if l.len>=l.url.len(){return None;}l.url[l.len]=b;l.len+=1;Some(())}
fn encode(l:&mut Link,s:&[u8])->Option<()>{const HEX:&[u8;16]=b"0123456789ABCDEF";for b in s{if b.is_ascii_alphanumeric()||matches!(*b,b'-'|b'_'|b'.'|b'*'){append(l,*b)?;}else if *b==b' '{append(l,b'+')?;}else{append(l,b'%')?;append(l,HEX[(b>>4)as usize])?;append(l,HEX[(b&15)as usize])?;}}Some(())}
fn origin(s:&[u8])->Option<&[u8]>{let n=if s.starts_with(b"https://"){8}else if s.starts_with(b"http://"){7}else{return None;};let e=s[n..].iter().position(|b|matches!(*b,b'/'|b'?'|b'#')).map_or(s.len(),|i|i+n);Some(&s[..e])}
pub fn same_origin(a:&[u8],b:&[u8])->bool{matches!((origin(a),origin(b)),(Some(x),Some(y)) if x.eq_ignore_ascii_case(y))}
pub fn void_tag(t:&[u8])->bool{[b"area".as_slice(),b"base",b"br",b"col",b"embed",b"hr",b"img",b"input",b"link",b"meta",b"param",b"source",b"track",b"wbr"].iter().any(|v|t.eq_ignore_ascii_case(v))}
pub fn self_test()->bool{let d=unsafe{&mut *core::ptr::addr_of_mut!(DOM)};d.clear();let a=d.append(0,1,2,false).unwrap_or(0);let b=d.append(a,3,4,true).unwrap_or(0);let c=d.append(a,7,2,false).unwrap_or(0);let tree=a==1&&b==2&&c==3&&d.nodes[a].first==2&&d.nodes[a].last==3&&d.nodes[b].parent==1&&d.nodes[b].next==3;d.clear();
    let mut f=Form::new();let u=b"https://example.test/search";f.action.url[..u.len()].copy_from_slice(u);f.action.len=u.len();f.name[0]=b'q';f.name_len=1;f.ready=true;for b in b"a b&c"{f.input(*b);}tree&&f.target().is_some_and(|l|&l.url[..l.len]==b"https://example.test/search?q=a+b%26c")&&same_origin(b"https://example.test/a",u)&&!same_origin(b"https://example.test",b"http://example.test/") }
