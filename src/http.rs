//! Bounded HTTP body framing. Never treats an incomplete close-delimited response as complete.
use crate::net::MAX_BODY;
const HEADER_LIMIT:usize=16*1024;
pub fn status_code(b:&[u8])->Option<u16>{let l=b.split(|v|*v==10).next()?;if !(l.starts_with(b"HTTP/1.0 ")||l.starts_with(b"HTTP/1.1 "))||l.get(12).is_some_and(|v|!matches!(*v,b' '|b'\r')){return None;}let n=l.get(9..12)?;if !n.iter().all(u8::is_ascii_digit){return None;}let code=(n[0]-48)as u16*100+(n[1]-48)as u16*10+(n[2]-48)as u16;(100..600).contains(&code).then_some(code)}
#[derive(Clone,Copy)]enum Frame{Length(usize),Close,Chunked}
fn trim(mut b:&[u8])->&[u8]{while b.first().is_some_and(|c|*c==b' '||*c==b'\t'){b=&b[1..];}while b.last().is_some_and(|c|*c==b' '||*c==b'\t'){b=&b[..b.len()-1];}b}
fn number(b:&[u8],base:usize)->Option<usize>{
    if b.is_empty(){return None;}let mut n=0usize;
    for x in b{let d=match x{b'0'..=b'9'=>(*x-b'0')as usize,b'a'..=b'f' if base==16=>(*x-b'a'+10)as usize,b'A'..=b'F' if base==16=>(*x-b'A'+10)as usize,_=>return None};
        if d>=base{return None;}n=n.checked_mul(base)?.checked_add(d)?;if n>MAX_BODY{return None;}}
    Some(n)
}
fn header(b:&[u8])->Result<Option<(usize,Frame)>,&'static str>{
    let Some(i)=b.windows(4).position(|w|w==b"\r\n\r\n")else{if b.len()>HEADER_LIMIT{return Err("HTTP header too large");}return Ok(None);};
    if i>HEADER_LIMIT{return Err("HTTP header too large");}
    let mut lines=b[..i].split(|x|*x==10);let status=lines.next().ok_or("Missing HTTP status")?;
    let code=status_code(status).ok_or("Invalid HTTP status")?;
    if code<200||code==304{return Err("Unsupported HTTP status");}
    if code==204{return Ok(Some((i+4,Frame::Length(0))));}
    let mut length=None;let mut chunk=false;
    for line in lines{
        let l=line.strip_suffix(b"\r").unwrap_or(line);
        let col=l.iter().position(|x|*x==b':').ok_or("Invalid HTTP header")?;
        let key=&l[..col];let value=trim(&l[col+1..]);
        if key.eq_ignore_ascii_case(b"content-length"){if length.is_some(){return Err("Duplicate Content-Length");}length=Some(number(value,10).ok_or("Invalid length or over 4 MiB")?);}
        if key.eq_ignore_ascii_case(b"transfer-encoding"){if chunk||!value.eq_ignore_ascii_case(b"chunked"){return Err("Unsupported transfer encoding");}chunk=true;}
        if key.eq_ignore_ascii_case(b"content-encoding")&&!value.eq_ignore_ascii_case(b"identity"){return Err("Compressed response unsupported");}
    }
    if chunk&&length.is_some(){return Err("Conflicting HTTP body framing");}
    Ok(Some((i+4,if chunk{Frame::Chunked}else if let Some(n)=length{Frame::Length(n)}else{Frame::Close})))
}
// Returns total decoded length only once every chunk and the final trailer line are complete.
// This is a validation pass; the caller compacts the data only after success.
fn chunks(b:&[u8])->Result<Option<usize>,&'static str>{
    let mut p=0;let mut total=0;
    loop{
        let Some(e)=b[p..].windows(2).position(|w|w==b"\r\n")else{return if b.len()-p>128{Err("Chunk header too long")}else{Ok(None)};};
        if e>128{return Err("Chunk header too long");}
        let line=&b[p..p+e];let size=number(line.split(|x|*x==b';').next().unwrap_or(&[]),16).ok_or("Invalid chunk size")?;
        p+=e+2;
        if size==0{
            let mut trailers=0;
            loop{let Some(e)=b[p..].windows(2).position(|w|w==b"\r\n")else{return if b.len()-p>512{Err("Chunk trailer too long")}else{Ok(None)};};
                if e==0{if p+2!=b.len(){return Err("Unexpected trailing bytes");}return Ok(Some(total));}
                trailers+=e+2;if trailers>512||!b[p..p+e].contains(&b':'){return Err("Invalid chunk trailer");}p+=e+2;
            }
        }
        total+=size;if total>MAX_BODY{return Err("Body exceeds 4 MiB");}
        if p+size+2>b.len(){return Ok(None);}
        if &b[p+size..p+size+2]!=b"\r\n"{return Err("Invalid chunk terminator");}p+=size+2;
    }
}
pub fn complete(b:&mut[u8],used:usize,fin:bool)->Result<Option<(usize,u8)>,&'static str>{
    if used>b.len(){return Err("Response exceeds buffer");}
    let Some((start,fr))=header(&b[..used])?else{return if fin{Err("Incomplete HTTP header")}else{Ok(None)};};
    let count=used-start;
    match fr{
        Frame::Length(len)=>{if count>len{return Err("Unexpected trailing bytes");}if count<len{return if fin{Err("Incomplete HTTP body")}else{Ok(None)};}b.copy_within(start..used,0);Ok(Some((len,0)))},
        Frame::Close=>{if count>MAX_BODY{return Err("Body exceeds 4 MiB");}if !fin{return Ok(None);}b.copy_within(start..used,0);Ok(Some((count,1)))},
        Frame::Chunked=>{let Some(total)=chunks(&b[start..used])?else{return if fin{Err("Incomplete chunked body")}else{Ok(None)};};
            let mut p=start;let mut dst=0;
            loop{let e=b[p..used].windows(2).position(|w|w==b"\r\n").ok_or("Invalid chunk")?;
                let size=number(b[p..p+e].split(|x|*x==b';').next().unwrap_or(&[]),16).ok_or("Invalid chunk size")?;p+=e+2;if size==0{break;}
                b.copy_within(p..p+size,dst);dst+=size;p+=size+2;
            }Ok(Some((total,2)))
        }
    }
}
// Text-only HTML preview. No script execution, remote resources, CSS, forms or navigation.
// Strip tags and script/style/head regions; preserve simple block boundaries and basic entities.
pub fn preview(src:&[u8],out:&mut[u8])->usize{
    let mut i=0;let mut n=0;let mut skip=0u8;
    while i<src.len()&&n<out.len(){
        let c=src[i];
        if c==b'<'{
            let Some(e)=src[i..].iter().position(|b|*b==b'>')else{break;};
            let raw=trim(&src[i+1..i+e]);let close=raw.first()==Some(&b'/');let t=if close{trim(&raw[1..])}else{raw};
            let tag=t.split(|b|b.is_ascii_whitespace()||*b==b'/').next().unwrap_or(&[]);
            let group=if tag.eq_ignore_ascii_case(b"script"){1}else if tag.eq_ignore_ascii_case(b"style"){2}else if tag.eq_ignore_ascii_case(b"head"){3}else{0};
            if skip!=0{if close&&group==skip{skip=0;}}else if group!=0&&!close{skip=group;}else if [b"p".as_slice(),b"h1",b"h2",b"h3",b"br",b"div",b"li"].iter().any(|a|tag.eq_ignore_ascii_case(a)){if n>0&&out[n-1]!=b'\n'{out[n]=b'\n';n+=1;}}
            i+=e+1;continue;
        }
        if skip!=0{i+=1;continue;}
        if c==b'&'{let mut found=false;for (s,v) in [(b"&amp;".as_slice(),b'&'),(b"&lt;",b'<'),(b"&gt;",b'>'),(b"&quot;",b'"'),(b"&#39;",b'\''),(b"&nbsp;",b' ')]{if src[i..].starts_with(s){out[n]=v;n+=1;i+=s.len();found=true;break;}}if found{continue;}}
        if c.is_ascii_whitespace(){if n>0&&out[n-1]!=b' '&&out[n-1]!=b'\n'{out[n]=b' ';n+=1;}}
        else if (32..127).contains(&c){out[n]=c;n+=1;}i+=1;
    }n
}
pub fn self_test()->bool{
    fn test(src:&[u8],fin:bool,want:Result<Option<(usize,u8)>,&'static str>,payload:&[u8])->bool{
        let mut b=[0;512];b[..src.len()].copy_from_slice(src);let got=complete(&mut b,src.len(),fin);got==want&&(payload.is_empty()||&b[..payload.len()]==payload)
    }
    let c=b"HTTP/1.0 200 OK\r\n\r\nabc";
    let k=b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n1\r\nc\r\n0\r\n\r\n";
    let mut text=[0;64];let n=preview(b"<html><head><title>hidden</title></head><body><h1>Hello</h1><p>A &amp; B</p><script>bad()</script></body></html>",&mut text);
    test(b"HTTP/1.0 200 OK\r\nContent-Length: 3\r\n\r\nabc",false,Ok(Some((3,0))),b"abc")&&
    test(c,false,Ok(None),b"")&&test(c,true,Ok(Some((3,1))),b"abc")&&
    test(k,false,Ok(Some((3,2))),b"abc")&&test(&k[..k.len()-1],false,Ok(None),b"")&&
    test(&k[..k.len()-1],true,Err("Incomplete chunked body"),b"")&&
    test(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 2\r\n\r\n",false,Err("Conflicting HTTP body framing"),b"")&&
    &text[..n]==b"Hello\nA & B\n"
}

// Bounded navigation: HTTP only. Never reinterpret HTTPS as plaintext.
pub fn resolve_url(base:&[u8],href:&[u8],out:&mut[u8;128])->Option<usize>{
    let href=trim(href);if href.is_empty()||href[0]==b'#'||href.iter().any(|b|!(33..127).contains(b)||matches!(*b,b'<'|b'>'|b'"'|b'\''|b'\\')){return None;}
    let href=href.split(|b|*b==b'#').next()?;if href.is_empty(){return None;}
    let (scheme,origin)=if let Some(v)=base.strip_prefix(b"https://"){(b"https:".as_slice(),v)}else{(b"http:".as_slice(),base.strip_prefix(b"http://")?)};let end=origin.iter().position(|b|matches!(*b,b'/'|b'?')).unwrap_or(origin.len())+scheme.len()+2;
    let prefix=if href.starts_with(b"http://")||href.starts_with(b"https://"){b"".as_slice()}else if href.starts_with(b"//"){scheme}else if href.split(|b|*b==b'/').next()?.contains(&b':'){return None;}else if href[0]==b'/'{&base[..end]}else if href[0]==b'?'{base.split(|b|*b==b'?').next()?}else{let path=base.split(|b|*b==b'?').next()?;if path.len()==end{let n=base.len().checked_add(1)?.checked_add(href.len())?;if n>out.len(){return None;}out[..base.len()].copy_from_slice(base);out[base.len()]=b'/';out[base.len()+1..n].copy_from_slice(href);return Some(n);}let slash=path.iter().rposition(|b|*b==b'/')?;&path[..slash+1]};
    let n=prefix.len().checked_add(href.len())?;if n>out.len(){return None;}out[..prefix.len()].copy_from_slice(prefix);out[prefix.len()..n].copy_from_slice(href);Some(n)
}
pub fn redirect(b:&[u8])->Result<Option<&[u8]>,&'static str>{
    let Some(end)=b.windows(4).position(|w|w==b"\r\n\r\n")else{return Ok(None);};if end>HEADER_LIMIT{return Err("HTTP header too large");}
    let mut lines=b[..end].split(|x|*x==10);let status=lines.next().ok_or("Missing HTTP status")?;
    if ![b"301".as_slice(),b"302",b"303",b"307",b"308"].iter().any(|v|status.get(9..12)==Some(*v)){return Ok(None);}
    let mut loc=None;for line in lines{let Some(i)=line.iter().position(|b|*b==b':')else{return Err("Invalid redirect header");};if line[..i].eq_ignore_ascii_case(b"location"){if loc.is_some(){return Err("Duplicate redirect Location");}loc=Some(trim(line[i+1..].strip_suffix(b"\r").unwrap_or(&line[i+1..])));}}
    loc.map(Some).ok_or("Redirect missing Location")
}
#[derive(Clone,Copy)]pub struct Link{pub url:[u8;128],pub len:usize}
pub const EMPTY_LINK:Link=Link{url:[0;128],len:0};
pub fn links(src:&[u8],base:&[u8],out:&mut[Link;8])->usize{
    out.fill(EMPTY_LINK);let mut at=0;let mut n=0;let mut skip=0;
    while at<src.len()&&n<out.len(){let Some(rel)=src[at..].iter().position(|b|*b==b'<')else{break;};at+=rel+1;let Some(e)=src[at..].iter().position(|b|*b==b'>')else{break;};let raw=trim(&src[at..at+e]);at+=e+1;
        let name=raw.split(|b|b.is_ascii_whitespace()).next().unwrap_or(&[]);if name.eq_ignore_ascii_case(b"script"){skip=1;continue;}if name.eq_ignore_ascii_case(b"style"){skip=2;continue;}if skip!=0{if skip==1&&name.eq_ignore_ascii_case(b"/script")||skip==2&&name.eq_ignore_ascii_case(b"/style"){skip=0;}continue;}if !name.eq_ignore_ascii_case(b"a"){continue;}
        let mut attrs=&raw[name.len()..];while !attrs.is_empty(){attrs=trim(attrs);if attrs.is_empty(){break;}let k=attrs.iter().position(|b|b.is_ascii_whitespace()||*b==b'=').unwrap_or(attrs.len());if k==0{break;}let key=&attrs[..k];attrs=trim(&attrs[k..]);if attrs.first()!=Some(&b'='){continue;}attrs=trim(&attrs[1..]);if attrs.is_empty(){break;}let q=attrs[0];let (value,rest)=if q==b'"'||q==b'\''{let Some(e)=attrs[1..].iter().position(|b|*b==q)else{break;};(&attrs[1..1+e],&attrs[2+e..])}else{let e=attrs.iter().position(|b|b.is_ascii_whitespace()).unwrap_or(attrs.len());(&attrs[..e],&attrs[e..])};attrs=rest;if key.eq_ignore_ascii_case(b"href"){let mut url=[0;128];if let Some(len)=resolve_url(base,value,&mut url){if !out[..n].iter().any(|v|v.url[..v.len]==url[..len]){out[n]=Link{url,len};n+=1;}}break;}
        }
    }n
}
pub fn navigation_test()->bool{let mut a=[0;128];let base=b"http://example.test/dir/index.html";let mut l=[EMPTY_LINK;8];resolve_url(base,b"/setup.exe",&mut a).is_some_and(|n|&a[..n]==b"http://example.test/setup.exe")&&resolve_url(base,b"https://example.test/setup.exe",&mut a).is_some_and(|n|&a[..n]==b"https://example.test/setup.exe")&&resolve_url(b"https://example.test/a",b"//example.test/b",&mut a).is_some_and(|n|&a[..n]==b"https://example.test/b")&&resolve_url(base,b"other.msi",&mut a).is_some_and(|n|&a[..n]==b"http://example.test/dir/other.msi")&&redirect(b"HTTP/1.1 302 Found\r\nLocation: /setup.exe\r\n\r\n")==Ok(Some(b"/setup.exe".as_slice()))&&links(b"<a href='/setup.exe'>EXE</a><a href=\"other.msi\">MSI</a><script><a href='/bad'></script>",base,&mut l)==2}
