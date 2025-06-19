//use std::process::exit;

use crate::{
    graph::save_graph,
    info::save_all_chat,
};

/// 监听`ctrl-c`，在停止程序之前之前指定操作
/// 参考这个项目：https://github.com/Finomnis/tokio-graceful-shutdown/blob/main/src/signal_handling.rs
/// https://stackoverflow.com/questions/73681328/graceful-handling-of-sigterm-ctrl-c-and-shutdown-a-threadpool

/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(unix)]
pub async fn wait_for_signal_impl() {
    use tokio::signal::unix::{signal, SignalKind};

    // Infos here:
    // https://www.gnu.org/software/libc/manual/html_node/Termination-Signals.html
    let mut signal_terminate = signal(SignalKind::terminate()).unwrap();
    let mut signal_interrupt = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = signal_terminate.recv() => println!("Received SIGTERM."), // kill
        _ = signal_interrupt.recv() => println!("Received SIGINT."), // ctrl-c
    };
    //println!("do something ...");
    //exit(1);
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(windows)]
pub async fn wait_for_signal_impl() {
    use tokio::signal::windows;

    // Infos here:
    // https://learn.microsoft.com/en-us/windows/console/handlerroutine
    let mut signal_c = windows::ctrl_c().unwrap();
    let mut signal_break = windows::ctrl_break().unwrap();
    let mut signal_close = windows::ctrl_close().unwrap();
    let mut signal_shutdown = windows::ctrl_shutdown().unwrap();

    tokio::select! {
        _ = signal_c.recv() => println!("Received CTRL_C."), // ctrl-c
        _ = signal_break.recv() => println!("Received CTRL_BREAK."),
        _ = signal_close.recv() => println!("Received CTRL_CLOSE."),
        _ = signal_shutdown.recv() => println!("Received CTRL_SHUTDOWN."),
    };
    //println!("do something ...");
    //exit(1);
}

/// Registers signal handlers and waits for a signal that
/// indicates a shutdown request.
pub async fn wait_for_signal() {
    //println!("start waiting signal ...");
    wait_for_signal_impl().await;
    //println!("do something ...");
    save_graph(); // 退出前保存图文件
    save_all_chat(); // 退出前保存每个uuid的chat记录
}
