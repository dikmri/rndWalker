# rndWalker

<p align="center">
<img src="assets/icons/rndwalker-128.png" alt="rndWalker icon" width="128" height="128">
</p>

`rndWalker`는 로컬 폴더의 동영상과 음악을 무작위로 재생하는 Rust + eframe 데스크톱 앱입니다.

구 Electron 판은 `legacy/electron/` 에 퇴피하고 있어 향후의 개발은 Rust 판을 기준으로 진행합니다.

## 주요 기능

- MP4 동영상을 무작위로 재생
- 최대 3개 그룹의 MP4 폴더 관리
- 각 MP4 그룹에 최대 4개의 폴더를 등록 가능
- 동영상을 화면 가득 채우는 멀티뷰 재생
- MP3 오디오 독립 랜덤 재생
- 폴더 프리셋, 볼륨, 음소거, 설정 저장
- GitHub Releases를 사용하여 시작 시 자동 업데이트 확인
- Windows, macOS 및 Linux용 자동 릴리스

## 설치

최신 버전의 GitHub Release를 자동으로 결정하여 사용자 영역에 설치합니다.

### Windows

PowerShell에서 다음을 수행합니다.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.ps1 | iex"
```

기본 설치 위치는 `%LOCALAPPDATA%\Programs\rndWalker`입니다. 시작 메뉴 바로 가기를 만들고 사용자 PATH에 설치 대상을 추가합니다.

### macOS/Linux

터미널에서 다음을 수행합니다.

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | sh
```

기본 설치 위치는 `~/.local/share/rndWalker`입니다. `~/.local/bin/rndWalker` 에 기동용 링크를 작성합니다. macOS는 Intel / Apple Silicon을 자동으로 결정하고 Linux는 x86_64를 지원합니다.

macOS에서는 Homebrew를 사용할 수 있는 경우 FFmpeg를 자동으로 확인합니다. Linux에서는 `apt-get`, `dnf`, `pacman` 중 하나를 사용할 수있는 경우 FFmpeg와 SDL2 런타임을 도입합니다. Linux에서는 `sudo`의 비밀번호 입력이 필요할 수 있습니다. 종속성의 자동 배포를 피하려면 다음을 수행합니다.

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | env RNDWALKER_SKIP_DEPS=1 sh
```

## 키보드 조작

| 키 | 동작 |
| --- | --- |
| F5 | 동영상 폴더 다시 로드 |
| F11 | 전체 화면 전환 |
| Esc | 전체 화면 해제 또는 설정 화면 닫기 |
| M | 음소거 전환 |
| W/S | 볼륨 업 / 볼륨 다운 |
| D/A | 다음 무작위 동영상 / 이전 동영상. 멀티 뷰 중의`D`는 재 셔플 |
| 왼쪽 / 위 / 오른쪽 | 폴더 그룹 1/2/3으로 전환 |
| 아래 | 모든 폴더로 되돌리기 |

## 멀티뷰

설정 화면에서 `멀티 뷰 활성화`를 체크하면 현재 동영상 폴더의 동영상을 화면 전체에 깔아 동시 재생합니다.

`동영상 사이즈` 슬라이더로 타일 사이즈의 기준을 조정할 수 있습니다. 각 동영상은 컷도 변형도 하지 않고 전체를 표시한 채로, 행마다 높이를 가지런히 해 검은 틈이 나오지 않게 깔아 둡니다. 최하단의 행은 화면 아래에 조금 간과할 수 있습니다. 같은 화면에서 가능한 한 별도의 동영상을 선택합니다. 동영상 수가 타일 수보다 적으면 중복됩니다.

멀티 뷰의 각 동영상은 무음으로 재생되고 MP3 폴더의 음성 재생은 기존대로 독립적으로 계속됩니다.

각 타일은 표시 사이즈에 맞춘 해상도로 축소 디코딩되므로 다 타일 시에도 CPU/GPU 부하를 억제할 수 있습니다. 동영상을 로드하는 동안 검은 셀이 단시간에 표시되고 준비가 되면 바로 전환됩니다. 윈도우를 리사이즈해도 재생중의 타일은 그대로 유지되어 레이아웃만이 추종합니다.

## 빌드

비디오 디코딩에는 `vendor/egui-video`의 포크 버전을 사용하고 있습니다. FFmpeg 7 의 개발 라이브러리와 `pkg-config` 가 개발 환경에 필요합니다.

```powershell
cargo build --release
```

설정은 OS 표준의 설정 디렉토리에 `rndWalker/settings.json` 로서 보존됩니다.

## 릴리스 및 업데이트

`v0.1.12`와 같은 태그를 푸시하면 `.github/workflows/release.yml`이 실행되고 지원 플랫폼 용 ZIP이 GitHub Releases에 업로드됩니다.

릴리스 버전은 기동시에 `dikmri/rndWalker` 의 GitHub Releases 를 확인합니다. 업데이트가 있으면 실행 파일을 바꾸고 완료 후 다시 시작하라는 메시지를 표시합니다.

## 다국어 문서

README나 릴리스 노트의 정본은 일본어로 관리합니다. `main` 브랜치로 일본어 문서가 갱신되면, `.github/workflows/docs-i18n.yml` 가 번역을 생성해 `docs/i18n/` 에 반영합니다.

- 영어: `docs/i18n/README.en.md`
- 중국어: `docs/i18n/README.zh-CN.md`
- 한국어: `docs/i18n/README.ko.md`

번역은 자동 생성이므로 정확성이 필요한 수정은 일본어 정본에 반영해 주십시오.
