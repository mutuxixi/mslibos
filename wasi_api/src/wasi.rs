extern crate alloc;

use core::slice;
use alloc::{format, string::String, vec::Vec};
use spin::{Mutex, MutexGuard};
use hashbrown::HashMap;

use ms_hostcall::types::{OpenFlags, OpenMode};
use ms_std::{
    libos::libos,
    println,
    time::{SystemTime, UNIX_EPOCH},
};
use tinywasm::{FuncContext, MemoryStringExt};

#[repr(C)]
struct WasiCiovec {
    buf: u32,
    buf_len: u32,
}

#[repr(C)]
struct WasiFdstat {
    fs_filetype: u8,
    fs_flags: u16,
    fs_rights_base: u64,
    fs_rights_inheriting: u64,
}

#[repr(C)]
struct WasiPrestatDir {
    dirname_len: u32,
}

#[repr(C)]
struct WasiPrestatUt {
    dir: WasiPrestatDir,
}

#[repr(C)]
struct WasiPrestatT {
    tag: u8,
    u: WasiPrestatUt,
}

#[derive(Clone)]
pub struct WasiState {
    pub args: Vec<String>
}

lazy_static::lazy_static! {
    static ref WASI_STATE: Mutex<HashMap<usize, WasiState>> = Mutex::new( HashMap::new() );
}

// This is a non-pub function because it should not be init in other file.
fn get_hashmap_wasi_state_mut() -> MutexGuard<'static, HashMap<usize, WasiState>> {
    WASI_STATE.lock()
}

fn get_wasi_state<'a>(id: usize, map: &'a MutexGuard<'static, HashMap<usize, WasiState>>) -> &'a WasiState {
    let wasi_state =  map.get(&id).unwrap();
    if wasi_state.args.len() == 0 {
        panic!("WASI_STATE uninit")
    }
    wasi_state
}

pub fn set_wasi_state(id: usize, _args: Vec<String>) {
    let mut map = get_hashmap_wasi_state_mut();
    let wasi_state: WasiState = WasiState{args: _args};
    map.insert(id, (&wasi_state).clone());
}

struct LCG {
    state: u64,
}

impl LCG {
    fn new(seed: u64) -> Self {
        LCG { state: seed }
    }

    fn next_u8(&mut self) -> u8 {
        // LCG的参数
        const A: u64 = 1664525;
        const C: u64 = 1013904223;
        const MOD: u64 = 1 << 32;

        // 更新状态
        self.state = (A.wrapping_mul(self.state).wrapping_add(C)) % MOD;

        // 返回一个0到255之间的随机u8
        (self.state % 256) as u8
    }

    fn generate_random_u8_slice(&mut self, length: usize) -> Vec<u8> {
        (0..length).map(|_| self.next_u8()).collect()
    }
}

pub fn args_get(mut ctx: FuncContext, args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature="log")]
    {
        println!("[Debug] Invoke into args_get");
        println!("args: argv: {:?}, argv_buf: {:?}", args.0, args.1);
    }

    // argv是每个arg在argv_buf中的起始地址的数组的起始地址
    // argv_buf是存arg的buf的起始地址
    // 在buf中存入arg，并以\0结尾 (len需要+1)
    let argv = args.0 as usize;
    let argv_buf = args.1 as usize;

    let ctx_id = ctx.store().id();
    let map = WASI_STATE.lock();
    let wasi_state = get_wasi_state(ctx_id, &map);
    let args: Vec<&[u8]> = wasi_state.args
        .iter()
        .map(|a| a.as_bytes())
        .collect::<Vec<_>>();
    let mut offset: usize = 0;

    let mut mem = ctx.exported_memory_mut("memory")?;

    #[cfg(feature="log")]
    println!("arg_vec len: {}", args.len());
    for (i, arg) in args.iter().enumerate() {
        #[cfg(feature="log")]
        println!("i: {:?}, offset: {:?}, arg: {:?}, arg_len: {:?}", i, offset, arg, arg.len());
        let arg_addr = argv_buf + offset;

        mem.store(argv + i * core::mem::size_of::<u32>(), core::mem::size_of::<u32>(), &(arg_addr as u32).to_ne_bytes())?;
        mem.store(arg_addr, arg.len(), arg)?;
        mem.store(arg_addr + arg.len(), core::mem::size_of::<u8>(), "\0".as_bytes())?;

        offset += arg.len() + 1;
    }

    Ok(0)
}

