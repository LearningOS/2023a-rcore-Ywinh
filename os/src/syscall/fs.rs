//! File and filesystem-related syscalls

const FD_STDOUT: usize = 1;
use crate::batch::USER_STACK;

const USER_STACK_SIZE: usize = 4096 * 2;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;

/// write buf of length `len`  to a file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel: sys_write");
    match fd {
        FD_STDOUT => {
            //check:buf是否在栈内地址，buf+len是否在栈内，buf是否在app限制范围内，len是否小于APP_SIZE_LIMIT
            if (((buf as usize) >= USER_STACK.get_sp() - USER_STACK_SIZE)
                && ((buf as usize) + len <= USER_STACK.get_sp()))
                || (((buf as usize) >= APP_BASE_ADDRESS)
                    && ((buf as usize) + len <= APP_BASE_ADDRESS + APP_SIZE_LIMIT))
            {
                let slice = unsafe { core::slice::from_raw_parts(buf, len) };
                let str = core::str::from_utf8(slice).unwrap();
                print!("{}", str);
                len as isize
            } else {
                -1 as isize
            }
        }
        _ => -1 as isize,
    }
}
