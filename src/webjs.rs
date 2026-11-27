//! Fuel-limited QuickJS-NG WebAssembly adapter. No host filesystem, sockets,
 //! DOM pointers or arbitrary native calls are exposed to the module.
use wasmi::{Config, Engine, ExternType, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, Value};
const WASM:&[u8]=include_bytes!("../target/hi5-js/quickjs.wasm");
const FUEL:u64=120_000_000;
const MEMORY:usize=8*1024*1024;
pub struct Session{store:Store<StoreLimits>,instance:Instance,memory:Memory}
impl Session{
    pub fn new()->Result<Self,&'static str>{
        let mut config=Config::default();config.consume_fuel(true);
        let engine=Engine::new(&config);
        let module=Module::new(&engine,WASM).map_err(|_|"invalid WASM module")?;
        let mut store=Store::new(&engine,StoreLimitsBuilder::new().memory_size(MEMORY).table_elements(8192).instances(1).memories(1).tables(1).trap_on_grow_failure(true).build());
        store.limiter(|v|v);store.add_fuel(FUEL).map_err(|_|"fuel setup")?;
        let mut linker=Linker::new(&engine);
        for import in module.imports(){
            let ExternType::Func(ty)=import.ty() else{return Err("non-function import");};
            let kind=match (import.module(),import.name()){
                ("env","host_get_timezone_offset")=>0,
                ("env","host_interrupt")=>0,
                ("env","host_promise_rejection")=>0,
                ("env","host_module_normalize")=>0,
                ("env","host_module_load")=>0,
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
                }else if kind==2{if let Some(out)=results.first_mut(){*out=Value::I32(52);}}
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
        instance.get_typed_func::<i32,()>(&store,"qjs_set_memory_limit").map_err(|_|"memory limit export")?.call(&mut store,2*1024*1024).map_err(|_|"memory limit setup")?;
        Ok(Self{store,instance,memory})
    }
    fn alloc(&mut self,n:usize)->Result<i32,&'static str>{
        if n==0||n>64*1024{return Err("WASM input limit");}
        let p=self.instance.get_typed_func::<i32,i32>(&self.store,"wasm_malloc").map_err(|_|"malloc export")?.call(&mut self.store,n as i32).map_err(|_|"WASM allocation trap")?;
        if p<=0{return Err("WASM allocation failed");}Ok(p)
    }
    fn free(&mut self,p:i32){if let Ok(f)=self.instance.get_typed_func::<i32,()>(&self.store,"wasm_free"){let _=f.call(&mut self.store,p);}}
    fn input(&mut self,b:&[u8])->Result<i32,&'static str>{let p=self.alloc(b.len()+1)?;self.memory.write(&mut self.store,p as usize,b).map_err(|_|"WASM write")?;self.memory.write(&mut self.store,p as usize+b.len(),&[0]).map_err(|_|"WASM write")?;Ok(p)}
    fn evaluate(&mut self,src:&[u8])->Result<i32,&'static str>{
        if src.len()>48*1024{return Err("JS size limit");}
        let p=self.input(src)?;let file=self.input(b"hi5web")?;
        let eval=self.instance.get_typed_func::<(i32,i32,i32,i32),i32>(&self.store,"qjs_eval").map_err(|_|"eval export")?;
        let v=eval.call(&mut self.store,(p,src.len()as i32,file,0)).map_err(|_|"JavaScript budget or execution trap")?;
        self.free(p);self.free(file);
        if v<=0{return Err("JavaScript result allocation");}
        let exception=self.instance.get_typed_func::<i32,i32>(&self.store,"qjs_is_exception").map_err(|_|"exception export")?.call(&mut self.store,v).map_err(|_|"exception check trap")?;
        if exception!=0{return Err("JavaScript exception - unsupported API or syntax");}Ok(v)
    }
    fn release(&mut self,v:i32)->Result<(),&'static str>{self.instance.get_typed_func::<i32,()>(&self.store,"qjs_free_value").map_err(|_|"free value export")?.call(&mut self.store,v).map_err(|_|"free value trap")}
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
pub fn self_test()->bool{
    let Ok(mut s)=Session::new()else{return false;};
    let mut out=[0u8;64];
    let a=s.eval(b"let sum=0; for(let i=1;i<=7;i++){sum+=i}; String(sum)",&mut out);
    if a!=Ok(2)||&out[..2]!=b"28"{return false;}
    let b=s.eval(b"const f=(x)=>x*2; String(f(sum))",&mut out);
    b==Ok(2)&&&out[..2]==b"56"
}
