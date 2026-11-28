//! Hi5UI software rasteriser. Native RGB framebuffer; bounded back buffer.
use crate::boot::Framebuffer;
use core::ptr;
pub type Color=[u8;3];
pub const ACCENT:Color=[26,102,188];
pub const TEAL:Color=[8,199,206];
pub const MIDNIGHT:Color=[8,20,35];
#[derive(Clone,Copy)]pub struct Palette{pub wall:Color,pub wall2:Color,pub bar:Color,pub panel:Color,pub sub:Color,pub ink:Color,pub muted:Color,pub edge:Color,pub green:Color}
pub fn palette(dark:bool)->Palette{if dark{Palette{wall:MIDNIGHT,wall2:[16,40,61],bar:[12,29,46],panel:[19,39,57],sub:[32,59,80],ink:[243,247,251],muted:[177,200,216],edge:[54,80,100],green:[111,226,180]}}else{Palette{wall:[229,239,248],wall2:[188,217,233],bar:[243,247,251],panel:[252,253,255],sub:[228,238,245],ink:[16,40,61],muted:[61,85,106],edge:[188,207,221],green:[17,115,77]}}}
fn blend(a:Color,b:Color,n:usize,d:usize)->Color{let n=n.min(d);core::array::from_fn(|i|((a[i]as usize*(d-n)+b[i]as usize*n)/d.max(1))as u8)}
fn aurora_color(x:usize,y:usize,dark:bool)->Color{
    // Landscape in a 1024x1024 normalised coordinate space. Smooth aurora arcs,
    // layered ridgelines, a reflective lake and atmospheric perspective.
    let triangle=|v:usize,period:usize|{let t=v%period;t.min(period-t)};
    let horizon=708usize;
    let yy=if y>horizon{horizon.saturating_sub((y-horizon)*2/3)}else{y};
    let sky=blend([6,15,38],[19,69,107],yy.min(horizon),horizon);
    let smooth=|v:usize,period:usize|{let t=triangle(v,period)*1024/(period/2);t*t*(3072-2*t)/1048576};
    let wave=150+smooth(x+100,1024)*290/1024+smooth(x+31,510)*40/1024;
    let ribbon=110usize.saturating_sub(yy.abs_diff(wave));
    let wave2=90+smooth(x+450,1024)*230/1024;
    let ribbon2=78usize.saturating_sub(yy.abs_diff(wave2));
    let glow=blend(sky,[17,209,190],ribbon*ribbon/110,140);
    let glow=blend(glow,[82,77,214],ribbon2*ribbon2/78,105);
    let ridge1=570usize.saturating_sub(triangle(x+60,370)*2/5+triangle(x+20,127)/3);
    let ridge2=650usize.saturating_sub(triangle(x+174,480)*3/5+triangle(x+17,91)/4);
    let ridge3=708usize.saturating_sub(triangle(x+320,620)*2/3+triangle(x+19,75)/4);
    let mut c=glow;
    if yy>=ridge1{c=blend([37,79,119],[20,49,83],yy-ridge1,220);}
    if yy>=ridge2{c=blend([29,60,92],[12,32,57],yy-ridge2,150);}
    if yy>=ridge3{c=blend([21,42,68],[5,19,35],yy-ridge3,120);}
    if y>horizon{c=blend(c,[4,25,47],(y-horizon)/2+20,190);let ripple=triangle(x*3+y*13,43);if ripple<2{c=blend(c,[61,129,152],12,100);}}
    if dark{c}else{blend(c,[196,226,242],24,100)}

}
pub fn self_test()->bool{
    let contrast=|a:Color,b:Color|{let l=|c:Color|(299*c[0]as i32+587*c[1]as i32+114*c[2]as i32)/1000;(l(a)-l(b)).abs()>105};
    [false,true].iter().all(|&d|{let p=palette(d);contrast(p.ink,p.panel)&&contrast(p.muted,p.sub)&&aurora_color(30,550,d)!=aurora_color(510,550,d)})&&blend([0;3],[100;3],50,100)==[50;3]
}