pub fn args_sizes_get(mut ctx: FuncContext, args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into args_sizes_get");
        println!("args: argc: {:?}, argv_buf_size: {:?}", args.0, args.1);
    }

    let argc_ptr = args.0 as usize;
    let argv_buf_size_ptr = args.1 as usize;

    let ctx_id = ctx.store().id();
    let map = WASI_STATE.lock();
    let wasi_state = get_wasi_state(ctx_id, &map);
    let argc_val = wasi_state.args.len();
    let argv_buf_size_val: usize = wasi_state.args.iter().map(|v| v.len() + 1).sum();

    #[cfg(feature="log")]
    println!("argc_val={:?}, argv_buf_size_val: {:?}", argc_val, argv_buf_size_val);

    let mut mem = ctx.exported_memory_mut("memory")?;

    mem.store(argc_ptr, core::mem::size_of::<u32>(), &(argc_val as u32).to_ne_bytes())?;
    mem.store(argv_buf_size_ptr, core::mem::size_of::<u32>(), &(argv_buf_size_val as u32).to_ne_bytes())?;

    Ok(0)
}

pub fn clock_time_get(mut ctx: FuncContext, args: (i32, i64, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into clock_time_get");
        println!(
            "args: clock_id: {:?}, precision: {:?}, time: {:?}",
            args.0, args.1, args.2
        );
    }

    println!("[Debug] Invoke into clock_time_get");
    println!(
        "args: clock_id: {:?}, precision: {:?}, time: {:?}",
        args.0, args.1, args.2
    );

    let time_ptr = args.2 as usize;
    let mut mem = ctx.exported_memory_mut("memory")?;
    let time_var = SystemTime::now().duration_since(UNIX_EPOCH).as_nanos();
    mem.store(
        time_ptr,
        core::mem::size_of::<u128>(),
        &time_var.to_ne_bytes(),
    )?;

    Ok(0)
}

pub fn environ_get(_: FuncContext<'_>, _args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into environ_get");
        println!("args: environ: {:?}, environ_buf: {:?}", _args.0, _args.1);
    }
    Ok(0)
}

pub fn environ_sizes_get(mut ctx: FuncContext, args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into environ_sizes_get");
        println!(
            "args: environ_count: {:?}, environ_buf_size: {:?}",
            args.0, args.1
        );
    }

    let count_ptr = args.0 as usize;
    let buf_size_ptr = args.1 as usize;
    let count = 0i32;
    let buf_size = 0i32;
    let mut mem = ctx.exported_memory_mut("memory")?;
    mem.store(count_ptr, core::mem::size_of::<i32>(), &count.to_ne_bytes())?;
    mem.store(
        buf_size_ptr,
        core::mem::size_of::<i32>(),
        &buf_size.to_ne_bytes(),
    )?;

    Ok(0)
}

pub fn fd_close(mut _ctx: FuncContext, _args: i32) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_close");
        println!("args: fd: {:?}", _args);
    }

    let fd = _args as u32;
    libos!(close(fd)).unwrap();

    Ok(0)
}

