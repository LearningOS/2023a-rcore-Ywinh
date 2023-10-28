//! Process management syscalls

#![allow(unused)]
use core::num;

use alloc::vec::{self, Vec};
use riscv::register::fcsr::Flag;

use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    console::print,
    mm::{
        frame_alloc, translate_va2pa, MapPermission, MemorySet, PageTable, PhysAddr, StepByOne,
        VirtAddr, VirtPageNum, KERNEL_SPACE,
    },
    task::{
        change_program_brk, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus, TASK_MANAGER,
    },
    timer::{get_time, get_time_ms, get_time_us},
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
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
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

    unsafe {
        (*ptr) = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        }
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

    let inner = TASK_MANAGER.inner.exclusive_access();
    let current = inner.current_task;

    unsafe {
        (*ptr).status = TaskStatus::Running;
        (*ptr).time = get_time_ms() - inner.tasks[current].task_start;
        (*ptr)
            .syscall_times
            .copy_from_slice(&inner.tasks[current].task_syscall_times);
    }

    0
}

// YOUR JOB: Implement mmap.
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

    let mut inner = TASK_MANAGER.inner.exclusive_access();
    let current = inner.current_task;
    let memory_set_mut = &mut inner.tasks[current].memory_set;

    let start_vpn_floor = VirtAddr::from(start).floor();
    let mut end_vpn_ceil = VirtAddr::from(start + len).ceil();

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

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    if start % PAGE_SIZE != 0 || len == 0 {
        return -1;
    }
    //let mut page_table = PageTable::from_token(current_user_token());

    let mut inner = TASK_MANAGER.inner.exclusive_access();
    let current = inner.current_task;
    let memory_set_mut = &mut inner.tasks[current].memory_set;

    let start_vpn_floor = VirtAddr::from(start).floor();
    let mut end_vpn_ceil = VirtAddr::from(start + len).ceil();

    for i in start_vpn_floor.0..end_vpn_ceil.0 {
        //不能出现没有mapped过的
        if memory_set_mut.page_table.mapped_valid(VirtPageNum::from(i)) {
            println!("ok{}", i);
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
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
