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

// Windows 세션 콜백에서 메인 이벤트 루프로 신호 전달 목적
static EVENT_PROXY: OnceLock<EventLoopProxy<AppEvent>> = OnceLock::new();

pub struct ExecutionStateController {
    tx: mpsc::Sender<bool>,
}

impl ExecutionStateController {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<bool>();

        thread::spawn(move || {
            // SetThreadExecutionState 호출 스레드 고정 목적
            while let Ok(active) = rx.recv() {
                apply_execution_state(active);
            }

            // 종료 시 기본 전원 정책 복원 목적
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
        // 메인 스레드가 초기화 실패를 즉시 감지할 수 있도록 준비 결과 전달
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
    // 화면과 시스템 절전 방지를 함께 제어하는 플래그 조합
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
        // Win+L 잠금과 잠금 해제를 내부 이벤트로 변환
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
        // 화면에 표시되지 않는 메시지 전용 윈도우 생성
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
        // 세션 변경 메시지 수신 대기와 디스패치 루프
        let message_state = unsafe { GetMessageW(&mut message, None, 0, 0).0 };

        if message_state <= 0 {
            break;
        }

        unsafe {
            DispatchMessageW(&message);
        }
    }

    unsafe {
        // 세션 알림 해제와 임시 윈도우 정리
        let _ = WTSUnRegisterSessionNotification(context.hwnd);
        let _ = DestroyWindow(context.hwnd);
        let _ = UnregisterClassW(SESSION_MONITOR_CLASS_NAME, Some(context.hinstance));
    }
}