pub fn fd_fdstat_get(mut ctx: FuncContext<'_>, args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_fdstat_get");
        println!("args: fd: {:?}, retptr: {:?}", args.0, args.1);
    }
    let fd = args.0 as u32;
    let retptr = args.1 as usize;
    let mut mem = ctx.exported_memory_mut("memory")?;

    let mut fdstat: WasiFdstat = WasiFdstat {
        fs_filetype: 0,
        fs_flags: 0,
        fs_rights_base: 0,
        fs_rights_inheriting: 0,
    };
    match fd {
        0 => {
            // stdin
            fdstat.fs_filetype = 2; // CharacterDevice
            fdstat.fs_flags = 0;
            fdstat.fs_rights_base = 0xFFFFFFFFFFFFFFFF;
            fdstat.fs_rights_inheriting = 0;
        }
        1 => {
            // stdout
            fdstat.fs_filetype = 2;
            fdstat.fs_flags = 1;
            fdstat.fs_rights_base = 0xFFFFFFFFFFFFFFFF;
            fdstat.fs_rights_inheriting = 0;
        }
        2 => {
            // stderr
            fdstat.fs_filetype = 2;
            fdstat.fs_flags = 1;
            fdstat.fs_rights_base = 0xFFFFFFFFFFFFFFFF;
            fdstat.fs_rights_inheriting = 0;
        }
        3 => {
            // root inode
            fdstat.fs_filetype = 3; // Directory
            fdstat.fs_flags = 0;
            fdstat.fs_rights_base = 0xFFFFFFFFFFFFFFFF;
            fdstat.fs_rights_inheriting = 0xFFFFFFFFFFFFFFFF;
        }
        _ => (),
    }
    // Todo: 从表中寻找fd
    // let msFdStat = table.find(fd);
    // fdstat.fs_filetype = match msFdStat.kind {
    //     0 => 4, // 0 代表File，4代表RegularFile
    //     1 => 3  // 1 代表Dir，3代表Directory
    // };
    // fdstat.fs_flags = msFdStat.flags;
    // fdstat.fs_rights_base = msFdStat.fs_rights_base;
    // fdstat.fs_rights_inheriting = msFdStat.fs_rights_inheriting;
    if fd == 4 || fd == 5 || fd == 6 || fd == 7 || fd == 8 {
        // 假设前面几个都打开的文件
        fdstat.fs_filetype = 4;
        fdstat.fs_flags = 0;
        fdstat.fs_rights_base = 0xFFFFFFFFFFFFFFFF;
        fdstat.fs_rights_inheriting = 0xFFFFFFFFFFFFFFFF;
    }

    let ret = (&fdstat) as *const _ as usize;
    let ret = unsafe {
        core::slice::from_raw_parts(ret as *const u8, core::mem::size_of::<WasiFdstat>())
    };
    mem.store(retptr, core::mem::size_of::<WasiFdstat>(), ret)?;

    Ok(0)
}

pub fn fd_fdstat_set_flags(mut _ctx: FuncContext<'_>, args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_fdstat_set_flags");
        println!("args: fd: {:?}, flag: {:?}", args.0 as u32, args.1 as u16);
    }

    let _fd = args.0 as u32;
    let _flag = args.1 as u16;

    Ok(0)
}

pub fn fd_filestat_get(_: FuncContext<'_>, _args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_filestat_get");
        println!("args: fd: {:?}, buf: {:?}", _args.0, _args.1);
    }

    Ok(0)
}

pub fn fd_filestat_set_size(_: FuncContext<'_>, _args: (i32, i64)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_filestat_set_size");
        println!("args: fd: {:?}, st_size: {:?}", _args.0, _args.1);
    }

    Ok(0)
}

pub fn fd_prestat_get(mut ctx: FuncContext<'_>, args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_prestat_get");
        println!("args: fd: {:?}, retptr: {:?}", args.0, args.1);
    }
    let fd = args.0 as u32;
    let retptr = args.1 as usize;
    let mut mem = ctx.exported_memory_mut("memory")?;

    match fd {
        3 => {
            // root inode
            let prestat = WasiPrestatT {
                tag: 0, // tag 应为 0，表示这是一个目录，非0表示unknown
                u: WasiPrestatUt {
                    dir: WasiPrestatDir {
                        // dirname_len: "/".len() as u32 + 1, // +1以防止递归错误
                        dirname_len: "/".len() as u32,
                    },
                },
            };

            let ret = (&prestat) as *const _ as usize;
            let ret = unsafe {
                core::slice::from_raw_parts(ret as *const u8, core::mem::size_of::<WasiPrestatT>())
            };
            mem.store(retptr, core::mem::size_of::<WasiPrestatT>(), ret)?;

            Ok(0) // Success
        }
        // Todo: libos需要维护一个表，从表中找fd，找不到就返回Badf
        _ => Ok(8), // Badf
    }
}

