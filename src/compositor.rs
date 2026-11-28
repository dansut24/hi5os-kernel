//! Hi5Compositor retained software surfaces for interactive window movement.
//! All buffers are bounded and optional; allocation failure retains immediate rendering.
use alloc::vec::Vec;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Area{pub x:usize,pub y:usize,pub w:usize,pub h:usize}
impl Area{
    pub fn clipped(self,w:usize,h:usize)->Self{let x=self.x.min(w);let y=self.y.min(h);Self{x,y,w:self.w.min(w-x),h:self.h.min(h-y)}}
    pub fn union(self,b:Self)->Self{let x=self.x.min(b.x);let y=self.y.min(b.y);Self{x,y,w:self.x.saturating_add(self.w).max(b.x.saturating_add(b.w))-x,h:self.y.saturating_add(self.h).max(b.y.saturating_add(b.h))-y}}
    pub fn shadow(self)->Self{Self{w:self.w.saturating_add(12),h:self.h.saturating_add(12),..self}}
}
pub struct Surface{pub w:usize,pub h:usize,pixels:Vec<u32>}
impl Surface{
    pub fn new(w:usize,h:usize)->Option<Self>{let n=w.checked_mul(h)?;if n==0||n>2048*1536{return None;}let mut pixels=Vec::new();pixels.try_reserve_exact(n).ok()?;pixels.resize(n,0);Some(Self{w,h,pixels})}
    pub fn row(&self,y:usize)->&[u32]{&self.pixels[y*self.w..(y+1)*self.w]}
    pub fn row_mut(&mut self,y:usize)->&mut[u32]{&mut self.pixels[y*self.w..(y+1)*self.w]}
}
pub fn inside_round(x:usize,y:usize,w:usize,h:usize,r:usize)->bool{
    if x>=w||y>=h{return false;}let r=r.min(w/2).min(h/2);
    let dx=r.saturating_sub(x.min(w-1-x));let dy=r.saturating_sub(y.min(h-1-y));
    dx*dx+dy*dy<=r*r
}
pub fn span(y:usize,w:usize,h:usize,r:usize)->(usize,usize){
    if y>=h||w==0{return (0,0);}let r=r.min(w/2).min(h/2);let mut start=0;
    while start<r&&!inside_round(start,y,w,h,r){start+=1;}
    (start,w-start)
}
pub struct DragLayer{pub backdrop:Surface,pub window:Surface,pub frames:u64,pub pixels:u64,pub tick:u64}
impl DragLayer{
    // 100 Hz PIT => 2 ticks, at most 50 interactive presents per second.
    pub fn due(&self,now:u64)->bool{now.saturating_sub(self.tick)>=2}
}
pub fn self_test()->bool{
    let a=Area{x:10,y:20,w:80,h:50};let b=Area{x:40,y:10,w:90,h:50};
    if a.union(b)!= (Area{x:10,y:10,w:120,h:60})||a.shadow().clipped(60,50)!=(Area{x:10,y:20,w:50,h:30})||inside_round(0,0,80,50,14)||!inside_round(40,0,80,50,14)||inside_round(80,0,80,50,14){return false;}
    if span(0,80,50,14)!=(14,66)||span(25,80,50,14)!=(0,80){return false;}
    let Some(mut s)=Surface::new(4,3)else{return false;};s.row_mut(2)[3]=0x123456;
    s.row(2)[3]==0x123456&&s.row(0)[0]==0&&Surface::new(usize::MAX,2).is_none()&&Surface::new(0,3).is_none()
}
