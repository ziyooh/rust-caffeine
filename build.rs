fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/active.ico"); // .exe 파일에 입힐 아이콘
        res.compile().unwrap();
    }
}