pub fn fd_prestat_dir_name(
    mut ctx: FuncContext<'_>,
    args: (i32, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_prestat_dir_name");
        println!(
            "args: fd: {:?}, path_addr: {:?}, path_len: {:?}",
            args.0, args.1, args.2
        );
    }

    let fd = args.0 as u32;
    let path = args.1 as u32;
    let path_len = args.2 as u32;
    let mut mem = ctx.exported_memory_mut("memory")?;

    // Todo: 从表中寻找fd
    if fd == 3 {
        let name = "/";
        mem.store(path as usize, path_len as usize, name.as_bytes())?;

        Ok(0)
    } else {
        Ok(61) // Overflow
    }
}

pub fn fd_read(mut ctx: FuncContext<'_>, args: (i32, i32, i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_read");
        println!(
            "args: fd: {:?}, iovs_ptr: {:?}, iovs_len: {:?}, retptr: {:?}",
            args.0, args.1, args.2, args.3
        );
    }
    let fd = args.0 as u32;
    let iovs_ptr = args.1 as usize;
    let iovs_len = args.2 as usize;
    let retptr = args.3 as usize;

    let mut mem = ctx.exported_memory_mut("memory")?;
    let mut read_size: usize = 0;

    for i in 0..iovs_len {
        let offset = iovs_ptr + i * core::mem::size_of::<WasiCiovec>();
        let iovs: &[u8] = mem.load(offset, core::mem::size_of::<WasiCiovec>())?;
        let iovs: &WasiCiovec = unsafe { &*(iovs.as_ptr() as *const WasiCiovec) };
        let buf: &[u8] = mem.load(iovs.buf as usize, iovs.buf_len as usize)?;
        let buf: &mut [u8] = unsafe {
            slice::from_raw_parts_mut(buf.as_ptr() as usize as *mut u8, iovs.buf_len as usize)
        };
        read_size += libos!(read(fd, buf)).unwrap();
    }

    #[cfg(feature = "log")]
    println!("read_size: {:?}", read_size);
    mem.store(
        retptr,
        core::mem::size_of::<usize>(),
        &read_size.to_ne_bytes(),
    )?;

    Ok(0)
}

pub fn fd_readdir(
    mut _ctx: FuncContext<'_>,
    _args: (i32, i32, i32, i64, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_readdir");
        println!(
            "args: fd: {:?}, buf: {:?}, buf_len: {:?}, cookie: {:?}, bufused: {:?}",
            _args.0, _args.1, _args.2, _args.3, _args.4
        );
    }

    Ok(0)
}

pub fn fd_seek(mut _ctx: FuncContext<'_>, _args: (i32, i64, i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_seek");
        println!(
            "args: fd: {:?}, offset: {:?}, whence: {:?}, pos: {:?}",
            _args.0, _args.1, _args.2, _args.3
        );
    }

    // TO BE FIX FOR PY HELLO
    // let fd = args.0 as u32;
    // let offset = args.1;
    // let whence = args.2;
    // let pos = offset as u32;
    // // if whence == 0 {

    // // } else if whence == 1 {

    // // } else if whence == 2{

    // // }

    // libos!(lseek(fd, pos)).unwrap();

    Ok(0)
}

pub fn fd_sync(_: FuncContext<'_>, _args: i32) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_sync");
        println!("args: fd: {:?}", _args);
    }
    Ok(0)
}

