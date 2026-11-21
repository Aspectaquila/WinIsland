//! Windows toast notifications, if the OS grants this unpackaged app access. Degrades silently otherwise.

use crate::model::{post, Event};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotificationChangedKind};

pub fn start() {
    std::thread::Builder::new()
        .name("notify".into())
        .spawn(|| {
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            }
            let status = match run() {
                Ok(s) => s,
                Err(e) => format!("unavailable: {}", e.message()),
            };
            post(Event::NotifyStatus(status));
            // keep the thread (and the event registration) alive
            if crate::model::now_ms() > 0 {
                loop {
                    std::thread::park();
                }
            }
        })
        .ok();
}

fn run() -> windows::core::Result<String> {
    let l = UserNotificationListener::Current()?;
    let st = l.RequestAccessAsync()?.get()?;
    if st != UserNotificationListenerAccessStatus::Allowed {
        return Ok(format!("denied: {}", st.0));
    }
    let seen = Arc::new(Mutex::new(HashSet::new()));
    for n in l.GetNotificationsAsync(NotificationKinds::Toast)?.get()? {
        seen.lock().unwrap().insert(n.Id()?);
    }
    let l2 = l.clone();
    l.NotificationChanged(&TypedEventHandler::new(move |_, e: windows::core::Ref<windows::UI::Notifications::UserNotificationChangedEventArgs>| {
        if let Some(e) = e.as_ref() {
            if e.ChangeKind()? != UserNotificationChangedKind::Added {
                return Ok(());
            }
            let id = e.UserNotificationId()?;
            if !seen.lock().unwrap().insert(id) {
                return Ok(());
            }
            let Ok(n) = l2.GetNotification(id) else { return Ok(()) };
            let mut texts = vec![];
            if let Ok(b) = n.Notification()?.Visual()?.GetBinding(&KnownNotificationBindings::ToastGeneric()?) {
                for t in b.GetTextElements()? {
                    texts.push(t.Text()?.to_string());
                }
            }
            let app = n.AppInfo().and_then(|a| a.DisplayInfo()).and_then(|d| d.DisplayName()).map(|h| h.to_string()).unwrap_or_default();
            post(Event::Notification {
                app,
                title: texts.first().cloned().unwrap_or_default(),
                body: texts.get(1).cloned().unwrap_or_default(),
            });
        }
        Ok(())
    }))?;
    std::mem::forget(l);
    Ok("ok".into())
}
