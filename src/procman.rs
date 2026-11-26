//! Bounded process/thread bookkeeping, ready queue and IPC, not a CPU context switcher.
#[derive(Clone,Copy,PartialEq,Eq)]enum State{Free,Ready,Running,Waiting,Exited}
#[derive(Clone,Copy)]struct Thread{id:u32,pid:u32,state:State,wait:u32,code:u32}
const T:Thread=Thread{id:0,pid:0,state:State::Free,wait:0,code:0};
#[derive(Clone,Copy)]struct Process{id:u32,alive:bool,namespace:u32}
const P:Process=Process{id:0,alive:false,namespace:0};
#[derive(Clone,Copy)]struct Mail{from:u32,to:u32,value:u64}
const M:Mail=Mail{from:0,to:0,value:0};
pub struct Manager{p:[Process;16],t:[Thread;64],mail:[Mail;64],next:u32,cursor:usize}
impl Manager{
    pub const fn new()->Self{Self{p:[P;16],t:[T;64],mail:[M;64],next:1,cursor:0}}
    fn id(&mut self)->Option<u32>{let n=self.next;self.next=n.checked_add(1)?;Some(n)}
    pub fn spawn(&mut self)->Option<u32>{let i=self.p.iter().position(|p|!p.alive)?;let id=self.id()?;self.p[i]=Process{id,alive:true,namespace:id};Some(id)}
    fn exists(&self,pid:u32)->bool{self.p.iter().any(|p|p.alive&&p.id==pid)}
    pub fn thread(&mut self,pid:u32)->Option<u32>{if !self.exists(pid){return None;}let i=self.t.iter().position(|t|matches!(t.state,State::Free|State::Exited))?;let id=self.id()?;self.t[i]=Thread{id,pid,state:State::Ready,wait:0,code:0};Some(id)}
    pub fn dispatch(&mut self)->Option<u32>{for t in &mut self.t{if t.state==State::Running{t.state=State::Ready;}}for _ in 0..64{let i=self.cursor;self.cursor=(i+1)%64;if self.t[i].state==State::Ready{self.t[i].state=State::Running;return Some(self.t[i].id);}}None}
    pub fn wait(&mut self,id:u32,event:u32)->bool{if event==0{return false;}let Some(t)=self.t.iter_mut().find(|t|t.id==id&&matches!(t.state,State::Running|State::Ready))else{return false;};t.state=State::Waiting;t.wait=event;true}
    pub fn signal(&mut self,pid:u32,event:u32)->usize{let mut count=0;for t in &mut self.t{if t.pid==pid&&t.state==State::Waiting&&t.wait==event{t.state=State::Ready;t.wait=0;count+=1;}}count}
    pub fn send(&mut self,from:u32,to:u32,value:u64)->bool{if !self.exists(from)||!self.exists(to){return false;}let Some(m)=self.mail.iter_mut().find(|m|m.to==0)else{return false;};*m=Mail{from,to,value};true}
    pub fn recv(&mut self,pid:u32)->Option<(u32,u64)>{if !self.exists(pid){return None;}let m=self.mail.iter_mut().find(|m|m.to==pid)?;let out=(m.from,m.value);*m=M;Some(out)}
    pub fn namespace(&self,pid:u32)->Option<u32>{self.p.iter().find(|p|p.alive&&p.id==pid).map(|p|p.namespace)}
    pub fn exit(&mut self,pid:u32,code:u32)->bool{let Some(p)=self.p.iter_mut().find(|p|p.alive&&p.id==pid)else{return false;};p.alive=false;for t in &mut self.t{if t.pid==pid{t.state=State::Exited;t.code=code;}}for m in &mut self.mail{if m.from==pid||m.to==pid{*m=M;}}true}
}
pub fn self_test()->bool{
    let mut m=Manager::new();let Some(a)=m.spawn()else{return false;};let Some(b)=m.spawn()else{return false;};
    let Some(x)=m.thread(a)else{return false;};let Some(y)=m.thread(b)else{return false;};
    if m.namespace(a)==m.namespace(b)||m.dispatch()!=Some(x)||!m.wait(x,1)||m.dispatch()!=Some(y)||m.signal(b,1)!=0||m.signal(a,1)!=1||!m.send(a,b,42)||m.recv(a).is_some()||m.recv(b)!=Some((a,42)){return false;}
    m.dispatch()==Some(x)&&m.exit(a,0)&&m.namespace(a).is_none()&&!m.send(a,b,1)&&m.dispatch()==Some(y)
}