pub fn fd_write(mut ctx: FuncContext<'_>, args: (i32, i32, i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into fd_write");
        println!(
            "args: fd: {:?}, iovs_ptr: {:?}, iovs_len: {:?}, retptr: {:?}",
            args.0, args.1, args.2, args.3
        );
    }
    let fd = args.0 as u32;
    let iovs_ptr = args.1 as usize;
    let iovs_len = args.2 as usize;
    let retptr = args.3 as usize;

    let mut mem = ctx.exported_memory_mut("memory")?;
    let mut write_size: usize = 0;

    for i in 0..iovs_len {
        let offset = iovs_ptr + i * core::mem::size_of::<WasiCiovec>();
        let iovs: &[u8] = mem.load(offset, core::mem::size_of::<WasiCiovec>())?;
        let iovs: &WasiCiovec = unsafe { &*(iovs.as_ptr() as *const WasiCiovec) };
        let buf = mem.load(iovs.buf as usize, iovs.buf_len as usize)? as &[u8];
        write_size += libos!(write(fd, buf)).unwrap();
    }

    #[cfg(feature = "log")]
    println!("write_size: {:?}", write_size);
    mem.store(
        retptr,
        core::mem::size_of::<usize>(),
        &write_size.to_ne_bytes(),
    )?;
    Ok(0)
}

pub fn path_create_directory(
    _ctx: FuncContext<'_>,
    _args: (i32, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_create_directory");
        println!(
            "args: fd: {:?}, path: {:?}, path_len: {:?}",
            _args.0, _args.1, _args.2
        );
    }

    Ok(0)
}

pub fn path_filestat_get(
    _ctx: FuncContext<'_>,
    _args: (i32, i32, i32, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_filestat_get");
        println!(
            "args: fd: {:?}, flags: {:?}, path: {:?}, path_len: {:?}, buf: {:?}",
            _args.0, _args.1, _args.2, _args.3, _args.4
        );
    }

    Ok(0)
}

pub fn path_filestat_set_times(
    _: FuncContext<'_>,
    _args: (i32, i32, i32, i32, i64, i64, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_filestat_set_times");
        println!("args: fd: {:?}, flags: {:?}, path: {:?}, path_len: {:?}, st_atim: {:?}, st_mtim: {:?}, fst_flags: {:?}", _args.0, _args.1, _args.2, _args.3, _args.4, _args.5, _args.6);
    }
    Ok(0)
}

pub fn path_link(
    _ctx: FuncContext<'_>,
    _args: (i32, i32, i32, i32, i32, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_link");
        println!("args: old_fd: {:?}, old_flags: {:?}, old_path: {:?}, old_path_len: {:?}, new_fd: {:?}, new_path: {:?}, new_path_len: {:?}", _args.0, _args.1, _args.2, _args.3, _args.4, _args.5, _args.6);
    }
    Ok(0)
}

pub fn path_open(
    mut ctx: FuncContext<'_>,
    args: (i32, i32, i32, i32, i32, i64, i64, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_open");
        println!("args: fd: {:?}, dirflags: {:?}, path_addr: {:?}, path_len: {:?}, oflags: {:?}, fs_rights_base: {:?}, fs_rights_inheriting: {:?}, fdflags: {:?}, retptr: {:?}", args.0 as u32, args.1 as u32, args.2 as u32, args.3 as u32, args.4 as u16, format!("{:064b}", args.5 as u64), format!("{:064b}", args.6 as u64), args.7 as u16, args.8 as u32);
    }
    let mut mem = ctx.exported_memory_mut("memory")?;
    let _fd = args.0 as u32;
    let _dirflags = args.1 as u32;
    let path_addr = args.2 as u32;
    let path_len = args.3 as u32;
    let oflags = args.4 as u16;

    let fs_rights_base = args.5 as u64;
    let _fs_rights_inheriting = args.6 as u64;
    let fdflags = args.7 as u16;
    let retptr = args.8 as usize;

    let path = mem.load_string(path_addr as usize, path_len as usize)?;
    #[cfg(feature = "log")]
    println!("path: {:?}", path);

    let mut flags: OpenFlags = OpenFlags::empty();
    if (fdflags & 1) == 1 {
        flags |= OpenFlags::O_APPEND;
    }
    if (oflags & 1) == 1 {
        flags |= OpenFlags::O_CREAT;
    }

    let mut mode: OpenMode = OpenMode::empty();
    if ((fs_rights_base >> 1) & 1) == 1 {
        mode |= OpenMode::RD;
    }
    if ((fs_rights_base >> 6) & 1) == 1 {
        mode |= OpenMode::WR;
    }

    let ret_fd = libos!(open(&path, flags, mode)).unwrap() as i32;
    // Todo: 将对应信息添加到table中，结构体struct {name, fs_flags, fs_rights_base, fs_rights_inheriting, kind}
    // 其中，name是路径，fs_flags是Fdflags类型，fs_rights_base和fs_rights_inheriting直接存输入参数。
    // kind表示文件类型，是个enum。eg. File Dir
    #[cfg(feature = "log")]
    println!("ret_fd: {:?}", ret_fd);
    mem.store(retptr, core::mem::size_of::<i32>(), &ret_fd.to_ne_bytes())?;
    Ok(0)
}

