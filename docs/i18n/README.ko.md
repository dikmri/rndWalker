# rndWalker

`rndWalker`는 로컬 폴더의 동영상과 음악을 무작위로 재생하는 Rust + eframe 데스크톱 앱입니다.

구 Electron 판은 `legacy/electron/` 에 퇴피하고 있어 향후의 개발은 Rust 판을 기준으로 진행합니다.

## 주요 기능

- MP4 동영상을 무작위로 재생
- 최대 3개 그룹의 MP4 폴더 관리
- 각 MP4 그룹에 최대 4개의 폴더를 등록 가능
- MP3 오디오 독립 랜덤 재생
- 폴더 프리셋, 볼륨, 음소거, 설정 저장
- GitHub Releases를 사용하여 시작 시 자동 업데이트 확인
- Windows, macOS, Linux용 자동 릴리스

## 키보드 조작

| 키 | 동작 |
| --- | --- |
| F5 | 동영상 폴더 다시 로드 |
| F11 | 전체 화면 전환 |
| Esc | 전체 화면 해제 또는 설정 화면 닫기 |
| M | 음소거 전환 |
| W/S | 볼륨 업 / 볼륨 다운 |
| D/A | 다음 무작위 동영상 / 이전 동영상 |
| 왼쪽 / 위 / 오른쪽 | 현재 동영상 종료 후 폴더 그룹 1/2/3으로 전환 |
| 아래 | 현재 동영상 종료 후 모든 폴더로 되돌리기 |

## 빌드

`egui-video`가 FFmpeg를 사용하기 때문에, 개발 환경에는 FFmpeg 7의 개발 라이브러리와 `pkg-config`가 필요합니다.

```powershell
cargo build --release
```

설정은 OS 표준의 설정 디렉토리에 `rndWalker/settings.json` 로서 보존됩니다.

## 릴리스 및 업데이트

`v0.1.12`와 같은 태그를 푸시하면 `.github/workflows/release.yml`이 실행되고 지원 플랫폼 용 ZIP이 GitHub Releases에 업로드됩니다.

릴리스 버전은 시작시 `dikmri / rndWalker`의 GitHub Releases를 확인합니다. 업데이트가 있으면 실행 파일을 바꾸고 완료 후 다시 시작하라는 메시지를 표시합니다.

## 다국어 문서

README나 릴리스 노트의 정본은 일본어로 관리합니다. `main` 브랜치로 일본어 문서가 갱신되면, `.github/workflows/docs-i18n.yml` 가 번역을 생성해 `docs/i18n/` 에 반영합니다.

- 영어: `docs/i18n/README.en.md`
- 중국어: `docs/i18n/README.zh-CN.md`
- 한국어: `docs/i18n/README.ko.md`

번역은 자동 생성이므로 정확성이 필요한 수정은 일본어 정본에 반영해 주십시오.
