# lab2

## 编程作业1：跟着教程做

主要是要用引入的mod里的用户栈初始函数来替换lab1写的，具体如下，调用几个引入的mod里的api

```rust
        let elf = ElfLoader::new(elf_data).unwrap();
        user_sp = elf.init_stack(new_token, user_sp, args.clone());
```

## 编程作业2：添加syscall

根据教程，每步出现`Unsupported syscall_id: 29`这种报错，我们采用以下步骤初步处理

1. 在`syscall/mod.rs`里添加对应的`syscall_id`常量
2. 查询对应id的syscall，通过[此网页](https://jborza.com/post/2021-05-11-riscv-linux-syscalls/)
3. 重点关注`DESCRIPTION`,`RETURN VALUE`,`ERROR`，适当取舍实现

通过报错我们依次处理了三个syscall

### 29 sys_ioctl

尝试直接0返回，但是没有输出`hellostd`，尝试直接搬过来sys_write，但是失败（不知道怎么实现）

进汇编看一下调用ioctl的时候，传了哪些参数，因为翻看手册对里面写的`request`参数感觉很模糊，不知道要干嘛

在ecall前，传入的参数为

![image-20231115140405483](https://cdn.jsdelivr.net/gh/Ywinh/TyporaImages@main/202311151404589.png)

后来发现这个地方**直接0返回就行**，因为我把`writev`看成了`readv`，因此输出`hellostd`这个活应该是在`writev`这里干的

### 66 sys_writev

> 这里一开始看错了，把66看成了readv，导致全部syscall改完之后，以为唯一需要更改的是ioctl这个，但是无从下手，事实上readv,writev这两个对应的手册就是一样的...

#### 思考过程

发现直接调用`sys_write`好像不行，报错

```
xrzr|r~r[kernel] Panicked at src/fs/stdio.rs:55 called `Result::unwrap()` on an `Err` value: Utf8Error { valid_up_to: 0, error_len: Some(1) }
```

转回去看手册，`writev`就是从几个地址写入`fd`，写入`iovcnt`次，因此思路就是调用`iovcnt`次`sys_write`，但是为了保险起见，我还是没有使用嵌套的系统调用，复制粘贴修改了一下`sys_write`写到`sys_writev`中

```c
ssize_t writev(int fd, const struct iovec *iov, int iovcnt);
```

去查阅了一下`musl`源码，全局搜索`iovec`，得到`iovec`结构体的定义

```c 
STRUCT iovec { void *iov_base; size_t iov_len; };
```

改写到rust中

```rust
/os/src/syscall/mod.rs

/// Iovec
pub struct Iovec {
    /// base addr
    pub iov_base: *const u8,
    /// len
    pub iov_len: usize,
}
```

#### 代码实现

```rust
/// writev syscall
pub fn sys_writev(fd: usize, buf: *const Iovec, iovcnt: usize) -> isize {
    // println!("fd is {}", fd);
    // println!("buf is {:?}", buf);
    // println!("iovcnt is {}", iovcnt);
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    let mut write_num = 0;
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        for i in 0..iovcnt {
            let iov_ptr = translated_ref(token, buf.wrapping_add(i));
            write_num += file.write(UserBuffer::new(translated_byte_buffer(
                token,
                (*iov_ptr).iov_base,
                (*iov_ptr).iov_len,
            ))) as isize;
        }
    } else {
        return -1;
    }
    write_num
}
```

自己第一次没思考清晰的地方

* 在这个函数里面传入了`Iovec`结构体的指针，这是个虚拟地址，需要先得到它的物理地址才能访问这个结构体里面存的东西，对应`translated_ref`
* 结构体内存的是一个虚拟地址和一个usize，因此这个虚拟地址还需要再次翻译才能正确写入，对应`translated_byte_buffer`

**最终结果**

![image-20231115160854543](https://cdn.jsdelivr.net/gh/Ywinh/TyporaImages@main/202311151608600.png)



### 94 sys_exitgroup

> 退出一个进程的所有线程

尝试直接0返回，成功



## 问答作业

options，调用可直接按位或

![image-20231115171046087](https://cdn.jsdelivr.net/gh/Ywinh/TyporaImages@main/202311151710134.png)