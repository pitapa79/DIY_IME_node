//! Trusted local C-API probe, not a public plugin loader or OS sandbox.
//! ABI: official Wasmtime 50.0.1 full x64 Windows C API headers.
#[cfg(all(windows, target_arch = "x86_64"))]
mod probe {
    use std::{
        ffi::{CStr, OsStr, c_void},
        io::Read,
        os::windows::ffi::OsStrExt,
        path::Path,
        ptr,
    };
    type P = *mut c_void;
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Handle {
        store: u64,
        private: usize,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Func {
        store: u64,
        private: P,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct StoredMemory {
        store: u64,
        private1: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Memory {
        stored: StoredMemory,
        private2: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    union ExternPayload {
        function: Func,
        memory: Memory,
        storage: [u64; 3],
    }
    impl Default for ExternPayload {
        fn default() -> Self {
            Self { storage: [0; 3] }
        }
    }
    #[repr(C)]
    #[derive(Default)]
    struct Extern {
        kind: u8,
        of: ExternPayload,
    }
    // Full distribution GC payload is24bytes. Inactive fields may be uninitialized.
    #[repr(C)]
    #[derive(Clone, Copy)]
    union ValPayload {
        i32: i32,
        i64: i64,
        storage: [u64; 3],
    }
    impl Default for ValPayload {
        fn default() -> Self {
            Self { storage: [0; 3] }
        }
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Val {
        kind: u8,
        of: ValPayload,
    }
    impl Val {
        fn i32(value: u32) -> Self {
            Self {
                kind: 0,
                of: ValPayload { i32: value as i32 },
            }
        }
    }
    #[repr(C)]
    struct Bytes {
        size: usize,
        data: *mut u8,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(file: *const u16, reserved: P, flags: u32) -> P;
        fn GetProcAddress(module: P, name: *const u8) -> P;
    }
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptOpenAlgorithmProvider(
            handle: *mut P,
            algorithm: *const u16,
            implementation: *const u16,
            flags: u32,
        ) -> i32;
        fn BCryptHash(
            handle: P,
            secret: *const u8,
            secret_size: u32,
            input: *const u8,
            input_size: u32,
            output: *mut u8,
            output_size: u32,
        ) -> i32;
        fn BCryptCloseAlgorithmProvider(handle: P, flags: u32) -> i32;
    }
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }
    fn verify_runtime(path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("runtime read: {e}"))?;
        let size = u32::try_from(bytes.len()).map_err(|_| "runtime file too large")?;
        let mut algorithm = ptr::null_mut();
        let mut digest = [0u8; 32];
        let name = wide(OsStr::new("SHA256"));
        // SAFETY: initialized handles/buffers, system CNG SHA256 provider, exact sizes.
        unsafe {
            if BCryptOpenAlgorithmProvider(&mut algorithm, name.as_ptr(), ptr::null(), 0) != 0 {
                return Err("SHA256 provider unavailable".into());
            }
            let status = BCryptHash(
                algorithm,
                ptr::null(),
                0,
                bytes.as_ptr(),
                size,
                digest.as_mut_ptr(),
                32,
            );
            BCryptCloseAlgorithmProvider(algorithm, 0);
            if status != 0 {
                return Err("SHA256 calculation failed".into());
            }
        }
        let hash = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if hash != "fa7fdafc1dcb98ff20acd6ea480f305b90b7a8f518cb35110904ef345517e4ef" {
            return Err("runtime does not match pinned official 50.0.1 full x64 DLL".into());
        }
        Ok(())
    }
    struct Library(P);
    // Process-owned: runtime global handlers/threads may outlive an Engine.
    // Keep the DLL resident; the OS releases it at genuine process exit.
    macro_rules! api {
  ($($field:ident:[$ty:ty] => $name:literal),+ $(,)?)=>{
   struct Api {_library:Library,$($field:$ty),+}
   impl Api {fn load(path:&Path)->Result<Self,String>{
    verify_runtime(path)?;
    let name=wide(path.as_os_str());
    // SAFETY: verified full official DLL; explicit absolute path and restricted dependency search.
    let library=Library(unsafe{LoadLibraryExW(name.as_ptr(),ptr::null_mut(),0x1100)});
    if library.0.is_null(){return Err("cannot load verified Wasmtime DLL".into());}
    $(let symbol=unsafe{GetProcAddress(library.0,concat!($name,"\0").as_ptr())};if symbol.is_null(){return Err(format!("runtime API missing: {}",$name));}
    // SAFETY: exact function signature from pinned release headers; library outlives every call.
    let $field=unsafe{std::mem::transmute::<P,$ty>(symbol)};)+
    Ok(Self{_library:library,$($field),+})
   }}
  }
 }
    api! {
     config_new:[unsafe extern "C" fn()->P] =>"wasm_config_new",
     fuel_enable:[unsafe extern "C" fn(P,bool)] =>"wasmtime_config_consume_fuel_set",
     engine_new:[unsafe extern "C" fn(P)->P] =>"wasm_engine_new_with_config",
     engine_delete:[unsafe extern "C" fn(P)] =>"wasm_engine_delete",
     store_new:[unsafe extern "C" fn(P,P,Option<unsafe extern "C" fn(P)>)->P] =>"wasmtime_store_new",
     store_context:[unsafe extern "C" fn(P)->P] =>"wasmtime_store_context",
     limiter:[unsafe extern "C" fn(P,i64,i64,i64,i64,i64)] =>"wasmtime_store_limiter",
     store_delete:[unsafe extern "C" fn(P)] =>"wasmtime_store_delete",
     fuel_set:[unsafe extern "C" fn(P,u64)->P] =>"wasmtime_context_set_fuel",
     module_new:[unsafe extern "C" fn(P,*const u8,usize,*mut P)->P] =>"wasmtime_module_new",
     module_delete:[unsafe extern "C" fn(P)] =>"wasmtime_module_delete",
     instantiate:[unsafe extern "C" fn(P,P,*const Extern,usize,*mut Handle,*mut P)->P] =>"wasmtime_instance_new",
     export:[unsafe extern "C" fn(P,*const Handle,*const u8,usize,*mut Extern)->bool] =>"wasmtime_instance_export_get",
     extern_delete:[unsafe extern "C" fn(*mut Extern)] =>"wasmtime_extern_delete",
     call:[unsafe extern "C" fn(P,*const Func,*const Val,usize,*mut Val,usize,*mut P)->P] =>"wasmtime_func_call",
     unroot:[unsafe extern "C" fn(*mut Val)] =>"wasmtime_val_unroot",
     memory_data:[unsafe extern "C" fn(P,*const Memory)->*mut u8] =>"wasmtime_memory_data",
     memory_size:[unsafe extern "C" fn(P,*const Memory)->usize] =>"wasmtime_memory_data_size",
     error_message:[unsafe extern "C" fn(P,*mut Bytes)] =>"wasmtime_error_message",
     error_delete:[unsafe extern "C" fn(P)] =>"wasmtime_error_delete",
     trap_message:[unsafe extern "C" fn(P,*mut Bytes)] =>"wasm_trap_message",
     trap_delete:[unsafe extern "C" fn(P)] =>"wasm_trap_delete",
     bytes_delete:[unsafe extern "C" fn(*mut Bytes)] =>"wasm_byte_vec_delete",
     wat:[unsafe extern "C" fn(*const u8,usize,*mut Bytes)->P] =>"wasmtime_wat2wasm",
    }
    impl Api {
        fn check(&self, error: P, trap: P) -> Result<(), String> {
            if error.is_null() && trap.is_null() {
                return Ok(());
            }
            let mut bytes = Bytes {
                size: 0,
                data: ptr::null_mut(),
            };
            // SAFETY: error/trap and owned byte-vector lifetimes follow pinned C API ownership.
            let message = unsafe {
                if !error.is_null() {
                    (self.error_message)(error, &mut bytes);
                } else {
                    (self.trap_message)(trap, &mut bytes);
                }
                let message = if bytes.data.is_null() {
                    "runtime failure".to_string()
                } else {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        bytes.data,
                        bytes.size.min(4096),
                    ))
                    .into_owned()
                };
                (self.bytes_delete)(&mut bytes);
                if !error.is_null() {
                    (self.error_delete)(error);
                }
                if !trap.is_null() {
                    (self.trap_delete)(trap);
                }
                message
            };
            Err(message)
        }
    }
    struct Runtime<'a> {
        api: &'a Api,
        engine: P,
        store: P,
        module: P,
        context: P,
        instance: Handle,
    }
    impl Drop for Runtime<'_> {
        fn drop(&mut self) {
            unsafe {
                if !self.store.is_null() {
                    (self.api.store_delete)(self.store);
                }
                if !self.module.is_null() {
                    (self.api.module_delete)(self.module);
                }
                if !self.engine.is_null() {
                    (self.api.engine_delete)(self.engine);
                }
            }
        }
    }
    impl<'a> Runtime<'a> {
        fn new(api: &'a Api, module: &[u8]) -> Result<Self, String> {
            // SAFETY: each object is owned by this Runtime, all handles belong to one Store.
            unsafe {
                let config = (api.config_new)();
                if config.is_null() {
                    return Err("config unavailable".into());
                }
                (api.fuel_enable)(config, true);
                let engine = (api.engine_new)(config);
                if engine.is_null() {
                    return Err("engine unavailable".into());
                }
                let mut result = Self {
                    api,
                    engine,
                    store: ptr::null_mut(),
                    module: ptr::null_mut(),
                    context: ptr::null_mut(),
                    instance: Handle::default(),
                };
                result.store = (api.store_new)(engine, ptr::null_mut(), None);
                if result.store.is_null() {
                    return Err("store unavailable".into());
                }
                (api.limiter)(result.store, 128 * 1024 * 1024, 10000, 1, 1, 1);
                result.context = (api.store_context)(result.store);
                api.check((api.fuel_set)(result.context, 50_000_000), ptr::null_mut())?;
                api.check(
                    (api.module_new)(engine, module.as_ptr(), module.len(), &mut result.module),
                    ptr::null_mut(),
                )?;
                let mut trap = ptr::null_mut();
                let error = (api.instantiate)(
                    result.context,
                    result.module,
                    ptr::null(),
                    0,
                    &mut result.instance,
                    &mut trap,
                );
                api.check(error, trap)?;
                Ok(result)
            }
        }
        fn export(&self, name: &CStr, kind: u8) -> Result<ExternPayload, String> {
            let mut export = Extern::default();
            // SAFETY: live instance/context in the same Store; output uses pinned header layout.
            let found = unsafe {
                (self.api.export)(
                    self.context,
                    &self.instance,
                    name.as_ptr().cast(),
                    name.to_bytes().len(),
                    &mut export,
                )
            };
            if !found {
                return Err("guest export missing".into());
            }
            let result = if export.kind == kind {
                Ok(export.of)
            } else {
                Err("guest export has wrong type".into())
            };
            unsafe {
                (self.api.extern_delete)(&mut export);
            }
            result
        }
        fn call(
            &mut self,
            name: &CStr,
            args: &[Val],
            result_kind: Option<u8>,
        ) -> Result<u64, String> {
            // SAFETY: export verified the function discriminant.
            let function = unsafe { self.export(name, 0)?.function };
            let mut result = Val::default();
            let mut trap = ptr::null_mut();
            // SAFETY: checked call verifies arity/types; buffers have their exact lengths.
            unsafe {
                self.api.check(
                    (self.api.fuel_set)(self.context, 50_000_000),
                    ptr::null_mut(),
                )?;
                let error = (self.api.call)(
                    self.context,
                    &function,
                    args.as_ptr(),
                    args.len(),
                    &mut result,
                    usize::from(result_kind.is_some()),
                    &mut trap,
                );
                self.api.check(error, trap)?;
                let valid = result_kind.is_none_or(|kind| kind == result.kind);
                // SAFETY: read only the field selected by the checked result kind.
                // In particular, never read an i32 result as a wider integer.
                let value = match (valid, result_kind) {
                    (true, Some(0)) => u64::from(result.of.i32 as u32),
                    (true, Some(1)) => result.of.i64 as u64,
                    _ => 0,
                };
                if result_kind.is_some() {
                    (self.api.unroot)(&mut result);
                }
                if valid {
                    Ok(value)
                } else {
                    Err("guest result has wrong type".into())
                }
            }
        }
        fn range(&self, memory: &Memory, offset: u32, length: usize) -> Result<*mut u8, String> {
            if length > 8 * 1024 * 1024 {
                return Err("guest buffer exceeds wire limit".into());
            }
            // Reacquire the base after each guest call, which may grow or relocate memory.
            let size = unsafe { (self.api.memory_size)(self.context, memory) };
            let offset = offset as usize;
            let end = offset.checked_add(length).ok_or("guest range overflow")?;
            if end > size {
                return Err("guest buffer out of bounds".into());
            }
            let data = unsafe { (self.api.memory_data)(self.context, memory) };
            if data.is_null() {
                return Err("guest memory unavailable".into());
            }
            Ok(unsafe { data.add(offset) })
        }
        fn invoke(&mut self, input: &[u8]) -> Result<Vec<u8>, String> {
            if input.is_empty() || input.len() > 8 * 1024 * 1024 {
                return Err("invalid request size".into());
            }
            if self.call(c"diyime_api_version", &[], Some(0))? != 1 {
                return Err("guest version unsupported".into());
            }
            // SAFETY: export verified the memory discriminant.
            let memory = unsafe { self.export(c"memory", 3)?.memory };
            let arg = Val::i32(input.len() as u32);
            let pointer = self.call(c"diyime_alloc", &[arg], Some(0))? as u32;
            if pointer == 0 {
                return Err("guest allocation rejected".into());
            }
            let target = self.range(&memory, pointer, input.len())?;
            // SAFETY: checked guest memory range, no concurrent guest calls, source is independent Host bytes.
            unsafe {
                ptr::copy_nonoverlapping(input.as_ptr(), target, input.len());
            }
            let reply = self.call(c"diyime_call", &[Val::i32(pointer), arg], Some(1))?;
            let length = (reply >> 32) as usize;
            let pointer = reply as u32;
            if pointer == 0 || length == 0 {
                return Err("guest returned empty descriptor".into());
            }
            let data = self.range(&memory, pointer, length)?;
            let bytes = unsafe { std::slice::from_raw_parts(data, length) }.to_vec();
            self.call(c"diyime_release_reply", &[], None)?;
            Ok(bytes)
        }
    }
    pub fn run() -> Result<(), String> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() != 3 {
            return Err(
                "usage: wasm_c_api_probe <verified-runtime-dll> <module.wasm> <request.json|->"
                    .into(),
            );
        }
        // Parent selected the absolute cache asset; do not query inaccessible ancestor metadata here.
        let runtime = Path::new(&args[0]);
        if !runtime.is_absolute() {
            return Err("runtime path must be absolute".into());
        }
        let api = Api::load(runtime)?;
        let mut module = std::fs::read(&args[1]).map_err(|e| format!("module read: {e}"))?;
        if Path::new(&args[1]).extension().is_some_and(|e| e == "wat") {
            let mut bytes = Bytes {
                size: 0,
                data: ptr::null_mut(),
            };
            // SAFETY: pinned runtime owns the output byte-vector; input bytes remain live.
            unsafe {
                api.check(
                    (api.wat)(module.as_ptr(), module.len(), &mut bytes),
                    ptr::null_mut(),
                )?;
                module = if bytes.size == 0 {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(bytes.data, bytes.size).to_vec()
                };
                (api.bytes_delete)(&mut bytes);
            }
        }
        if module.len() > 64 * 1024 * 1024 {
            return Err("module exceeds probe limit".into());
        }
        let input = if args[2] == "-" {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "request pipe failed")?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err("request exceeds wire limit".into());
            }
            bytes
        } else {
            std::fs::read(&args[2]).map_err(|e| format!("request read: {e}"))?
        };
        let mut guest = Runtime::new(&api, &module)?;
        let requests = if input.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'[') {
            let items: Vec<serde_json::Value> =
                serde_json::from_slice(&input).map_err(|_| "invalid probe request batch")?;
            if items.len() > 1000 {
                return Err("probe batch too large".into());
            }
            Some(items)
        } else {
            None
        };
        let value = if let Some(items) = requests {
            let mut output = Vec::new();
            for item in items {
                let input = serde_json::to_vec(&item).map_err(|e| e.to_string())?;
                let reply = guest.invoke(&input)?;
                output.push(
                    serde_json::from_slice::<serde_json::Value>(&reply)
                        .map_err(|_| "guest response is not JSON")?,
                );
            }
            serde_json::Value::Array(output)
        } else {
            serde_json::from_slice::<serde_json::Value>(&guest.invoke(&input)?)
                .map_err(|_| "guest response is not JSON")?
        };
        println!(
            "{}",
            serde_json::to_string(&value).map_err(|e| e.to_string())?
        );
        Ok(())
    }
    #[test]
    fn pinned_full_x64_c_layout() {
        assert_eq!(std::mem::size_of::<Handle>(), 16);
        assert_eq!(std::mem::size_of::<Func>(), 16);
        assert_eq!(std::mem::size_of::<Memory>(), 24);
        assert_eq!(std::mem::size_of::<Extern>(), 32);
        assert_eq!(std::mem::size_of::<Val>(), 32);
        assert_eq!(std::mem::align_of::<Val>(), 8);
    }
}
fn main() {
    #[cfg(all(windows, target_arch = "x86_64"))]
    if let Err(error) = probe::run() {
        eprintln!("wasm-probe: {error}");
        std::process::exit(1);
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        eprintln!("wasm-probe requires x64 Windows");
        std::process::exit(1);
    }
}
