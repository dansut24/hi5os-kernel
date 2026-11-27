//! Native Hi5Office document format v1 and bounded OOXML export.
use alloc::vec::Vec;
use alloc::string::String;
use core::fmt::Write;
pub const MAX:usize=4096;
pub const BOLD:u8=1;pub const ITALIC:u8=2;pub const UNDER:u8=4;pub const LARGE:u8=8;
pub const NAMES:[&str;4]=["Document 1.h5doc","Document 2.h5doc","Document 3.h5doc","Document 4.h5doc"];
pub const DOCX_NAMES:[&str;4]=["Document 1.docx","Document 2.docx","Document 3.docx","Document 4.docx"];
pub const WORD_MAGIC:&[u8;8]=b"H5DOC001";
pub const SHEET_MAGIC:&[u8;8]=b"H5SHT001";
pub const SLIDE_MAGIC:&[u8;8]=b"H5SLD001";
#[derive(Clone)]struct Snapshot{chars:Vec<(u8,u8)>,cursor:usize,align:u8,anchor:Option<usize>,style:u8}
pub struct Word{chars:Vec<(u8,u8)>,pub cursor:usize,pub style:u8,pub align:u8,pub dirty:bool,pub anchor:Option<usize>,undo:Vec<Snapshot>,redo:Vec<Snapshot>}
impl Word{
    pub fn new()->Self{Self{chars:Vec::new(),cursor:0,style:0,align:0,dirty:false,anchor:None,undo:Vec::new(),redo:Vec::new()}}
    pub fn len(&self)->usize{self.chars.len()}
    pub fn at(&self,i:usize)->(u8,u8){self.chars[i]}
    fn snapshot(&self)->Snapshot{Snapshot{chars:self.chars.clone(),cursor:self.cursor,align:self.align,anchor:self.anchor,style:self.style}}
    fn restore(&mut self,s:Snapshot){self.chars=s.chars;self.cursor=s.cursor;self.align=s.align;self.anchor=s.anchor;self.style=s.style;self.dirty=true;}
    fn push_history(v:&mut Vec<Snapshot>,s:Snapshot){if v.len()>=32{v.remove(0);}v.push(s);}
    fn edit(&mut self){let s=self.snapshot();Self::push_history(&mut self.undo,s);self.redo.clear();self.dirty=true;}
    pub fn undo(&mut self){if let Some(s)=self.undo.pop(){let now=self.snapshot();Self::push_history(&mut self.redo,now);self.restore(s);}}
    pub fn redo(&mut self){if let Some(s)=self.redo.pop(){let now=self.snapshot();Self::push_history(&mut self.undo,now);self.restore(s);}}
    pub fn selection(&self)->Option<(usize,usize)>{let a=self.anchor?.min(self.len());let b=self.cursor.min(self.len());if a==b{None}else{Some((a.min(b),a.max(b)))}}
    pub fn move_to(&mut self,pos:usize,extend:bool){if extend{if self.anchor.is_none(){self.anchor=Some(self.cursor);}}else{self.anchor=None;}self.cursor=pos.min(self.len());}
    pub fn select_all(&mut self){self.anchor=Some(0);self.cursor=self.len();}
    pub fn copy(&self)->Option<Vec<(u8,u8)>>{let(a,b)=self.selection()?;Some(self.chars[a..b].to_vec())}
    fn remove_selection(&mut self){if let Some((a,b))=self.selection(){self.chars.drain(a..b);self.cursor=a;}self.anchor=None;}
    pub fn cut(&mut self)->Option<Vec<(u8,u8)>>{let data=self.copy()?;self.edit();self.remove_selection();Some(data)}
    pub fn paste(&mut self,data:&[(u8,u8)])->bool{let removed=self.selection().map_or(0,|(a,b)|b-a);if data.is_empty()||data.len()>MAX-self.len()+removed||data.iter().any(|&(b,s)|!(b==10||(32..127).contains(&b))||s&!15!=0){return false;}self.edit();self.remove_selection();self.chars.splice(self.cursor..self.cursor,data.iter().copied());self.cursor+=data.len();true}
    pub fn toggle_style(&mut self,mask:u8){if !matches!(mask,BOLD|ITALIC|UNDER|LARGE){return;}if let Some((a,b))=self.selection(){let clear=self.chars[a..b].iter().all(|v|v.1&mask!=0);self.edit();for v in &mut self.chars[a..b]{if clear{v.1&=!mask;}else{v.1|=mask;}}if clear{self.style&=!mask;}else{self.style|=mask;}}else{self.style^=mask;}}
    pub fn insert(&mut self,b:u8)->bool{self.paste(&[(b,self.style)])}
    pub fn backspace(&mut self){if self.selection().is_some(){self.edit();self.remove_selection();}else if self.cursor>0{self.edit();self.cursor-=1;self.chars.remove(self.cursor);self.anchor=None;}}
    pub fn delete(&mut self){if self.selection().is_some(){self.edit();self.remove_selection();}else if self.cursor<self.len(){self.edit();self.chars.remove(self.cursor);self.anchor=None;}}
    pub fn clear(&mut self){if self.len()>0{self.edit();self.chars.clear();self.cursor=0;self.anchor=None;}}
    pub fn alignment(&mut self,a:u8){if a<3&&a!=self.align{self.edit();self.align=a;}}
    pub fn word_count(&self)->usize{let mut n=0;let mut word=false;for &(b,_) in &self.chars{if b.is_ascii_whitespace(){word=false;}else if !word{n+=1;word=true;}}n}
    pub fn encode(&self)->Vec<u8>{let mut out=Vec::with_capacity(20+self.len()*2);out.extend_from_slice(WORD_MAGIC);out.extend_from_slice(&(self.len()as u32).to_le_bytes());out.push(self.align);out.extend_from_slice(&[0;3]);for &(b,s) in &self.chars{out.push(b);out.push(s);}let c=crc(&out);out.extend_from_slice(&c.to_le_bytes());out}
    pub fn decode(b:&[u8])->Option<Self>{if b.len()<20||&b[..8]!=WORD_MAGIC||b[12]>2||b[13..16]!=[0;3]{return None;}let n=u32::from_le_bytes(b[8..12].try_into().ok()?)as usize;if n>MAX||b.len()!=20+2*n||crc(&b[..b.len()-4])!=u32::from_le_bytes(b[b.len()-4..].try_into().ok()?){return None;}let mut w=Self::new();for p in b[16..16+2*n].chunks_exact(2){if !(p[0]==10||(32..127).contains(&p[0]))||p[1]&!15!=0{return None;}w.chars.push((p[0],p[1]));}w.align=b[12];w.cursor=n;Some(w)}
    pub fn text_import(b:&[u8])->Option<Self>{let mut w=Self::new();let mut cr=false;for &v in b{if v==13{if w.len()>=MAX{return None;}w.chars.push((10,0));cr=true;continue;}if v==10&&cr{cr=false;continue;}cr=false;let v=if v==9{b' '}else{v};if !(v==10||(32..127).contains(&v))||w.len()>=MAX{return None;}w.chars.push((v,0));}w.cursor=w.len();w.dirty=true;Some(w)}
    pub fn docx(&self)->Option<Vec<u8>>{
        let mut xml=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>");
        let a=["left","center","right"][self.align as usize];let mut i=0;
        loop{write!(xml,"<w:p><w:pPr><w:jc w:val=\"{}\"/></w:pPr>",a).ok()?;
            while i<self.len()&&self.at(i).0!=10{let s=self.at(i).1;xml.push_str("<w:r><w:rPr>");if s&BOLD!=0{xml.push_str("<w:b/>");}if s&ITALIC!=0{xml.push_str("<w:i/>");}if s&UNDER!=0{xml.push_str("<w:u w:val=\"single\"/>");}xml.push_str(if s&LARGE!=0{"<w:sz w:val=\"36\"/>"}else{"<w:sz w:val=\"24\"/>"});xml.push_str("</w:rPr><w:t xml:space=\"preserve\">");
                while i<self.len()&&self.at(i).0!=10&&self.at(i).1==s{match self.at(i).0{b'&'=>xml.push_str("&amp;"),b'<'=>xml.push_str("&lt;"),b'>'=>xml.push_str("&gt;"),v=>xml.push(v as char)}i+=1;}xml.push_str("</w:t></w:r>");if xml.len()>120*1024{return None;}
            }xml.push_str("</w:p>");if i==self.len(){break;}i+=1;
        }
        xml.push_str("<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>");
        let types=br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
        let rels=br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
        zip_store(&[(b"[Content_Types].xml",types),(b"_rels/.rels",rels),(b"word/document.xml",xml.as_bytes())])
    }
}
pub fn crc(b:&[u8])->u32{let mut c=!0u32;for v in b{c^=*v as u32;for _ in 0..8{c=(c>>1)^(0xedb88320u32&0u32.wrapping_sub(c&1));}}!c}
fn u16le(o:&mut Vec<u8>,v:u16){o.extend_from_slice(&v.to_le_bytes());}
fn u32le(o:&mut Vec<u8>,v:u32){o.extend_from_slice(&v.to_le_bytes());}
fn zip_store(files:&[(&[u8],&[u8])])->Option<Vec<u8>>{
    let mut out=Vec::new();let mut central=Vec::new();
    for &(name,data) in files{let start=out.len()as u32;let len=data.len()as u32;let sum=crc(data);
        u32le(&mut out,0x04034b50);for v in [20,0,0,0,0x21]{u16le(&mut out,v);}for v in [sum,len,len]{u32le(&mut out,v);}u16le(&mut out,name.len()as u16);u16le(&mut out,0);out.extend_from_slice(name);out.extend_from_slice(data);
        u32le(&mut central,0x02014b50);for v in [20,20,0,0,0,0x21]{u16le(&mut central,v);}for v in [sum,len,len]{u32le(&mut central,v);}for v in [name.len()as u16,0,0,0,0]{u16le(&mut central,v);}u32le(&mut central,0);u32le(&mut central,start);central.extend_from_slice(name);
    }
    let offset=out.len()as u32;let size=central.len()as u32;out.extend_from_slice(&central);u32le(&mut out,0x06054b50);for v in [0,0,files.len()as u16,files.len()as u16]{u16le(&mut out,v);}u32le(&mut out,size);u32le(&mut out,offset);u16le(&mut out,0);if out.len()>130560{None}else{Some(out)}
}
fn selection_test()->bool{let mut w=Word::new();for b in b"abc def"{w.insert(*b);}w.move_to(4,false);w.move_to(7,true);w.toggle_style(BOLD);let Some(v)=w.copy()else{return false;};if v!=[(b'd',1),(b'e',1),(b'f',1)]{return false;}if w.cut()!=Some(v.clone())||w.len()!=4{return false;}if !w.paste(&v)||w.len()!=7{return false;}w.undo();w.undo();if w.len()!=7||w.selection()!=Some((4,7)){return false;}w.redo();w.redo();w.select_all();w.insert(b'X');if w.len()!=1||w.at(0).0!=b'X'{return false;}w.undo();w.len()==7&&w.at(4)==(b'd',1)}
pub fn self_test()->bool{if !selection_test(){return false;}let mut w=Word::new();for b in b"Hi5 & <Office>"{if !w.insert(*b){return false;}}w.style=BOLD|UNDER;w.insert(b'!');w.alignment(1);w.undo();w.redo();w.cursor=1;w.insert(b'X');w.backspace();let b=w.encode();let Some(r)=Word::decode(&b)else{return false;};if r.encode()!=b||r.word_count()!=3||r.align!=1||r.at(r.len()-1).1!=BOLD|UNDER{return false;}let mut bad=b.clone();bad[16]^=1;if Word::decode(&bad).is_some(){return false;}let Some(z)=r.docx()else{return false;};z.starts_with(b"PK\x03\x04")&&z.windows(5).any(|v|v==b"&amp;")&&z.windows(6).any(|v|v==b"<w:b/>")&&Word::text_import(b"a\r\nb").is_some_and(|w|w.len()==3)}
