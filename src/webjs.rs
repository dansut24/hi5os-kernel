//! Fuel-limited QuickJS-NG WebAssembly adapter. No host filesystem, sockets,
 //! DOM pointers or arbitrary native calls are exposed to the module.
use alloc::vec::Vec;
use wasmi::{Caller,Config, Engine, ExternType, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, Value};
const WASM:&[u8]=include_bytes!("../target/hi5-js/quickjs.wasm");
pub const SOURCE_CAP:usize=512*1024;
const FUEL:u64=360_000_000;
const MODULE_CAP:usize=2*1024*1024;
struct Host{limits:StoreLimits,modules:Vec<(Vec<u8>,Vec<u8>)>,bytes:usize}
pub struct Session{store:Store<Host>,instance:Instance,memory:Memory}
impl Session{
    pub fn new(ram_mib:u64)->Result<Self,&'static str>{
        let memory_limit=if ram_mib>=384{16*1024*1024}else{8*1024*1024};let js_limit=if ram_mib>=384{6*1024*1024}else{3*1024*1024};
        let mut config=Config::default();config.consume_fuel(true);
        let engine=Engine::new(&config);
        let module=Module::new(&engine,WASM).map_err(|_|"invalid WASM module")?;
        let mut store=Store::new(&engine,Host{limits:StoreLimitsBuilder::new().memory_size(memory_limit).table_elements(8192).instances(1).memories(1).tables(1).trap_on_grow_failure(true).build(),modules:Vec::new(),bytes:0});
        store.limiter(|v|&mut v.limits);store.add_fuel(FUEL).map_err(|_|"fuel setup")?;
        let mut linker=Linker::new(&engine);
        for import in module.imports(){
            let ExternType::Func(ty)=import.ty() else{return Err("non-function import");};
            let kind=match (import.module(),import.name()){
                ("env","host_get_timezone_offset")=>0,
                ("env","host_interrupt")=>0,
                ("env","host_promise_rejection")=>0,
                ("env","host_module_normalize")=>3,
                ("env","host_module_load")=>4,
                ("env","host_call")=>0,
                ("wasi_snapshot_preview1","clock_time_get")=>1,
                ("wasi_snapshot_preview1","fd_close"|"fd_fdstat_get"|"fd_seek"|"fd_write"|"random_get")=>2,
                _=>return Err("unknown WASM import"),
            };
            linker.func_new(import.module(),import.name(),ty.clone(),move |mut caller,params,results|{
                // Host clock is deliberately synthetic; no filesystem or random API.
                if kind==1{
                    let success=params.get(2).and_then(Value::i32).filter(|v|*v>=0)
                        .and_then(|at|caller.get_export("memory").and_then(|v|v.into_memory()).map(|m|m.write(&mut caller,at as usize,&0u64.to_le_bytes()).is_ok())).unwrap_or(false);
                    if let Some(out)=results.first_mut(){*out=Value::I32(if success{0}else{21});}
                }else if kind==2{if let Some(out)=results.first_mut(){*out=Value::I32(52);}}else if kind==3||kind==4{let p=module_host(&mut caller,kind,params).unwrap_or(0);if let Some(out)=results.first_mut(){*out=Value::I32(p);}}
                // The env shims deliberately return typed zero/null defaults.
                Ok(())
            }).map_err(|_|"WASM import setup")?;
        }
        let instance=linker.instantiate(&mut store,&module).map_err(|_|"WASM instantiate")?.start(&mut store).map_err(|_|"WASM start")?;
        let memory=instance.get_memory(&store,"memory").ok_or("WASM memory")?;
        let init=instance.get_typed_func::<(),()>(&store,"_initialize").map_err(|_|"WASI initialize export")?;
        init.call(&mut store,()).map_err(|_|"WASI initialize failed")?;
        let init=instance.get_typed_func::<(),i32>(&store,"qjs_init").map_err(|_|"QuickJS init export")?;
        if init.call(&mut store,()).map_err(|_|"QuickJS initialization trapped")?!=0{return Err("QuickJS initialization failed");}
        instance.get_typed_func::<i32,()>(&store,"qjs_set_memory_limit").map_err(|_|"memory limit export")?.call(&mut store,js_limit).map_err(|_|"memory limit setup")?;
        instance.get_typed_func::<i32,()>(&store,"qjs_set_module_loader").map_err(|_|"module loader export")?.call(&mut store,1).map_err(|_|"module loader init")?;
        Ok(Self{store,instance,memory})
    }
    fn alloc(&mut self,n:usize)->Result<i32,&'static str>{
        if n==0||n>SOURCE_CAP+1{return Err("WASM input limit");}
        let p=self.instance.get_typed_func::<i32,i32>(&self.store,"wasm_malloc").map_err(|_|"malloc export")?.call(&mut self.store,n as i32).map_err(|_|"WASM allocation trap")?;
        if p<=0{return Err("WASM allocation failed");}Ok(p)
    }
    fn free(&mut self,p:i32){if let Ok(f)=self.instance.get_typed_func::<i32,()>(&self.store,"wasm_free"){let _=f.call(&mut self.store,p);}}
    fn input(&mut self,b:&[u8])->Result<i32,&'static str>{let p=self.alloc(b.len()+1)?;self.memory.write(&mut self.store,p as usize,b).map_err(|_|"WASM write")?;self.memory.write(&mut self.store,p as usize+b.len(),&[0]).map_err(|_|"WASM write")?;Ok(p)}
    fn evaluate(&mut self,src:&[u8])->Result<i32,&'static str>{
        if src.len()>SOURCE_CAP{return Err("JS size limit");}
        let p=self.input(src)?;let file=self.input(b"hi5web")?;
        let eval=self.instance.get_typed_func::<(i32,i32,i32,i32),i32>(&self.store,"qjs_eval").map_err(|_|"eval export")?;
        let v=eval.call(&mut self.store,(p,src.len()as i32,file,0)).map_err(|_|"JavaScript budget or execution trap")?;
        self.free(p);self.free(file);
        if v<=0{return Err("JavaScript result allocation");}
        let exception=self.instance.get_typed_func::<i32,i32>(&self.store,"qjs_is_exception").map_err(|_|"exception export")?.call(&mut self.store,v).map_err(|_|"exception check trap")?;
        if exception!=0{self.release(v)?;let e=self.instance.get_typed_func::<(),i32>(&self.store,"qjs_get_exception").map_err(|_|"exception retrieval export")?.call(&mut self.store,()).map_err(|_|"exception retrieval trap")?;if e<=0{return Err("Exception allocation failed");}self.release(e)?;return Err("JavaScript exception - unsupported API or syntax");}Ok(v)
    }
    fn release(&mut self,v:i32)->Result<(),&'static str>{self.instance.get_typed_func::<i32,()>(&self.store,"qjs_free_value").map_err(|_|"free value export")?.call(&mut self.store,v).map_err(|_|"free value trap")}
    pub fn add_module(&mut self,name:&[u8],source:Vec<u8>)->bool{
        if name.len()>200||source.len()>SOURCE_CAP||source.len()>MODULE_CAP-self.store.data().bytes||self.store.data().modules.len()>=32||core::str::from_utf8(&source).is_err(){return false;}
        if self.store.data().modules.iter().any(|(n,_)|n==name){return true;}let mut key=Vec::new();if key.try_reserve_exact(name.len()).is_err(){return false;}key.extend_from_slice(name);let h=self.store.data_mut();if h.modules.try_reserve(1).is_err(){return false;}h.bytes+=source.len();h.modules.push((key,source));true
    }
    pub fn drain_jobs(&mut self)->Result<(),&'static str>{
        let pending=self.instance.get_typed_func::<(),i32>(&self.store,"qjs_is_job_pending").map_err(|_|"promise pending export")?;
        let execute=self.instance.get_typed_func::<(),i32>(&self.store,"qjs_execute_pending_job").map_err(|_|"promise job export")?;
        for _ in 0..64{if pending.call(&mut self.store,()).map_err(|_|"Promise budget trap")?==0{return Ok(());}if execute.call(&mut self.store,()).map_err(|_|"Promise budget trap")?<0{return Err("Promise job failed");}}
        if pending.call(&mut self.store,()).map_err(|_|"Promise budget trap")?!=0{Err("Promise job budget exhausted")}else{Ok(())}
    }
    pub fn exec_module(&mut self,name:&[u8])->Result<(),&'static str>{
        if name.len()>200||!name.iter().all(|b|(33..127).contains(b)&&!matches!(*b,b'"'|b'\\'|b'\'')){return Err("Invalid module name");}
        let mut src=Vec::new();src.extend_from_slice(b"import('");src.extend_from_slice(name);src.extend_from_slice(b"')");
        let v=self.evaluate(&src)?;self.drain_jobs()?;
        let state=self.instance.get_typed_func::<i32,i32>(&self.store,"qjs_promise_state").map_err(|_|"promise state export")?.call(&mut self.store,v).map_err(|_|"module promise trap")?;
        self.release(v)?;if state==1{Ok(())}else{Err("JavaScript exception - unsupported API or syntax")}
    }
    pub fn exec(&mut self,src:&[u8])->Result<(),&'static str>{let v=self.evaluate(src)?;self.release(v)}
    pub fn eval(&mut self,src:&[u8],out:&mut[u8])->Result<usize,&'static str>{
        if out.is_empty(){return Err("JS result buffer");}
        let v=self.evaluate(src)?;let lp=self.alloc(4)?;
        let s=self.instance.get_typed_func::<(i32,i32),i32>(&self.store,"qjs_get_string_len").map_err(|_|"string export")?.call(&mut self.store,(v,lp)).map_err(|_|"JS result conversion trap")?;
        if s<=0{return Err("JavaScript result string");}
        let mut b=[0u8;4];self.memory.read(&self.store,lp as usize,&mut b).map_err(|_|"WASM read")?;let n=u32::from_le_bytes(b)as usize;
        if n>out.len(){return Err("JavaScript result too large");}
        self.memory.read(&self.store,s as usize,&mut out[..n]).map_err(|_|"WASM result read")?;
        self.instance.get_typed_func::<i32,()>(&self.store,"qjs_free_cstring").map_err(|_|"free string export")?.call(&mut self.store,s).map_err(|_|"free string trap")?;
        self.release(v)?;self.free(lp);Ok(n)
    }
}
fn guest_string(c:&Caller<'_,Host>,p:i32)->Option<Vec<u8>>{
    if p<=0{return None;}let m=c.get_export("memory")?.into_memory()?;let d=m.data(c);let tail=d.get(p as usize..)?;let len=tail.iter().take(201).position(|v|*v==0)?;if len==0{return None;}let mut s=Vec::new();s.try_reserve_exact(len).ok()?;s.extend_from_slice(&tail[..len]);Some(s)
}
fn guest_alloc(c:&mut Caller<'_,Host>,s:&[u8])->Option<i32>{
    let f=c.get_export("wasm_malloc")?.into_func()?.typed::<i32,i32>(&*c).ok()?;let p=f.call(&mut *c,(s.len()+1)as i32).ok()?;if p<=0{return None;}let m=c.get_export("memory")?.into_memory()?;m.write(&mut *c,p as usize,s).ok()?;m.write(&mut *c,p as usize+s.len(),&[0]).ok()?;Some(p)
}
fn module_name(base:&[u8],name:&[u8])->Option<Vec<u8>>{
    // Only absolute and relative URL specifiers, exact matches in the preloaded registry.
    if name.starts_with(b"https://")||name.starts_with(b"http://"){return Some(name.to_vec());}
    if !(name.starts_with(b"./")||name.starts_with(b"../")||name.starts_with(b"/"))||name.starts_with(b"//"){return None;}
    let scheme=if base.starts_with(b"https://"){8}else if base.starts_with(b"http://"){7}else{return None;};let end=base[scheme..].iter().position(|v|*v==b'/').map_or(base.len(),|n|scheme+n);
    let root=&base[..end];let mut path=Vec::new();if !name.starts_with(b"/"){let p=base.split(|v|matches!(*v,b'?'|b'#')).next()?;let dir=p.iter().rposition(|v|*v==b'/').filter(|n|*n>=end).map_or(b"/".as_slice(),|n|&p[end..=n]);path.extend_from_slice(dir);}path.extend_from_slice(name);
    let mut parts:Vec<&[u8]>=Vec::new();for p in path.split(|v|*v==b'/'){match p{b""|b"."=>{},b".."=>{parts.pop();},_=>parts.push(p)}}
    let mut out=root.to_vec();for p in parts{out.push(b'/');out.extend_from_slice(p);}if out.len()>200{None}else{Some(out)}
}
fn module_host(c:&mut Caller<'_,Host>,kind:i32,params:&[Value])->Option<i32>{
    let a=guest_string(c,params.first()?.i32()?)?;
    if kind==3{let b=guest_string(c,params.get(1)?.i32()?)?;let name=module_name(&a,&b)?;
        if !c.data().modules.iter().any(|(n,_)|*n==name){return None;}guest_alloc(c,&name)
    }else{let ptr=params.get(1)?.i32()?;if ptr<=0{return None;}let (_,src)=c.data().modules.iter().find(|(n,_)|*n==a)?;let mut copy=Vec::new();copy.try_reserve_exact(src.len()).ok()?;copy.extend_from_slice(src);let len=copy.len()as u32;
        let p=guest_alloc(c,&copy)?;let m=c.get_export("memory")?.into_memory()?;m.write(&mut *c,ptr as usize,&len.to_le_bytes()).ok()?;Some(p)}
}
pub fn self_test()->bool{
    let Ok(mut s)=Session::new(256)else{return false;};
    let mut out=[0u8;64];
    let a=s.eval(b"let sum=0; for(let i=1;i<=7;i++){sum+=i}; String(sum)",&mut out);
    if a!=Ok(2)||&out[..2]!=b"28"{return false;}
    let b=s.eval(b"const f=(x)=>x*2; String(f(sum))",&mut out);
    b==Ok(2)&&&out[..2]==b"56"&&s.exec(b"throw new Error('fixture')")==Err("JavaScript exception - unsupported API or syntax")&&s.eval(b"String(sum+1)",&mut out)==Ok(2)&&&out[..2]==b"29"
}
