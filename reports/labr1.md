# lab1

## 修改hello测例

首先运行hello后，发现输出`Incorrect argc`，点进hello.c里面查看，发现是`argc`传入不对，结合实验书测例库里面，对于c语言和rCore的用户栈排布不一样，推测应该是这个原因，造成c语言里面传入的argc不是想要的参数

于是阅读ch7的命令行参数这一章节，了解了sys_exec是如何把命令行参数压入用户栈，以及用户库如何从用户栈上还原命令行参数

那么接下来就是找出C语言规定栈和rCore栈的区别了，为了方便把指导书上栈展示的顺序从低到高改为从高到低

```
position            content                     size (bytes)
  ------------------------------------------------------------------------
  stack pointer ->  [ argc = number of args ]     8
                    [ argv[0] (pointer) ]         8 (program name)
                    [ argv[1] (pointer) ]         8
                    [ argv[..] (pointer) ]        8 * x
                    [ argv[n - 1] (pointer) ]     8
                    [ argv[n] (pointer) ]         8   (= NULL)

                    [ argv[0] ]                   >=0 (program name)
                    [ '\0' ]                      1
                    [ argv[..] ]                  >=0
                    [ '\0' ]                      1
                    [ argv[n - 1] ]               >=0
                    [ '\0' ]                      1

  ------------------------------------------------------------------------
```

<img src="https://cdn.jsdelivr.net/gh/Ywinh/TyporaImages@main/202311142343823.png" alt="image-20231114234330727" style="zoom: 50%;" />

<img src="https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/user-stack-cmdargs.png" alt="../_images/user-stack-cmdargs.png" style="zoom: 67%;" />

可以发现**黄色部分和蓝色部分顺序是反的**，因此我们思路就有了，找到方法交换这两部分

修改后代码

```rust
 	//首先预留好所有的位置,从后往前预留,其实顺序无关紧要，减去的总数都是一样的
        for i in (0..args.len()).rev() {
            user_sp -= args[i].len() + 1;
        }
        let argv_end = user_sp;
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        //准备写入，user_sp目前指向argv数组的起始地址
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    new_token,
                    (user_sp + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        *argv[args.len()] = 0;

        let mut temp_ptr = argv_end;
        for i in 0..args.len() {
            *argv[i] = temp_ptr;
            let mut p = temp_ptr;
            for c in args[i].as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(new_token, p as *mut u8) = 0;
            temp_ptr += args[i].len() + 1;
        }

        //写入argc
        user_sp -= core::mem::size_of::<usize>();
        *translated_refmut(new_token, user_sp as *mut isize) = args.len() as isize;
		...
		...
		// trap_cx.x[11] = argv_base;
        trap_cx.x[11] = user_sp + core::mem::size_of::<usize>();
```

有一个很重要的点是，不要对齐user_sp，如果对齐了user_sp，那么在传入a1的时候，就不能这样赋值`user_sp + core::mem::size_of::<usize>()`，对齐后user_sp已经不是最初的版本了，会出现错误，一个解决方式是事先保存，不过rCore上面说不对齐对qemu没有影响，我就先不管了。

**实验结果**

![image-20231114234248317](https://cdn.jsdelivr.net/gh/Ywinh/TyporaImages@main/202311142342402.png)



## 问答题：elf与bin的区别

```
ch6_file0.elf: ELF 64-bit LSB executable, UCB RISC-V, RVC, double-float ABI, version 1 (SYSV), statically linked, stripped
ch6_file0.bin: data
```

 ELF 格式执行文件经过 `objcopy` 工具丢掉所有 ELF header 和符号变为二进制镜像文件bin

elf里面含有不少其他信息，程序头之类的，但是bin里面只有纯数据