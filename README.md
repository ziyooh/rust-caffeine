# Rust-Caffeine

Windows에서 절전 모드와 화면 꺼짐을 방지하는 시스템 트레이 애플리케이션입니다.

## 기능

- `SetThreadExecutionState`를 사용해 절전과 화면 꺼짐 방지
- 시스템 트레이에서 활성화, 비활성화, 종료 지원
- 아이콘이 포함된 단일 `.exe` 파일로 배포 가능
- 콘솔 창 없이 실행

## 사용 방법

1. `rust-caffeine.exe`를 실행합니다.
2. 시스템 트레이에서 아이콘 상태를 확인합니다.
3. 아이콘을 우클릭해 메뉴를 사용합니다.

`Caffeine 켜기/끄기`: 절전 방지 상태 전환  
`종료`: 프로그램 종료 및 시스템 상태 복원

아이콘 상태:

- 주황색: 활성화
- 회색: 비활성화

## 빌드

### 요구 사항

- [Rust & Cargo](https://rustup.rs/)
- Windows

### 권장 방식

```powershell
.\build-release.ps1
```

빌드가 끝나면 `dist\rust-caffeine.exe`만 남습니다.

### 수동 방식

```sh
cargo build --release
```

## 사용 크레이트

- `tray-icon`
- `tao`
- `muda`
- `windows`
- `image`
