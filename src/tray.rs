use image::ImageFormat;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::app::{AppResult, AppState};

// 실행 파일에 포함된 트레이 아이콘 원본 데이터
const ACTIVE_ICON_DATA: &[u8] = include_bytes!("../assets/active.ico");
const INACTIVE_ICON_DATA: &[u8] = include_bytes!("../assets/inactive.ico");

pub enum TrayUserAction {
    Toggle,
    Quit,
}

pub struct TrayUi {
    tray_icon: TrayIcon,
    active_icon: Icon,
    inactive_icon: Icon,
    status_menu_item: MenuItem,
    toggle_menu_item: MenuItem,
    quit_menu_item: MenuItem,
}

impl TrayUi {
    pub fn new(state: &AppState) -> AppResult<Self> {
        let active_icon = load_icon(ACTIVE_ICON_DATA)?;
        let inactive_icon = load_icon(INACTIVE_ICON_DATA)?;

        let status_menu_item = MenuItem::with_id("status", state.status_text(), false, None);
        let toggle_menu_item = MenuItem::with_id("toggle", "Caffeine 켜기/끄기", true, None);
        let quit_menu_item = MenuItem::with_id("quit", "종료", true, None);

        let tray_menu = Menu::new();
        tray_menu.append(&status_menu_item)?;
        tray_menu.append(&PredefinedMenuItem::separator())?;
        tray_menu.append(&toggle_menu_item)?;
        tray_menu.append(&quit_menu_item)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_menu_on_left_click(false)
            .with_tooltip(state.tooltip())
            .with_icon(select_icon(state.is_active(), &active_icon, &inactive_icon).clone())
            .build()?;

        Ok(Self {
            tray_icon,
            active_icon,
            inactive_icon,
            status_menu_item,
            toggle_menu_item,
            quit_menu_item,
        })
    }

    pub fn sync(&self, state: &AppState) -> AppResult<()> {
        // 상태 변경 시 아이콘, 툴팁, 메뉴 텍스트 동시 갱신
        self.tray_icon.set_icon(Some(
            select_icon(state.is_active(), &self.active_icon, &self.inactive_icon).clone(),
        ))?;
        self.tray_icon.set_tooltip(Some(state.tooltip()))?;
        self.status_menu_item.set_text(state.status_text());

        Ok(())
    }

    pub fn resolve_menu_action(&self, event: &MenuEvent) -> Option<TrayUserAction> {
        if event.id == self.toggle_menu_item.id() {
            Some(TrayUserAction::Toggle)
        } else if event.id == self.quit_menu_item.id() {
            Some(TrayUserAction::Quit)
        } else {
            None
        }
    }
}

fn select_icon<'a>(is_active: bool, active_icon: &'a Icon, inactive_icon: &'a Icon) -> &'a Icon {
    if is_active {
        active_icon
    } else {
        inactive_icon
    }
}

fn load_icon(data: &[u8]) -> AppResult<Icon> {
    // ico 파일을 tray-icon이 요구하는 RGBA 포맷으로 변환
    let image = image::load_from_memory_with_format(data, ImageFormat::Ico)?.into_rgba8();
    let (width, height) = image.dimensions();

    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}
