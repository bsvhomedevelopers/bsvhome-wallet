use std::time::Duration;

pub async fn sleep(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    let (sender, receiver) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        let _ = sender.send(());
    });
    let _ = receiver.await;
}
