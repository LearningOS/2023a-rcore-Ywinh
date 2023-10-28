# lab1总结

目的就是实现三个信息的统计

## status: TaskStatus,

* 按照提示直接设置为running

## syscall_times: [u32; MAX_SYSCALL_NUM]

* 第一次尝试直接在sys_task_info来加载，发现好像不行，因为不知道传入的ti: *mut TaskInfo，这个参数到底在哪里被初始化的，而且每个任务都需要有一个syscall_times数组
* 由此我在`TaskControBlock`中维护一个`pub task_syscall_times: [u32; MAX_SYSCALL_NUM]`数组，这样通过全局遍历TASK_MANAGER可以很好的在每次系统调用时更新
* 更新位置在`trap_handler`进入`syscall之前`，读取x17寄存器为syscall id

## time: usize

* 需要得到的是从第一次运行到现在的时间，现在的时间可以通过`get_time_ms`直接获得
* 第一次运行开始的时间，需要在应用第一次变成Running态的时候记载，因此我们为每个`TaskControBlock`中维护
  * `pub task_start: usize,` 记录任务第一次开始的时间
  * `pub task_flag: bool,` 标志是否为第一次，如果是就是false，然后我们更新`task_start`，并且将该变量置为false，保证只记录一次start time