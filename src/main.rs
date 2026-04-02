#![windows_subsystem = "windows"]

use std::error::Error;
use image::ImageFormat;
use tray_icon::{TrayIconBuilder, Icon, TrayIconEvent, MouseButton};
use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem, MenuEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED};
use std::sync::OnceLock;
use std::thread;

// 1. 컴파일 시점에 아이콘 파일을 바이너리에 포함
const ACTIVE_ICON_DATA: &[u8] = include_bytes!("../assets/active.ico");
const INACTIVE_ICON_DATA: &[u8] = include_bytes!("../assets/inactive.ico");

fn load_icon(data: &[u8]) -> Result<Icon, Box<dyn Error>> {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory_with_format(data, ImageFormat::Ico)?.into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Ok(Icon::from_rgba(icon_rgba, icon_width, icon_height)?)
}

// 2. 윈도우 API 호출로 절전 모드 방지 제어
fn set_caffeine(active: bool) {
    let state = if active {
        ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
    } else {
        ES_CONTINUOUS
    };
    
    unsafe {
        SetThreadExecutionState(state);
    }
}

// 3. 커스텀 이벤트
#[derive(Debug)]
enum AppEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
    Lock,
    Unlock,
}

static EVENT_PROXY: OnceLock<EventLoopProxy<AppEvent>> = OnceLock::new();

// 4. Session Monitor 로직 (Win+L 락 스크린 예외처리)
unsafe extern "system" fn session_wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::WM_WTSSESSION_CHANGE;
    const WTS_SESSION_LOCK: u32 = 0x7;
    const WTS_SESSION_UNLOCK: u32 = 0x8;

    if msg == WM_WTSSESSION_CHANGE {
        match wparam.0 as u32 {
            WTS_SESSION_LOCK => {
                if let Some(proxy) = EVENT_PROXY.get() {
                    let _ = proxy.send_event(AppEvent::Lock);
                }
            }
            WTS_SESSION_UNLOCK => {
                if let Some(proxy) = EVENT_PROXY.get() {
                    let _ = proxy.send_event(AppEvent::Unlock);
                }
            }
            _ => {}
        }
    }
    unsafe { windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn spawn_session_monitor() {
    thread::spawn(|| {
        use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, RegisterClassW, WNDCLASSW, MSG, GetMessageW, DispatchMessageW, HWND_MESSAGE, WS_OVERLAPPEDWINDOW, WINDOW_EX_STYLE
        };
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;

        unsafe {
            let class_name = windows::core::w!("CaffeineSessionMonitor");
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            
            let wc = WNDCLASSW {
                lpfnWndProc: Some(session_wnd_proc),
                lpszClassName: class_name,
                hInstance: hinstance.into(),
                ..Default::default()
            };
            RegisterClassW(&wc);
            
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                windows::core::w!(""),
                WS_OVERLAPPEDWINDOW,
                0, 0, 0, 0,
                Some(HWND_MESSAGE),
                None,
                Some(hinstance.into()),
                None,
            ).unwrap();
            
            let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
            
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                DispatchMessageW(&msg);
            }
        }
    });
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    
    // 백그라운드 락 스크린 모니터 스레드에 보낼 Proxy 세팅
    EVENT_PROXY.set(proxy.clone()).unwrap();
    spawn_session_monitor();

    let active_icon = load_icon(ACTIVE_ICON_DATA)?;
    let inactive_icon = load_icon(INACTIVE_ICON_DATA)?;

    let mut is_active = true;
    let mut is_locked = false;
    
    set_caffeine(is_active);

    let status_menu_item = MenuItem::with_id("status", "현재 상태: 활성화됨", false, None);
    let toggle_menu_item = MenuItem::with_id("toggle", "Caffeine 켜기/끄기", true, None);
    let quit_menu_item = MenuItem::with_id("quit", "종료", true, None);

    let tray_menu = Menu::new();
    tray_menu.append(&status_menu_item)?;
    tray_menu.append(&PredefinedMenuItem::separator())?;
    tray_menu.append(&toggle_menu_item)?;
    tray_menu.append(&quit_menu_item)?;

    let mut tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Caffeine: 켜짐")
        .with_icon(active_icon.clone())
        .build()?;

    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        let _ = tray_proxy.send_event(AppEvent::Tray(event));
    }));

    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = menu_proxy.send_event(AppEvent::Menu(event));
    }));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        let mut toggle_requested = false;
        let mut lock_state_changed = None;

        if let tao::event::Event::UserEvent(app_event) = event {
            match app_event {
                AppEvent::Tray(tray_event) => {
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = tray_event {
                        toggle_requested = true;
                    }
                }
                AppEvent::Menu(menu_event) => {
                    if menu_event.id == toggle_menu_item.id() {
                        toggle_requested = true;
                    } else if menu_event.id == quit_menu_item.id() {
                        set_caffeine(false);
                        *control_flow = ControlFlow::Exit;
                    }
                }
                AppEvent::Lock => {
                    lock_state_changed = Some(true);
                }
                AppEvent::Unlock => {
                    lock_state_changed = Some(false);
                }
            }
        }

        // 잠금/잠금해제 발생 시
        if let Some(locked) = lock_state_changed {
            is_locked = locked;
            if is_locked {
                // 잠긴 상태라면 능동적 상태(is_active)와 무관하게 절전 허용 (API 끄기)
                set_caffeine(false);
            } else {
                // 풀린 상태라면 원래 사용자가 세팅해둔 상태(is_active) 복구
                set_caffeine(is_active);
            }
        }

        // 토글 요청 발생 시
        if toggle_requested {
            is_active = !is_active;
            
            // 만약 현재 잠금 상태가 아니라면 실제로 바로 API 적용
            if !is_locked {
                set_caffeine(is_active);
            }
            
            if is_active {
                let _ = tray_icon.set_icon(Some(active_icon.clone()));
                let _ = tray_icon.set_tooltip(Some("Caffeine: 켜짐"));
                status_menu_item.set_text("현재 상태: 활성화됨");
            } else {
                let _ = tray_icon.set_icon(Some(inactive_icon.clone()));
                let _ = tray_icon.set_tooltip(Some("Caffeine: 꺼짐"));
                status_menu_item.set_text("현재 상태: 비활성화됨");
            }
        }
    });
}
