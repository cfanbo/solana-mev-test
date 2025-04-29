通过gRPC监听链交易，分析DEX交易，支持pump.fun 和 raydium 指令。
当前程序只是提供一个运行框架，跟单策略未实现，跟单交易通过 jito bundle 实现。
创建配置文件
```
cp .env.example .env
```

运行脚本

```shell
RUST_LOG=mybot=DEBUG cargo run
```
