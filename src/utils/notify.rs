//! 桌面通知：调用系统的 `notify-send`（libnotify）发出系统级弹窗。
//!
//! 只做「尽力而为」：进程在独立线程里发送，不阻塞主线程；找不到 `notify-send`
//! 或系统没有通知守护进程时只写 stderr，由 `app` 层的 AdwToast 兜底。
//! 软件关闭后不再有任何代码在跑，因此关闭期间插入手柄不会产生通知。

/// 发出一条系统通知（异步，调用立即返回）。
pub fn desktop(summary: &str, body: &str, icon: &str) {
    let summary = summary.to_string();
    let body = body.to_string();
    let icon = icon.to_string();
    std::thread::spawn(move || {
        let result = std::process::Command::new("notify-send")
            .arg("--app-name=Proton 启动管理器")
            .arg(format!("--icon={icon}"))
            .arg(&summary)
            .arg(&body)
            .output();
        match result {
            Ok(out) if out.status.success() => eprintln!("[通知] {summary} — {body}"),
            Ok(out) => eprintln!(
                "[通知失败] {} {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            Err(e) => eprintln!("[通知失败] 无法执行 notify-send: {e}"),
        }
    });
}
