//! Hi5Sheet prototype: 4x4 cells, signed integers, SUM ranges, cycle/overflow detection.
//! In-memory only; no XLSX parser or GUI grid yet.
#[derive(Clone,Copy)]enum Cell{Empty,Value(i64),Sum(usize,usize)}
pub struct Sheet{cells:[Cell;16]}
fn index(s:&[u8])->Option<usize>{
    if s.len()!=2{return None;}let col=s[0].to_ascii_lowercase();let row=s[1];
    if !(b'a'..=b'd').contains(&col)||!(b'1'..=b'4').contains(&row){return None;}
    Some((row-b'1')as usize*4+(col-b'a')as usize)
}
impl Sheet{
    pub const fn new()->Self{Self{cells:[Cell::Empty;16]}}
    pub fn set(&mut self,cell:&[u8],value:&[u8])->bool{
        let Some(i)=index(cell)else{return false;};
        let v=if value.eq_ignore_ascii_case(b"empty"){Cell::Empty}
        else if value.len()>6&&value[..5].eq_ignore_ascii_case(b"=sum(")&&value.last()==Some(&b')'){
            let b=&value[5..value.len()-1];let mut p=b.split(|x|*x==b':');
            let Some(a)=p.next().and_then(index)else{return false;};let Some(z)=p.next().and_then(index)else{return false;};
            if p.next().is_some()||a/4>z/4||a%4>z%4{return false;}Cell::Sum(a,z)
        }else{let Some(v)=core::str::from_utf8(value).ok().and_then(|s|s.parse::<i64>().ok())else{return false;};Cell::Value(v)};
        let old=self.cells[i];self.cells[i]=v;
        // Reject formula cycles or overflow before committing a change to any dependency.
        if (0..16).any(|j|self.eval(j,0).is_none()){self.cells[i]=old;return false;}true
    }
    fn eval(&self,i:usize,seen:u16)->Option<i64>{
        let mask=1u16<<i;if seen&mask!=0{return None;}let seen=seen|mask;
        match self.cells[i]{Cell::Empty=>Some(0),Cell::Value(v)=>Some(v),Cell::Sum(a,z)=>{
            let mut sum=0i64;for row in a/4..=z/4{for col in a%4..=z%4{sum=sum.checked_add(self.eval(row*4+col,seen)?)?;}}Some(sum)
        }}
    }
    pub fn cell_name(i:usize)->[u8;2]{[b'a'+(i%4)as u8,b'1'+(i/4)as u8]}
    pub fn value(&self,i:usize)->Option<i64>{if i>=16||matches!(self.cells[i],Cell::Empty){None}else{self.eval(i,0)}}
    pub fn get(&self,cell:&[u8])->Option<i64>{self.eval(index(cell)?,0)}
    pub fn command(&mut self,cmd:&[u8])->Option<i64>{
        let mut p=cmd.split(|b|*b==b' ');let op=p.next()?;let cell=p.next()?;
        if op==b"set"{let value=p.next()?;if p.next().is_some()||!self.set(cell,value){return None;}self.get(cell)}
        else if op==b"get"&&p.next().is_none(){self.get(cell)}else{None}
    }
}
pub fn self_test()->bool{
    let mut s=Sheet::new();s.set(b"a1",b"12")&&s.set(b"a2",b"30")&&s.set(b"b1",b"=sum(a1:a2)")&&s.get(b"b1")==Some(42)&&
    s.set(b"a1",b"-2")&&s.get(b"b1")==Some(28)&&!s.set(b"a1",b"=sum(b1:b1)")&&s.get(b"a1")==Some(-2)&&
    !s.set(b"a2",b"-9223372036854775808")&&s.get(b"a2")==Some(30)&&s.command(b"get b1")==Some(28)
}
