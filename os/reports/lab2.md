直接<<12直接这样会报错overflow，但是那个函数确实就是干了这个事情，只是我帮他弄了一把，很奇怪，还是最后用函数了

taskInfo报错，按照群里大佬这样修改，但不知道为什么这样修改

```rust
//原
pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / MICRO_PER_SEC)
}
//修改为
pub fn get_time_us() -> usize {
    time::read() * MICRO_PER_SEC / CLOCK_FREQ
}

```

### 疑问

1. `vpn_end`计算有问题，len需要/8吗：不需要，因为VA就是取最低39位，不会左移右移啥的
2. 上取整，如果已经对齐的情况下还会上取整吗：回答，不会



### bug与问题

1. 对于判断是否mapped过，只考虑了`find_pte`不能为`None`，没有考虑`find_pte`存在，但是`pte.is_valid()`不合法这件事，卡了很久，也不好调试
2. MapPermission不好进行零初始化，那么就用match，但是match要解决穷尽匹配，我们先把不合法的删去，然后最后一个_只代表`6`的情况
3. 对题意理解有问题，在mmap中，我以为如果start和end之间有已经被映射的页，我们还是需要分配len这么长，也就是不error，映射一段不连续的虚拟内存，写了比较复杂，后面才知道直接error
4. 这章很难debug，看样子甚至是多线程跑测试，所以花费很多时间