const ART:&[u8;960*540*2]=include_bytes!("generated/aurora565.bin");
const MAX:usize=2048*1536;
#[repr(align(4096))]
struct Buffer([u32;MAX]);
static mut BACK:Buffer=Buffer([0;MAX]);
static REG:&[u8;97280]=include_bytes!("generated/regular.bin");
static BIG:&[u8;97280]=include_bytes!("generated/large.bin");
static REG_ADV:&[u8;95]=include_bytes!("generated/regular-adv.bin");
static BIG_ADV:&[u8;95]=include_bytes!("generated/large-adv.bin");
pub struct Canvas{pub fb:Framebuffer,pub physical:Framebuffer,pub scale:usize,pub buffered:bool}
impl Canvas{
    pub fn new(fb:Framebuffer)->Self{Self::scaled(fb,100)}
    pub fn supported(fb:Framebuffer,scale:usize)->bool{matches!(scale,100|125|150|175|200)&&fb.width*100/scale>=640&&fb.height*100/scale>=480&&fb.width.checked_mul(fb.height).is_some_and(|n|n<=MAX)}
    pub fn scaled(physical:Framebuffer,scale:usize)->Self{let scale=if Self::supported(physical,scale){scale}else{100};let mut fb=physical;fb.width=physical.width*100/scale;fb.height=physical.height*100/scale;Self{fb,physical,scale,buffered:fb.width.checked_mul(fb.height).is_some_and(|n|n<=MAX)}}
    pub fn restore_pixel(&self,x:usize,y:usize){if self.buffered&&x<self.fb.width&&y<self.fb.height{let v=unsafe{ptr::read(self.target(x,y))};self.front_value(x,y,v);}}
    fn front_value(&self,x:usize,y:usize,v:u32){let p=self.physical;let x0=x*p.width/self.fb.width;let x1=(x+1)*p.width/self.fb.width;let y0=y*p.height/self.fb.height;let y1=(y+1)*p.height/self.fb.height;unsafe{for yy in y0..y1{for xx in x0..x1{ptr::write_volatile((p.base as *mut u32).add(yy*p.stride+xx),v);}}}}
    pub fn text_width(&self,t:&str)->usize{Self::measure_text(t.as_bytes(),false)}
    pub fn measure_text(t:&[u8],big:bool)->usize{let adv=if big{BIG_ADV}else{REG_ADV};t.iter().copied().map(|b|adv[if (32..127).contains(&b){(b-32)as usize}else{31}]as usize).sum()}
    fn pack(&self,c:Color)->u32{if self.fb.format==0{u32::from_le_bytes([c[0],c[1],c[2],0])}else{u32::from_le_bytes([c[2],c[1],c[0],0])}}
    fn unpack(&self,v:u32)->Color{let b=v.to_le_bytes();if self.fb.format==0{[b[0],b[1],b[2]]}else{[b[2],b[1],b[0]]}}
    fn target(&self,x:usize,y:usize)->*mut u32{unsafe{if self.buffered{ptr::addr_of_mut!(BACK.0).cast::<u32>().add(y*self.fb.width+x)}else{(self.fb.base as *mut u32).add(y*self.fb.stride+x)}}}
    fn set(&self,x:usize,y:usize,v:u32){if x>=self.fb.width||y>=self.fb.height{return;}unsafe{if self.buffered{ptr::write(self.target(x,y),v);}else{ptr::write_volatile(self.target(x,y),v);}}}
    pub fn pixel(&self,x:usize,y:usize,c:Color){self.set(x,y,self.pack(c));}
    pub fn rect(&self,x:usize,y:usize,w:usize,h:usize,c:Color){
        let x2=x.saturating_add(w).min(self.fb.width);let y2=y.saturating_add(h).min(self.fb.height);
        if x>=x2||y>=y2{return;}let v=self.pack(c);
        for yy in y..y2{unsafe{let dst=self.target(x,yy);if self.buffered{for i in 0..x2-x{ptr::write(dst.add(i),v);}}else{for i in 0..x2-x{ptr::write_volatile(dst.add(i),v);}}}}
    }
    pub fn round(&self,x:usize,y:usize,w:usize,h:usize,r:usize,c:Color){
        let r=r.min(w/2).min(h/2);
        if r==0{self.rect(x,y,w,h,c);return;}
        self.rect(x,y+r,w,h-2*r,c);
        for i in 0..r{
            let d=r-i;let mut off=r;
            for j in 0..r{let dx=r-j;if dx*dx+d*d<=r*r{off=j;break;}}
            self.rect(x+off,y+i,w-2*off,1,c);
            self.rect(x+off,y+h-1-i,w-2*off,1,c);
        }
    }
    /// Cached scenic wallpaper; bounded 512x288 RGB pixels, expanded to the desktop.
    pub fn aurora(&self,height:usize,dark:bool){
        let w=self.fb.width.max(1);let h=height.min(self.fb.height).max(1);
        for y in 0..h{let sy=y*540/h;for x in 0..w{
            let q=(sy*960+x*960/w)*2;let v=u16::from_le_bytes([ART[q],ART[q+1]]);
            let rgb=[(((v>>11)&31)*255/31)as u8,(((v>>5)&63)*255/63)as u8,((v&31)*255/31)as u8];
            self.set(x,y,self.pack(if dark{rgb}else{blend(rgb,[180,218,242],18,100)}));
        }}
    }
    pub fn wallpaper(&self,height:usize,alternate:bool){if alternate{let h=height.min(self.fb.height);for y in 0..h{let col=blend([4,13,28],[12,53,79],y,h.max(1));self.rect(0,y,self.fb.width,1,col);}}else{self.aurora(height,true);}}
    // Sample the existing backdrop before modifying it; 8x8 blocks and a
    // separable neighbourhood approximation keep memory bounded.
    pub fn frost(&self,x:usize,y:usize,w:usize,h:usize,r:usize,fill:Color,edge:Color){
        let w=w.min(self.fb.width.saturating_sub(x));let h=h.min(self.fb.height.saturating_sub(y));
        if w==0||h==0{return;}const STEP:usize=8;let cols=(w+7)/8;let rows=(h+7)/8;
        let mut samples=alloc::vec::Vec::<Color>::new();if samples.try_reserve_exact(cols*rows).is_err(){self.glass(x,y,w,h,r,fill,edge);return;}
        for j in 0..rows{for i in 0..cols{let px=x+(i*STEP+4).min(w-1);let py=y+(j*STEP+4).min(h-1);let mut sum=[0usize;3];
            for oy in [-10isize,0,10]{for ox in [-10isize,0,10]{let sx=(px as isize+ox).clamp(0,self.fb.width as isize-1)as usize;let sy=(py as isize+oy).clamp(0,self.fb.height as isize-1)as usize;let c=self.unpack(unsafe{ptr::read_volatile(self.target(sx,sy))});for k in 0..3{sum[k]+=c[k]as usize;}}}
            samples.push(core::array::from_fn(|k|(sum[k]/9)as u8));
        }}
        let r=r.min(w/2).min(h/2);
        for yy in 0..h{for xx in 0..w{let dx=r.saturating_sub(xx.min(w-1-xx));let dy=r.saturating_sub(yy.min(h-1-yy));if dx*dx+dy*dy>r*r{continue;}let border=xx==0||yy==0||xx+1==w||yy+1==h||(dx>0&&dy>0&&dx*dx+dy*dy>r.saturating_sub(2).pow(2));let a=samples[(yy/STEP)*cols+xx/STEP];self.pixel(x+xx,y+yy,if border{blend(a,edge,80,100)}else{blend(a,fill,80,100)});}}
    }
    pub fn tile_gradient(&self,x:usize,y:usize,w:usize,h:usize,top:Color,bottom:Color){
        self.round(x,y,w,h,9,bottom);if w<6||h<6{return;}for yy in 2..h-2{let inset=if yy<7{7-yy}else if yy>h-8{yy-(h-8)}else{2};let inset=inset.max(2);if w>2*inset{self.rect(x+inset,y+yy,w-2*inset,1,blend(top,bottom,yy,h));}}
    }
    // True alpha compositing into the existing software back buffer.
    pub fn glass(&self,x:usize,y:usize,w:usize,h:usize,r:usize,fill:Color,edge:Color){
        let r=r.min(w/2).min(h/2);
        for yy in 0..h.min(self.fb.height.saturating_sub(y)){for xx in 0..w.min(self.fb.width.saturating_sub(x)){
            let dx=r.saturating_sub(xx.min(w-1-xx));let dy=r.saturating_sub(yy.min(h-1-yy));
            if dx*dx+dy*dy>r*r{continue;}
            let border=xx==0||yy==0||xx+1==w||yy+1==h||dx*dx+dy*dy>r.saturating_sub(2).pow(2)&&dx>0&&dy>0;
            let old=self.unpack(unsafe{ptr::read_volatile(self.target(x+xx,y+yy))});
            let color=if border{blend(old,edge,75,100)}else{blend(old,fill,86,100)};
            self.pixel(x+xx,y+yy,color);
        }}
    }
    pub fn grid_mark(&self,x:usize,y:usize,col:Color){for i in 0..3{for j in 0..3{self.round(x+i*7,y+j*7,4,4,2,col);}}}
    pub fn search_mark(&self,x:usize,y:usize,col:Color){self.round(x,y,16,16,8,col);self.round(x+2,y+2,12,12,6,[18,47,69]);for i in 0..6{self.pixel(x+13+i,y+13+i,col);self.pixel(x+14+i,y+13+i,col);}}
    pub fn panel(&self,x:usize,y:usize,w:usize,h:usize,r:usize,p:Palette){
        for i in (1..=5).rev(){self.round(x+i,y+i*2,w,h,r+i,blend(p.wall,p.bar,i,6));}
        self.round(x,y,w,h,r,p.edge);
        if w>2&&h>2{self.round(x+1,y+1,w-2,h-2,r.saturating_sub(1),p.panel);}
    }
    pub fn control(&self,x:usize,y:usize,w:usize,h:usize,label:&str,fill:Color,ink:Color){
        self.round(x,y,w,h,8,fill);
        if w>10&&h>8{self.round(x+4,y+2,w-8,2,1,blend(fill,[255;3],1,10));}
        let tw=self.text_width(label);let px=x+w.saturating_sub(tw)/2;
        self.text_limit(px.max(x+3),y+h.saturating_sub(22)/2,label,ink,false,x+w.saturating_sub(3));
    }
    pub fn brand(&self,x:usize,y:usize,ink:Color){
        self.round(x,y,32,32,9,[16,40,61]);
        self.round(x+10,y+13,15,13,5,ink);
        for(i,h)in [8usize,12,15,10].into_iter().enumerate(){self.round(x+10+i*4,y+15-h,3,h+6,1,ink);}
        self.round(x+5,y+13,7,5,2,ink);
    }
    pub fn window_control(&self,x:usize,y:usize,kind:u8,p:Palette){
        self.round(x,y,30,30,8,p.sub);
        match kind{0=>self.rect(x+9,y+19,12,2,p.ink),1=>{self.rect(x+9,y+9,12,12,p.ink);self.rect(x+11,y+11,8,8,p.sub);},_=>{for i in 0..11{self.rect(x+10+i,y+10+i,2,2,p.ink);self.rect(x+20-i,y+10+i,2,2,p.ink);}}}
    }
    pub fn text(&self,x:usize,y:usize,t:&str,c:Color){self.text_limit(x,y,t,c,false,self.fb.width);}
    pub fn title(&self,x:usize,y:usize,t:&str,c:Color){self.text_limit(x,y,t,c,true,self.fb.width);}
    pub fn text_limit(&self,mut x:usize,y:usize,t:&str,c:Color,big:bool,right:usize){
        let (atlas,adv)=if big{(BIG,BIG_ADV)}else{(REG,REG_ADV)};
        for byte in t.bytes(){
            let idx=if (32..127).contains(&byte){(byte-32)as usize}else{31};
            let width=adv[idx]as usize;
            if x.saturating_add(width+1)>right{break;}
            for yy in 0..32{if y+yy>=self.fb.height{break;}for xx in 0..32{
                if x+xx>=right||x+xx>=self.fb.width{break;}
                let a=atlas[idx*1024+yy*32+xx]as u32;if a==0{continue;}
                if a==255{self.pixel(x+xx,y+yy,c);}else{
                    let old=unsafe{ptr::read_volatile(self.target(x+xx,y+yy))};let bg=self.unpack(old);
                    let mix=|i:usize|((c[i]as u32*a+bg[i]as u32*(255-a)+127)/255)as u8;
                    self.pixel(x+xx,y+yy,[mix(0),mix(1),mix(2)]);
                }
            }}
            x+=width;
        }
    }
    pub fn office_glyph(&self,x:usize,y:usize,b:u8,style:u8,c:Color,right:usize,floor:usize){
        let big=style&crate::office::LARGE!=0;let(atlas,adv)=if big{(BIG,BIG_ADV)}else{(REG,REG_ADV)};let idx=if (32..127).contains(&b){(b-32)as usize}else{31};
        for yy in 0..32{if y+yy>=floor||y+yy>=self.fb.height{break;}let slant=if style&crate::office::ITALIC!=0{(31-yy)/8}else{0};
            for xx in 0..32{let px=x+xx+slant;if px>=right||px>=self.fb.width{break;}let a=atlas[idx*1024+yy*32+xx]as u32;if a==0{continue;}
                let paint=|px:usize|{let old=unsafe{ptr::read_volatile(self.target(px,y+yy))};let bg=self.unpack(old);let mix=|i:usize|((c[i]as u32*a+bg[i]as u32*(255-a)+127)/255)as u8;self.pixel(px,y+yy,[mix(0),mix(1),mix(2)]);};paint(px);if style&crate::office::BOLD!=0&&px+1<right&&px+1<self.fb.width{paint(px+1);}
            }}
        let baseline=if big{29}else{19};if style&crate::office::UNDER!=0&&y+baseline<floor&&x<right{self.rect(x,y+baseline,(adv[idx]as usize).min(right-x),1,c);}
    }
    pub fn num(&self,x:usize,y:usize,mut n:u64,c:Color){let mut b=[b'0';20];let mut i=20;loop{i-=1;b[i]=b'0'+(n%10)as u8;n/=10;if n==0{break;}}if let Ok(t)=core::str::from_utf8(&b[i..]){self.text(x,y,t,c);}}
    /// Capture only the requested visible logical rectangle, from the back buffer.
    pub fn capture(&self,a:crate::compositor::Area)->Option<crate::compositor::Surface>{
        if !self.buffered||a!=a.clipped(self.fb.width,self.fb.height){return None;}
        let mut s=crate::compositor::Surface::new(a.w,a.h)?;
        for y in 0..a.h{unsafe{ptr::copy_nonoverlapping(self.target(a.x,a.y+y),s.row_mut(y).as_mut_ptr(),a.w);}}
        Some(s)
    }
    pub fn restore_area(&self,s:&crate::compositor::Surface,a:crate::compositor::Area){
        if !self.buffered{return;}let a=a.clipped(s.w.min(self.fb.width),s.h.min(self.fb.height));
        for y in a.y..a.y+a.h{unsafe{ptr::copy_nonoverlapping(s.row(y).as_ptr().add(a.x),self.target(a.x,y),a.w);}}
    }
    pub fn compose_window(&self,s:&crate::compositor::Surface,x:usize,y:usize,floor:usize,p:Palette){
        if !self.buffered{return;}let h=floor.min(self.fb.height);let w=self.fb.width;
        // Recreate the panel's layered shadow; clip below the dock.
        for i in (1..=5).rev(){let sx=x+i;let sy=y+i*2;let v=self.pack(blend(p.wall,p.bar,i,6));let a=crate::compositor::Area{x:sx,y:sy,w:s.w,h:s.h}.clipped(w,h);
            for yy in 0..a.h{
                let (l,r)=crate::compositor::span(yy,s.w,s.h,14+i);let left=sx+l.min(a.w);let right=sx+r.min(a.w);if left>=right{continue;}
                // Skip shadow pixels that the opaque window will cover.
                let py=sy+yy;let (cl,cr)=if py>=y&&py<y+s.h{let(l,r)=crate::compositor::span(py-y,s.w,s.h,14);(x+l,x+r)}else{(0,0)};
                let fill=|a:usize,b:usize|{if a<b{unsafe{let dst=self.target(a,py);for j in 0..b-a{ptr::write(dst.add(j),v);}}}};
                if cl>=cr||right<=cl||left>=cr{fill(left,right);}else{fill(left,right.min(cl));fill(left.max(cr),right);}
            }}
        let a=crate::compositor::Area{x,y,w:s.w,h:s.h}.clipped(w,h);
        for yy in 0..a.h{let row=s.row(yy);let (start,end)=crate::compositor::span(yy,s.w,s.h,14);let start=start.min(a.w);let end=end.min(a.w);
            if end>start{unsafe{ptr::copy_nonoverlapping(row.as_ptr().add(start),self.target(x+start,y+yy),end-start);}}}
    }
    pub fn present(&self){if !self.buffered{return;}unsafe{
        let src=ptr::addr_of!(BACK.0).cast::<u32>();let dst=self.fb.base as *mut u32;
        if self.scale==100{for y in 0..self.fb.height{for x in 0..self.fb.width{ptr::write_volatile(dst.add(y*self.fb.stride+x),ptr::read(src.add(y*self.fb.width+x)));}}}else{for y in 0..self.fb.height{for x in 0..self.fb.width{self.front_value(x,y,ptr::read(src.add(y*self.fb.width+x)));}}}
    }}
    pub fn present_rect(&self,x:usize,y:usize,w:usize,h:usize){
        if !self.buffered{return;}let right=x.saturating_add(w).min(self.fb.width);let bottom=y.saturating_add(h).min(self.fb.height);
        for yy in y..bottom{for xx in x..right{let v=unsafe{ptr::read(self.target(xx,yy))};if self.scale==100{unsafe{ptr::write_volatile((self.physical.base as *mut u32).add(yy*self.physical.stride+xx),v);}}else{self.front_value(xx,yy,v);}}}
    }
    pub fn front_pixel(&self,x:usize,y:usize,c:Color){if x<self.fb.width&&y<self.fb.height{self.front_value(x,y,self.pack(c));}}
}
