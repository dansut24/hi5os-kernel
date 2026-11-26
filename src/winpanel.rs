//! Limited native Win32 panel controls. Guest-supplied text and rows only.
pub const LIST:u64=u64::MAX-1;pub const COMBO:u64=u64::MAX-2;pub const EDIT:u64=u64::MAX-3;pub const STATUS:u64=u64::MAX-4;pub const REBAR:u64=u64::MAX-5;
pub fn class(name:&[u8])->Option<u64>{for (n,p) in [(b"SysListView32".as_slice(),LIST),(b"ComboBoxEx32".as_slice(),COMBO),(b"ComboBox".as_slice(),COMBO),(b"Edit".as_slice(),EDIT),(b"msctls_statusbar32".as_slice(),STATUS),(b"ReBarWindow32".as_slice(),REBAR)]{if name.eq_ignore_ascii_case(n){return Some(p);}}None}
pub fn native(p:u64)->bool{matches!(p,LIST|COMBO|EDIT|STATUS|REBAR)}
#[derive(Clone,Copy)]pub struct Text{pub len:usize,pub buf:[u16;128]}
pub const TEXT:Text=Text{len:0,buf:[0;128]};
#[derive(Clone,Copy)]pub struct Column{pub text:Text,pub width:i32}
pub const COL:Column=Column{text:TEXT,width:100};
#[derive(Clone,Copy)]pub struct Row{pub cells:[Text;6],pub state:u32,pub image:i32,pub data:u64,pub callback:u8}
pub const ROW:Row=Row{cells:[TEXT;6],state:0,image:-1,data:0,callback:0};
#[derive(Clone,Copy)]struct Control{h:u64,kind:u64,unicode:bool,ex:u32,images:[u64;3],text:Text,edit:u64,columns:[Column;6],cols:usize,rows:[Row;32],len:usize,parts:[i32;8],part_text:[Text;8],nparts:usize,redraw:bool,order:[usize;6],bands:[u64;4],band_count:usize}
const EMPTY:Control=Control{h:0,kind:0,unicode:false,ex:0,images:[0;3],text:TEXT,edit:0,columns:[COL;6],cols:0,rows:[ROW;32],len:0,parts:[0;8],part_text:[TEXT;8],nparts:0,redraw:true,order:[0,1,2,3,4,5],bands:[0;4],band_count:0};
pub struct Panels{slots:[Control;8]}
impl Panels{
    pub const fn new()->Self{Self{slots:[EMPTY;8]}}
    pub fn reset(&mut self){for c in &mut self.slots{c.h=0;c.len=0;c.cols=0;c.nparts=0;}}
    fn idx(&self,h:u64)->Option<usize>{self.slots.iter().position(|c|c.h==h&&h!=0)}
    pub fn kind(&self,h:u64)->Option<u64>{Some(self.slots[self.idx(h)?].kind)}
    pub fn create(&mut self,h:u64,kind:u64)->bool{if !native(kind)||h==0||self.idx(h).is_some(){return false;}let Some(i)=self.slots.iter().position(|c|c.h==0)else{return false;};let c=&mut self.slots[i];c.h=h;c.kind=kind;c.unicode=false;c.ex=0;c.images=[0;3];c.text=TEXT;c.edit=0;c.cols=0;c.len=0;c.nparts=0;c.redraw=true;c.order=[0,1,2,3,4,5];c.band_count=0;c.bands=[0;4];true}
    pub fn set_edit(&mut self,h:u64,e:u64)->bool{let Some(i)=self.idx(h)else{return false;};self.slots[i].edit=e;true}
    pub fn edit(&self,h:u64)->Option<u64>{Some(self.slots[self.idx(h)?].edit)}
    pub fn unicode(&mut self,h:u64,set:Option<bool>)->Option<bool>{let c=&mut self.slots[self.idx(h)?];let old=c.unicode;if let Some(v)=set{c.unicode=v;}Some(old)}
    pub fn ex(&mut self,h:u64,mask:u32,value:Option<u32>)->Option<u32>{let i=self.idx(h)?;let c=&mut self.slots[i];let old=c.ex;if let Some(v)=value{c.ex=if mask==0{v}else{(old&!mask)|(v&mask)};}Some(old)}
    pub fn image(&mut self,h:u64,slot:usize,value:Option<u64>)->Option<u64>{let i=self.idx(h)?;let v=self.slots[i].images.get_mut(slot)?;let old=*v;if let Some(x)=value{*v=x;}Some(old)}
    pub fn text(&mut self,h:u64,value:Option<Text>)->Option<Text>{let i=self.idx(h)?;let c=&mut self.slots[i];let old=c.text;if let Some(v)=value{c.text=v;}Some(old)}
    pub fn insert_col(&mut self,h:u64,i:usize,v:Column)->Option<usize>{let k=self.idx(h)?;let c=&mut self.slots[k];if i>c.cols||c.cols==6||v.width<0||v.width>16384{return None;}c.columns.copy_within(i..c.cols,i+1);c.columns[i]=v;c.cols+=1;Some(i)}
    pub fn col(&self,h:u64,i:usize)->Option<Column>{let c=&self.slots[self.idx(h)?];if i>=c.cols{return None;}Some(c.columns[i])}
    pub fn delete_col(&mut self,h:u64,i:usize)->bool{let Some(k)=self.idx(h)else{return false;};let c=&mut self.slots[k];if i>=c.cols{return false;}c.columns.copy_within(i+1..c.cols,i);c.cols-=1;true}
    pub fn set_col(&mut self,h:u64,i:usize,v:Column)->bool{let Some(k)=self.idx(h)else{return false;};if i>=self.slots[k].cols||v.width<0||v.width>16384{return false;}self.slots[k].columns[i]=v;true}
    pub fn order(&mut self,h:u64,v:Option<&[usize]>)->Option<[usize;6]>{let k=self.idx(h)?;let c=&mut self.slots[k];let old=c.order;if let Some(v)=v{if v.len()!=c.cols||v.iter().enumerate().any(|(i,x)|*x>=v.len()||v[..i].contains(x)){return None;}c.order[..v.len()].copy_from_slice(v);}Some(old)}
    pub fn redraw(&mut self,h:u64,v:bool)->bool{let Some(i)=self.idx(h)else{return false;};self.slots[i].redraw=v;true}
    pub fn swap(&mut self,h:u64,a:usize,b:usize)->bool{let Some(i)=self.idx(h)else{return false;};let c=&mut self.slots[i];if a>=c.len||b>=c.len{return false;}c.rows.swap(a,b);true}
    pub fn add_band(&mut self,h:u64,child:u64)->bool{let Some(i)=self.idx(h)else{return false;};let c=&mut self.slots[i];if child==0||c.band_count==4||c.bands[..c.band_count].contains(&child){return false;}c.bands[c.band_count]=child;c.band_count+=1;true}
    pub fn bands(&self,h:u64)->usize{self.idx(h).map_or(0,|i|self.slots[i].band_count)}
    pub fn cols(&self,h:u64)->usize{self.idx(h).map_or(0,|i|self.slots[i].cols)}
    pub fn count(&self,h:u64)->usize{self.idx(h).map_or(0,|i|self.slots[i].len)}
    pub fn insert_row(&mut self,h:u64,i:usize,v:Row)->Option<usize>{let k=self.idx(h)?;let c=&mut self.slots[k];if c.len==32{return None;}let i=i.min(c.len);c.rows.copy_within(i..c.len,i+1);c.rows[i]=v;c.len+=1;Some(i)}
    pub fn row(&self,h:u64,i:usize)->Option<Row>{let c=&self.slots[self.idx(h)?];if i>=c.len{return None;}Some(c.rows[i])}
    pub fn set_row(&mut self,h:u64,i:usize,v:Row)->bool{let Some(k)=self.idx(h)else{return false;};let c=&mut self.slots[k];if i>=c.len{return false;}c.rows[i]=v;true}
    pub fn selected(&self,h:u64)->Option<usize>{let c=&self.slots[self.idx(h)?];c.rows[..c.len].iter().position(|r|r.state&2!=0)}
    pub fn select(&mut self,h:u64,i:usize)->bool{let Some(k)=self.idx(h)else{return false;};let c=&mut self.slots[k];if i>=c.len{return false;}for (j,r) in c.rows[..c.len].iter_mut().enumerate(){r.state=(r.state&!3)|if j==i{3}else{0};}true}
    pub fn clear(&mut self,h:u64)->bool{let Some(k)=self.idx(h)else{return false;};self.slots[k].len=0;true}
    pub fn delete_row(&mut self,h:u64,i:usize)->bool{let Some(k)=self.idx(h)else{return false;};let c=&mut self.slots[k];if i>=c.len{return false;}c.rows.copy_within(i+1..c.len,i);c.len-=1;true}
    pub fn parts(&mut self,h:u64,parts:&[i32])->bool{let Some(k)=self.idx(h)else{return false;};if parts.len()>8{return false;}let c=&mut self.slots[k];c.parts[..parts.len()].copy_from_slice(parts);c.nparts=parts.len();c.part_text=[TEXT;8];true}
    pub fn part_count(&self,h:u64)->usize{self.idx(h).map_or(0,|i|self.slots[i].nparts)}
    pub fn part_edge(&self,h:u64,i:usize)->Option<i32>{let c=&self.slots[self.idx(h)?];if i>=c.nparts{return None;}Some(c.parts[i])}
    pub fn part(&mut self,h:u64,i:usize,v:Option<Text>)->Option<Text>{let k=self.idx(h)?;let c=&mut self.slots[k];if i>=c.nparts{return None;}let old=c.part_text[i];if let Some(v)=v{c.part_text[i]=v;}Some(old)}
}
pub fn test(p:&mut Panels)->bool{
    p.reset();if !p.create(1,LIST)||p.create(1,EDIT)||p.unicode(1,Some(true))!=Some(false)||p.unicode(1,None)!=Some(true)||p.ex(1,0,Some(3))!=Some(0)||p.ex(1,1,Some(0))!=Some(3){return false;}
    let mut text=TEXT;text.buf[0]=65;text.len=1;let mut row=ROW;row.cells[0]=text;row.data=42;
    let ok=p.insert_col(1,0,Column{text,width:120})==Some(0)&&p.insert_row(1,0,row)==Some(0)&&p.row(1,0).is_some_and(|r|r.data==42&&r.cells[0].buf[0]==65)&&p.insert_row(1,1,Row{data:99,..ROW})==Some(1)&&p.select(1,1)&&p.selected(1)==Some(1)&&p.swap(1,0,1)&&p.selected(1)==Some(0)&&p.row(1,0).is_some_and(|r|r.data==99)&&p.delete_row(1,0)&&p.delete_row(1,0)&&p.count(1)==0&&p.delete_col(1,0)&&p.cols(1)==0&&p.create(2,STATUS)&&p.parts(2,&[80,-1])&&p.part_count(2)==2&&p.part_edge(2,0)==Some(80)&&p.part_edge(2,1)==Some(-1)&&p.part_edge(2,2).is_none()&&p.part(2,1,Some(text)).is_some()&&p.part(2,1,None).is_some_and(|v|v.len==1);p.reset();ok
}
