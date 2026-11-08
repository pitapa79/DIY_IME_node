//! Separate producer/consumer modules. No default registry or package cutover.
#[cfg(target_arch = "wasm32")]
mod abi {
    use std::sync::Mutex;
    static REQUEST: Mutex<Option<Box<[u8]>>> = Mutex::new(None);
    static RESPONSE: Mutex<Option<Box<[u8]>>> = Mutex::new(None);
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_api_version() -> u32 {
        1
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_alloc(length: u32) -> u32 {
        let length = length as usize;
        if length == 0 || length > diy_ime2_nodes::guest::MAX_MESSAGE_BYTES {
            return 0;
        }
        let bytes = vec![0; length].into_boxed_slice();
        let pointer = bytes.as_ptr() as u32;
        *REQUEST.lock().unwrap() = Some(bytes);
        pointer
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_call(pointer: u32, length: u32) -> u64 {
        let input = {
            let mut pending = REQUEST.lock().unwrap();
            if !pending.as_ref().is_some_and(|bytes| {
                bytes.as_ptr() as u32 == pointer && bytes.len() == length as usize
            }) {
                return 0;
            }
            pending.take().unwrap()
        };
        #[cfg(feature = "producer")]
        let result =
            diy_ime2_nodes::dictionary_artifact_guest::handle_prepare(&input).into_boxed_slice();
        #[cfg(not(feature = "producer"))]
        let result =
            diy_ime2_nodes::dictionary_artifact_guest::handle_execute(&input).into_boxed_slice();
        let descriptor = ((result.len() as u64) << 32) | result.as_ptr() as u64;
        *RESPONSE.lock().unwrap() = Some(result);
        descriptor
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_release_reply() {
        *RESPONSE.lock().unwrap() = None;
    }
}
