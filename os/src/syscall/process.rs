//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    config::{BIG_STRIDE, MAX_SYSCALL_NUM, PAGE_SIZE},
    loader::get_app_data_by_name,
    mm::{translate_va2pa, MapPermission, VirtAddr, VirtPageNum},
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskControlBlock, TaskStatus,
    },
    timer::{get_time_ms, get_time_us},
};

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

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
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

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!(
        "kernel::pid[{}] sys_waitpid [{}]",
        current_task().unwrap().pid.0,
        pid
    );
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
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");

    let va = VirtAddr::from(ti as usize);
    let pa = translate_va2pa(current_user_token(), va);
    let ptr: &mut TaskInfo = pa.get_mut();

    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();

    (*ptr).status = TaskStatus::Running;
    (*ptr).time = get_time_ms() - inner.task_start;
    (*ptr)
        .syscall_times
        .copy_from_slice(&inner.task_syscall_times);
    0
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
    if let Some(data) = get_app_data_by_name(&path) {
        let new_task = Arc::new(TaskControlBlock::new(data));
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
