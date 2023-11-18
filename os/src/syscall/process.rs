//! Process management syscalls

use crate::{
    config::{BIG_STRIDE, MAIL_BUF_SIZE, MAX_SYSCALL_NUM, PAGE_SIZE},
    fs::{open_file, OpenFlags},
    mm::{
        translate_va2pa, translated_ref, translated_refmut, translated_str, MapPermission,
        VirtAddr, VirtPageNum,
    },
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next, pid2task,
        suspend_current_and_run_next, SignalAction, SignalFlags, TaskControlBlock, TaskStatus,
        MAX_SIG,
    },
    timer::get_time_us,
};
use alloc::{string::String, sync::Arc, vec::Vec};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    let mut args_vec: Vec<String> = Vec::new();
    loop {
        let arg_str_ptr = *translated_ref(token, args);
        if arg_str_ptr == 0 {
            break;
        }
        args_vec.push(translated_str(token, arg_str_ptr as *const u8));
        unsafe {
            args = args.add(1);
        }
    }
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        let argc = args_vec.len();
        task.exec(all_data.as_slice(), args_vec);
        // return argc because cx.x[10] will be covered with it later
        argc as isize
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

pub fn sys_kill(pid: usize, signum: i32) -> isize {
    trace!("kernel:pid[{}] sys_kill", current_task().unwrap().pid.0);
    if let Some(task) = pid2task(pid) {
        if let Some(flag) = SignalFlags::from_bits(1 << signum) {
            // insert the signal if legal
            let mut task_ref = task.inner_exclusive_access();
            if task_ref.signals.contains(flag) {
                return -1;
            }
            task_ref.signals.insert(flag);
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    let us = get_time_us();
    let va = VirtAddr::from(ts as usize);
    let pa = translate_va2pa(current_user_token(), va);
    let ptr = pa.get_mut();

    (*ptr) = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };

    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!(
        "kernel:pid[{}] sys_task_info NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    -1
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");

    //syscall ID：222
    //申请长度为 len 字节的物理内存（不要求实际物理内存位置，可以随便找一块），将其映射到 start 开始的虚存，内存页属性为 port
    if len == 0 {
        return 0;
    }
    if port & 0x7 == 0 || port & !0x7 != 0 || start % PAGE_SIZE != 0 {
        return -1;
    }
    //println!("start:{},len:{}1", start, len);
    //let page_table = PageTable::from_token(current_user_token());

    //必须要解决穷尽匹配的问题,以下表示这段映射的内存页属性
    let permission = match port {
        1 => MapPermission::R,
        2 => MapPermission::W,
        3 => MapPermission::R | MapPermission::W,
        4 => MapPermission::X,
        5 => MapPermission::R | MapPermission::X,
        6 => MapPermission::W | MapPermission::X,
        _ => MapPermission::R | MapPermission::W | MapPermission::X,
    };
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let memory_set_mut = &mut inner.memory_set;

    let start_vpn_floor = VirtAddr::from(start).floor();
    let end_vpn_ceil = VirtAddr::from(start + len).ceil();

    for i in start_vpn_floor.0..end_vpn_ceil.0 {
        //不能出现有mapped过的
        if memory_set_mut.page_table.mapped_valid(VirtPageNum::from(i)) {
            return -1;
        }
    }

    memory_set_mut.insert_framed_area(
        VirtAddr::from(start_vpn_floor),
        VirtAddr::from(end_vpn_ceil),
        permission | MapPermission::U,
    );

    0
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");

    if start % PAGE_SIZE != 0 || len == 0 {
        return -1;
    }
    //let mut page_table = PageTable::from_token(current_user_token());

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let memory_set_mut = &mut inner.memory_set;

    let start_vpn_floor = VirtAddr::from(start).floor();
    let end_vpn_ceil = VirtAddr::from(start + len).ceil();

    for i in start_vpn_floor.0..end_vpn_ceil.0 {
        //不能出现没有mapped过的
        if memory_set_mut.page_table.mapped_valid(VirtPageNum::from(i)) {
            //println!("ok{}", i);
        } else {
            return -1;
        }
    }

    for i in start_vpn_floor.0..end_vpn_ceil.0 {
        memory_set_mut.page_table.unmap(VirtPageNum::from(i));
    }

    0
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    //模仿TaskControBlock::new，fork,和sysfork，以及task/mod里面的维护父子关系
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let new_task = Arc::new(TaskControlBlock::new(all_data.as_slice()));
        let mut new_task_inner = new_task.inner_exclusive_access();
        let pid = new_task.getpid();

        let current_task = current_task().unwrap();
        let mut current_task_inner = current_task.inner_exclusive_access();

        //建立父子关系
        new_task_inner.parent = Some(Arc::downgrade(&current_task));
        current_task_inner.children.push(new_task.clone());
        drop(new_task_inner);

        //好像不需要exec，因为entry point设置了
        //task_control_block.exec(data);
        //大问题，忘记add_task了，哭
        add_task(new_task);

        return pid as isize;
    } else {
        return -1;
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio < 2 {
        return -1;
    }
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.priority = _prio as usize;
    inner.pass = BIG_STRIDE / inner.priority;

    _prio
}

pub fn sys_sigprocmask(mask: u32) -> isize {
    trace!(
        "kernel:pid[{}] sys_sigprocmask",
        current_task().unwrap().pid.0
    );
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        let old_mask = inner.signal_mask;
        if let Some(flag) = SignalFlags::from_bits(mask) {
            inner.signal_mask = flag;
            old_mask.bits() as isize
        } else {
            -1
        }
    } else {
        -1
    }
}

pub fn sys_sigreturn() -> isize {
    trace!(
        "kernel:pid[{}] sys_sigreturn",
        current_task().unwrap().pid.0
    );
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        inner.handling_sig = -1;
        // restore the trap context
        let trap_ctx = inner.get_trap_cx();
        *trap_ctx = inner.trap_ctx_backup.unwrap();
        // Here we return the value of a0 in the trap_ctx,
        // otherwise it will be overwritten after we trap
        // back to the original execution of the application.
        trap_ctx.x[10] as isize
    } else {
        -1
    }
}

