// Try-out: runs a Lua script inside Path of Building's own LuaJIT (its lua51.dll), so PoB's C modules load too.
//   pobhost <PoB folder> <script.lua>
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void, CStr, CString};

type State = *mut c_void;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    std::env::set_current_dir(dir).expect("PoB folder");
    unsafe {
        let lib = Library::new(std::path::Path::new(dir).join("lua51.dll")).expect("lua51.dll");
        let newstate: Symbol<unsafe extern "C" fn() -> State> = lib.get(b"luaL_newstate").unwrap();
        let openlibs: Symbol<unsafe extern "C" fn(State)> = lib.get(b"luaL_openlibs").unwrap();
        let loadfile: Symbol<unsafe extern "C" fn(State, *const c_char) -> c_int> = lib.get(b"luaL_loadfile").unwrap();
        let pcall: Symbol<unsafe extern "C" fn(State, c_int, c_int, c_int) -> c_int> = lib.get(b"lua_pcall").unwrap();
        let tolstring: Symbol<unsafe extern "C" fn(State, c_int, *mut usize) -> *const c_char> = lib.get(b"lua_tolstring").unwrap();
        let l = newstate();
        openlibs(l);
        let script = CString::new(args[2].as_str()).unwrap();
        let mut r = loadfile(l, script.as_ptr());
        if r == 0 { r = pcall(l, 0, 0, 0); }
        if r != 0 {
            let p = tolstring(l, -1, std::ptr::null_mut());
            eprintln!("Lua error: {}", if p.is_null() { "?".into() } else { CStr::from_ptr(p).to_string_lossy() });
            std::process::exit(1);
        }
    }
}
