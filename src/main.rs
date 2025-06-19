use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::exit;

use tokio::net::TcpListener;
use tracing::{event, Level};
use tracing_subscriber;
/*
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
};
*/

/// PARAS: 存储命令行参数的全局变量
/// api::configure: 路由
use chatsong::{
    parse_paras::PARAS,
    api::configure,
    ctrlc::wait_for_signal,
};

/// 参考：https://github.com/joelparkerhenderson/demo-rust-axum

#[tokio::main]
async fn main() {
    // 监听`ctrl-c`，在停止程序之前保存图结构和各uuid的chat记录
    tokio::spawn(async {
        wait_for_signal().await;
        exit(1);
    });

    // Start tracing
    //tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).init();
    tracing_subscriber::fmt().with_max_level(Level::INFO).init(); // 限制输入级别，如果是TRACE则全部输出，会有很多信息，尤其联网搜索时特别多，这里限制为INFO，即INFO、WARN、ERROR的信息才输出，https://github.com/tokio-rs/tracing/blob/master/examples/examples/hyper-echo.rs

    // 测试不同Level（TRACE、DEBUG、INFO、WARN、ERROR），可以比较，TRACE最高，ERROR最低，越高则有越多的verbose
    //event!(Level::TRACE, "Running on http://{}:{}", PARAS.addr_str, PARAS.port); // 紫色，very low priority, often extremely verbose, information. The most fine-grained information, useful for detailed debugging.
    //event!(Level::DEBUG, "Running on http://{}:{}", PARAS.addr_str, PARAS.port); // 蓝色，lower priority information. Useful during development for debugging problems.
    event!(Level::INFO, "Running on http://{}:{}", PARAS.addr_str, PARAS.port);  // 绿色，useful information. General operational information about the state of the application.
    //event!(Level::WARN, "Running on http://{}:{}", PARAS.addr_str, PARAS.port);  // 黄绿色，hazardous situations. Indication of issues that are not critical but might lead to problems.
    //event!(Level::ERROR, "Running on http://{}:{}", PARAS.addr_str, PARAS.port); // 黄色，very serious errors. Critical problems that need immediate attention.

    // 定义监听地址和端口
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(PARAS.addr[0], PARAS.addr[1], PARAS.addr[2], PARAS.addr[3])), PARAS.port);
    // 创建路由
    let router = configure();
    // 开启TCP，也可以直接使用字符串：let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            println!("{}", e); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
            exit(1);
        },
    };
    // 开启http服务
    if let Err(e) = axum::serve(listener, router.into_make_service()).await {
        println!("{}", e); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
        exit(1);
    }
}