fn check_sigaction_error(signal: SignalFlags, action: usize, old_action: usize) -> bool {
    if action == 0
        || old_action == 0
        || signal == SignalFlags::SIGKILL
        || signal == SignalFlags::SIGSTOP
    {
        true
    } else {
        false
    }
}

pub fn sys_sigaction(
    signum: i32,
    action: *const SignalAction,
    old_action: *mut SignalAction,
) -> isize {
    trace!(
        "kernel:pid[{}] sys_sigaction",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if signum as usize > MAX_SIG {
        return -1;
    }
    if let Some(flag) = SignalFlags::from_bits(1 << signum) {
        if check_sigaction_error(flag, action as usize, old_action as usize) {
            return -1;
        }
        let prev_action = inner.signal_actions.table[signum as usize];
        *translated_refmut(token, old_action) = prev_action;
        inner.signal_actions.table[signum as usize] = *translated_ref(token, action);
        0
    } else {
        -1
    }
}

pub fn sys_mail_read(buf: *mut u8, mut len: usize) -> isize {
    println!("[sys_mail_read]:input_len is {}", len);
    if len > MAIL_BUF_SIZE {
        len = MAIL_BUF_SIZE;
    }
    let token = current_user_token();
    let task = current_task().unwrap();
    let mut inner = task.inner.exclusive_access();

    //buf无效，len为0，邮箱空，都返回1
    if len == 0 {
        if inner.mails.len() == 0 {
            return -1;
        } else {
            return 0;
        }
    }
    if buf == core::ptr::null_mut() || inner.mails.len() == 0 {
        return -1;
    }
    //开始读取
    let mail = inner.mails.pop_front().unwrap();
    if len > mail.len() {
        len = mail.len();
    }
    println!("[sys_mail_read]:read len is {}", len);

    //println!("read len is {}", arr.len);
    for i in 0..len {
        let buf_ptr = translated_refmut(token, unsafe { buf.add(i) });
        *buf_ptr = mail.as_bytes()[i];
    }

    len as isize
}

pub fn sys_mail_write(pid: usize, buf: *mut u8, mut len: usize) -> isize {
    println!("[sys_mail_write]:len is {}", len);
    if len > MAIL_BUF_SIZE {
        len = MAIL_BUF_SIZE;
    }
    let token = current_user_token();
    if pid2task(pid).is_none() {
        return -1;
    }
    let task_dest = pid2task(pid).unwrap();
    let mut inner = task_dest.inner.exclusive_access();
    //let mut mails = &inner.mails;
    //let buf_ptr = translated_refmut(token, buf);
    if len == 0 {
        if inner.mails.len() == 16 {
            return -1;
        } else {
            return 0;
        }
    }
    if buf == core::ptr::null_mut() || inner.mails.len() == 16 {
        return -1;
    }

    let mut s = translated_str(token, buf);
    while s.len() > len {
        s.remove(len);
    }

    println!("write len is {}", s.len());
    inner.mails.push_back(s);

    len as isize
}
