use std::error::Error;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, TrayIconEvent};

use crate::platform::{ExecutionStateController, register_event_proxy, spawn_session_monitor};
use crate::tray::{TrayUi, TrayUserAction};

pub type AppResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub enum AppEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
    Lock,
    Unlock,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AppState {
    is_active: bool,
    is_locked: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_active: true,
            is_locked: false,
        }
    }
}

impl AppState {
    pub(crate) fn is_active(&self) -> bool {
        self.is_active
    }

    pub(crate) fn tooltip(&self) -> &'static str {
        if self.is_active {
            "Caffeine: 켜짐"
        } else {
            "Caffeine: 꺼짐"
        }
    }

    pub(crate) fn status_text(&self) -> &'static str {
        if self.is_active {
            "현재 상태: 활성화됨"
        } else {
            "현재 상태: 비활성화됨"
        }
    }

    fn effective_active(&self) -> bool {
        // 사용자 설정과 잠금 상태를 함께 반영한 실제 절전 방지 상태
        self.is_active && !self.is_locked
    }

    fn toggle(&mut self) {
        self.is_active = !self.is_active;
    }

    fn set_locked(&mut self, locked: bool) {
        self.is_locked = locked;
    }
}

enum UserAction {
    Toggle,
    SetLocked(bool),
    Quit,
}

pub fn run() -> AppResult<()> {
    // 플랫폼 초기화와 UI 구성을 마친 뒤 단일 이벤트 루프로 진입
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    register_event_proxy(proxy.clone())?;
    spawn_session_monitor()?;

    let execution_state = ExecutionStateController::spawn();
    let mut state = AppState::default();
    execution_state.set_active(state.effective_active());

    let tray_ui = TrayUi::new(&state)?;
    install_event_handlers(proxy);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::UserEvent(app_event) = event {
            handle_user_event(
                app_event,
                &mut state,
                &tray_ui,
                &execution_state,
                control_flow,
            );
        }
    });
}

fn install_event_handlers(proxy: EventLoopProxy<AppEvent>) {
    // 외부 라이브러리 콜백을 애플리케이션 사용자 이벤트로 수렴
    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        let _ = tray_proxy.send_event(AppEvent::Tray(event));
    }));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy.send_event(AppEvent::Menu(event));
    }));
}

fn handle_user_event(
    app_event: AppEvent,
    state: &mut AppState,
    tray_ui: &TrayUi,
    execution_state: &ExecutionStateController,
    control_flow: &mut ControlFlow,
) {
    // 트레이 입력, 메뉴 입력, 세션 변경을 단일 처리 흐름으로 통합
    let Some(action) = resolve_user_action(app_event, tray_ui) else {
        return;
    };

    match action {
        UserAction::Toggle => {
            state.toggle();
            execution_state.set_active(state.effective_active());

            if tray_ui.sync(state).is_err() {
                // 트레이 상태 동기화 실패 시 절전 방지 해제 후 종료
                execution_state.set_active(false);
                *control_flow = ControlFlow::Exit;
            }
        }
        UserAction::SetLocked(locked) => {
            state.set_locked(locked);
            execution_state.set_active(state.effective_active());
        }
        UserAction::Quit => {
            execution_state.set_active(false);
            *control_flow = ControlFlow::Exit;
        }
    }
}

fn resolve_user_action(app_event: AppEvent, tray_ui: &TrayUi) -> Option<UserAction> {
    // 플랫폼 이벤트를 내부 액션으로 정규화
    match app_event {
        AppEvent::Tray(tray_event) => {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = tray_event
            {
                Some(UserAction::Toggle)
            } else {
                None
            }
        }
        AppEvent::Menu(menu_event) => {
            tray_ui
                .resolve_menu_action(&menu_event)
                .map(|action| match action {
                    TrayUserAction::Toggle => UserAction::Toggle,
                    TrayUserAction::Quit => UserAction::Quit,
                })
        }
        AppEvent::Lock => Some(UserAction::SetLocked(true)),
        AppEvent::Unlock => Some(UserAction::SetLocked(false)),
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;

    #[test]
    fn default_state_is_effectively_active() {
        let state = AppState::default();

        assert!(state.is_active);
        assert!(state.effective_active());
    }

    #[test]
    fn lock_disables_effective_state_without_resetting_user_choice() {
        let mut state = AppState::default();

        state.set_locked(true);

        assert!(state.is_active);
        assert!(!state.effective_active());
    }

    #[test]
    fn unlock_restores_effective_state_when_user_choice_is_active() {
        let mut state = AppState::default();

        state.set_locked(true);
        state.set_locked(false);

        assert!(state.effective_active());
    }

    #[test]
    fn inactive_state_stays_inactive_after_unlock() {
        let mut state = AppState::default();

        state.toggle();
        state.set_locked(true);
        state.set_locked(false);

        assert!(!state.is_active);
        assert!(!state.effective_active());
    }
}
