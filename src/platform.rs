use std::io;
use std::sync::{OnceLock, mpsc};
use std::thread;

use tao::event_loop::EventLoopProxy;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, RegisterClassW, UnregisterClassW, WINDOW_EX_STYLE, WM_WTSSESSION_CHANGE, WNDCLASSW,
};
use windows::core::w;

use crate::app::{AppEvent, AppResult};

const SESSION_MONITOR_CLASS_NAME: windows::core::PCWSTR = w!("CaffeineSessionMonitor");
const WTS_SESSION_LOCK: u32 = 0x7;
const WTS_SESSION_UNLOCK: u32 = 0x8;

static EVENT_PROXY: OnceLock<EventLoopProxy<AppEvent>> = OnceLock::new();

pub struct ExecutionStateController {
    tx: mpsc::Sender<bool>,
}

impl ExecutionStateController {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<bool>();

        thread::spawn(move || {
            while let Ok(active) = rx.recv() {
                apply_execution_state(active);
            }

            apply_execution_state(false);
        });

        Self { tx }
    }

    pub fn set_active(&self, active: bool) {
        let _ = self.tx.send(active);
    }
}

pub fn register_event_proxy(proxy: EventLoopProxy<AppEvent>) -> AppResult<()> {
    EVENT_PROXY
        .set(proxy)
        .map_err(|_| io::Error::other("이벤트 프록시 중복 초기화"))?;

    Ok(())
}

pub fn spawn_session_monitor() -> AppResult<()> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    thread::spawn(move || {
        let setup_result = create_session_monitor();

        match setup_result {
            Ok(context) => {
                let _ = ready_tx.send(Ok(()));
                run_session_monitor(context);
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
            }
        }
    });

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => Err(io::Error::other(message).into()),
        Err(_) => Err(io::Error::other("세션 모니터 초기화 응답 수신 실패").into()),
    }
}

fn apply_execution_state(active: bool) {
    let state = if active {
        ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
    } else {
        ES_CONTINUOUS
    };

    unsafe {
        let _ = SetThreadExecutionState(state);
    }
}

struct SessionMonitorContext {
    hwnd: HWND,
    hinstance: HINSTANCE,
}

unsafe extern "system" fn session_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE {
        let event = match wparam.0 as u32 {
            WTS_SESSION_LOCK => Some(AppEvent::Lock),
            WTS_SESSION_UNLOCK => Some(AppEvent::Unlock),
            _ => None,
        };

        if let Some(event) = event {
            if let Some(proxy) = EVENT_PROXY.get() {
                let _ = proxy.send_event(event);
            }
        }
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn create_session_monitor() -> AppResult<SessionMonitorContext> {
    let hinstance: HINSTANCE = unsafe { GetModuleHandleW(None)? }.into();

    let window_class = WNDCLASSW {
        lpfnWndProc: Some(session_wnd_proc),
        lpszClassName: SESSION_MONITOR_CLASS_NAME,
        hInstance: hinstance,
        ..Default::default()
    };

    if unsafe { RegisterClassW(&window_class) } == 0 {
        return Err(io::Error::last_os_error().into());
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            SESSION_MONITOR_CLASS_NAME,
            w!(""),
            Default::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance),
            None,
        )?
    };

    unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)? };

    Ok(SessionMonitorContext { hwnd, hinstance })
}

fn run_session_monitor(context: SessionMonitorContext) {
    let mut message = MSG::default();

    loop {
        let message_state = unsafe { GetMessageW(&mut message, None, 0, 0).0 };

        if message_state <= 0 {
            break;
        }

        unsafe {
            DispatchMessageW(&message);
        }
    }

    unsafe {
        let _ = WTSUnRegisterSessionNotification(context.hwnd);
        let _ = DestroyWindow(context.hwnd);
        let _ = UnregisterClassW(SESSION_MONITOR_CLASS_NAME, Some(context.hinstance));
    }
}
