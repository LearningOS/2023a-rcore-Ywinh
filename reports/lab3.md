# ch5

## 继承上一章修改
今天上下午一直在移植代码，尝试了`git cherry-pick`试了很久，重置过去重置过来，问了gpt，看了b站，csdn都无果，就是没有合并，只显示reports文件夹有冲突，主要的os没有，遂还是采用`git diff`打patch的笨方法，冲突太多了，合并了小一个小时。

## 修理waitpid
移植好之后，`make run`确实能跑了，但是随便输一个就报错，说`waitpid`清除僵尸进程的引用计数有错，本来应该是1，结果是2，多了一个，debug找不出来，println也没看出来在哪里。仔细想想，找了跟`Arc`有关的所有代码，可以肯定一件事，模板代码一定没问题，那问题就出在我自己移植过来的代码，最后一个个注释排除法，找到了原来是我自己用了一个Arc没有drop，我以为drop了inner的RefMut就可以了，没想到这个也要drop。为啥这个不会自动drop呢？

目前还有usertest卡住的问题，再看看。

## spawn
通过注释发现卡住的原因是spawn的实现有问题，重点在维护父子关系，注意`drop`的位置

* spawn就是新建一个进程而已，不要想着用fork+exec，之前直接调用fork()和exec()会出问题，也不好调试，于是自己仿照fork内容与exec自己实现

## stride
stride感觉倒是很简单，根据提示BIG_STRIDE需要大一点，于是把BIG_STRIDE设置为了0x100000，然后每次调度的时候，都要fetch_task，于是在这里找出最小的stride返回，pass的维护在set_piro里面实现，因为prio只会在这里修改