pub fn path_readlink(
    _ctx: FuncContext<'_>,
    _args: (i32, i32, i32, i32, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_readlink");
        println!("args: dir_fd: {:?}, path: {:?}, path_len: {:?}, buf: {:?}, buf_len: {:?}, buf_used: {:?}", _args.0, _args.1, _args.2, _args.3, _args.4, _args.5);
    }
    Ok(0)
}

pub fn path_remove_directory(
    _ctx: FuncContext<'_>,
    _args: (i32, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_remove_directory");
        println!(
            "args: fd: {:?}, path: {:?}, path_len: {:?}",
            _args.0, _args.1, _args.2
        );
    }

    Ok(0)
}

pub fn path_rename(
    _ctx: FuncContext<'_>,
    _args: (i32, i32, i32, i32, i32, i32),
) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_rename");
        println!("args: old_fd: {:?}, old_path: {:?}, old_path_len: {:?}, new_fd: {:?}, new_path: {:?}, new_path_len: {:?}", _args.0, _args.1, _args.2, _args.3, _args.4, _args.5);
    }

    Ok(0)
}

pub fn path_unlink_file(_ctx: FuncContext<'_>, _args: (i32, i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into path_unlink_file");
        println!(
            "args: fd: {:?}, path: {:?}, path_len: {:?}",
            _args.0, _args.1, _args.2
        );
    }

    Ok(0)
}

pub fn poll_oneoff(_ctx: FuncContext<'_>, _args: (i32, i32, i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into poll_oneoff");
        println!(
            "args: in_: {:?}, out_: {:?}, nsubscriptions: {:?}, nevents: {:?}",
            _args.0, _args.1, _args.2, _args.3
        );
    }

    Ok(0)
}

pub fn proc_exit(_ctx: FuncContext<'_>, _args: i32) -> tinywasm::Result<()> {
    #[cfg(feature = "log")]
    println!("[Debug] Invoke into proc_exit");

    panic!("normally exit")
}

pub fn random_get(mut ctx: FuncContext<'_>, args: (i32, i32)) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into random_get");
        println!("args: buf: {:?}, buf_len: {:?}", args.0, args.1);
    }

    let mut mem = ctx.exported_memory_mut("memory")?;
    let buf = args.0 as usize;
    let buf_len = args.1 as usize;
    // let seed: u64 = 42;
    let mut lcg = LCG::new(buf as u64);
    let array = lcg.generate_random_u8_slice(buf_len);

    let data: &[u8] = &array;
    mem.store(buf, buf_len, data)?;

    Ok(0)
}

pub fn sched_yield(_ctx: FuncContext<'_>, _args: ()) -> tinywasm::Result<i32> {
    #[cfg(feature = "log")]
    {
        println!("[Debug] Invoke into sched_yield");
    }

    Ok(0)
}
