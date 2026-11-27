# Generation Manager (`gm`)

`gm`은 Git worktree에서 개발하고, 빌드 산출물을 번호가 매겨진 **세대(generation)**로 저장하고, 실행 중인 서비스를 이전 세대로 되돌릴 수 있는 로컬 CLI입니다. 세대를 활성화할 때 설정된 헬스체크가 실패하면 직전에 활성화되어 있던 세대의 복구를 시도합니다.

## 요구 사항

- **macOS 또는 Linux.** Linux에서는 서비스 프로세스 그룹을 종료하는 데 `pidfd`를 사용하므로 커널 6.9 이상이 필요합니다. Windows에서는 기본 WSL 2 배포판에서 Linux 바이너리를 실행합니다. WSL의 커널에도 같은 요구 사항이 적용됩니다.
- **Rust 및 Cargo.** 소스에서 빌드할 때는 [rust-toolchain.toml](rust-toolchain.toml)에 지정된 nightly 툴체인을 사용합니다.
- **Git.** worktree 워크플로에 필요합니다. worktree를 만들려면 프로젝트 저장소에 커밋이 하나 이상 있어야 합니다.
- **프로젝트의 빌드 및 실행 도구.** 예를 들어 Node 프리셋은 `npm`과 `node`를 사용합니다.

## 설치

이 저장소의 루트에서 실행합니다. 설치 후 Cargo의 바이너리 디렉터리가 `PATH`에 있어야 합니다.

```sh
cargo install --locked --path apps/gm-cli
gm --help
```

바이너리만 빌드하려면 `cargo build --release --locked -p gm-cli`를 실행합니다. 기본 출력 경로는 `target/release/gm`입니다.

## 빠른 시작

관리할 프로젝트의 루트에서 매니페스트를 생성합니다. 아래 예시는 실행 바이너리 이름이 `demo`인 Rust 프로젝트를 가정합니다. 생성된 명령과 산출물 경로를 프로젝트에 맞게 수정하고, `.gm/`은 Git에서 제외합니다.

```sh
gm project init --preset rust --name demo
```

worktree를 만든 다음 그 안에서 개발합니다. `gm worktree run`은 기본적으로 빌드 후 전경에서 실행합니다. Ctrl-C로 종료한 뒤 세대를 빌드하고 활성화합니다.

```sh
gm worktree create add-cache
cd .gm/worktrees/add-cache

gm worktree run
gm generation build --activate
gm generation list
gm service status
gm service logs -n 100
```

활성 세대보다 번호가 낮은 세대가 남아 있으면 `gm generation rollback`으로 그중 가장 높은 번호의 세대를 활성화할 수 있습니다. 프로젝트 루트에서 `gm generation build --activate`를 실행하면 루트의 코드를 빌드해 활성화합니다.

관리되는 worktree 안에서 실행해도 원본 프로젝트의 매니페스트와 `.gm/` 상태를 사용합니다. 빌드와 개발 실행은 대상 이름을 생략하면 현재 worktree를 사용합니다.

## 설정

`generation-manager.toml`에서 빌드, 테스트, 산출물, 실행, 헬스체크를 설정합니다. `gm project init`은 `rust`, `node`, `python`, `go`, `generic` 프리셋을 지원합니다. 프리셋을 생략하면 `Cargo.toml`, `package.json`, `go.mod`, `pyproject.toml` 또는 `requirements.txt` 순으로 확인하고, 일치하는 파일이 없으면 `generic`을 사용합니다.

다음은 포트 8080에서 수신하는 Rust 서비스의 예시입니다.

```toml
[project]
name = "demo"

[build]
cmd = "cargo build --release"

[test]
cmd = "cargo test"

[artifacts]
include = ["target/release/demo"]

[run]
cmd = "./target/release/demo"
env = { RUST_LOG = "info" }
stop_timeout_secs = 10

[health]
tcp = "127.0.0.1:8080"
timeout_secs = 30
interval_secs = 1
```

- `project.name`과 `run.cmd`는 필수입니다. `build.cmd`와 `test.cmd`는 생략하거나 비워 두면 해당 단계를 건너뜁니다. `[build]`, `[test]`, `[run]`의 `env`로 각 명령의 환경 변수를 지정할 수 있습니다.
- 명령은 `sh -c`로 실행합니다. 빌드와 테스트는 선택한 worktree 또는 프로젝트 루트에서 실행합니다. 개발 실행은 worktree에서, 세대 서비스와 `health.cmd`는 저장된 세대의 `root/`에서 실행합니다.
- `artifacts.include`에는 실행에 필요한 파일과 디렉터리를 빌드 디렉터리 기준 상대 경로로 지정합니다. 경로 구조를 유지해 세대에 복사하며, 내부를 가리키는 심볼릭 링크는 대상을 복사합니다. 절대 경로, `..`를 포함한 경로, 외부로 이어지는 링크, 디렉터리 순환, 저장 경로와 겹치는 디렉터리는 거부합니다.
- `run.stop_timeout_secs`는 SIGTERM을 보낸 뒤 SIGKILL을 보내기까지의 대기 시간입니다. 기본값은 10초입니다.

### 헬스체크와 롤백

| 설정 | 통과 조건 |
| --- | --- |
| `tcp = "host:port"` | TCP 연결 성공 |
| `http = "http://host:port/path"` | 평문 HTTP 응답 상태가 2xx 또는 3xx |
| `cmd = "..."` | 셸 명령의 종료 코드가 0 |

설정한 프로브를 모두 통과해야 합니다. 제한 시간은 기본 30초이며, 재시도 간격은 기본 1초입니다. 생성된 프리셋에는 프로브가 들어 있지 않으므로 검증을 사용하려면 직접 추가하세요. 프로브 없이 활성화하면 헬스체크를 건너뜁니다.

`generation activate`, `generation build --activate`, `generation rollback`은 서비스 슬롯의 기존 프로세스를 종료하고, 활성 세대 포인터를 원자적으로 교체한 뒤 대상 세대를 시작합니다. 시작 또는 헬스체크에 실패하면 대상 세대를 `rejected`로 표시하고, 직전 활성 세대가 대상 세대와 다르면 포인터를 되돌려 다시 시작하고 검증합니다. 복구할 세대가 없으면 활성 포인터를 지웁니다. 활성화가 실패한 명령은 종료 코드 1을 반환합니다. 서비스 실행 중에는 헬스체크를 지속적으로 수행하지 않습니다.

## 명령어

전역 옵션 `-C, --directory <DIR>`로 명령의 시작 디렉터리를 지정할 수 있습니다. 자세한 옵션은 `gm <그룹> <명령> --help`에서 확인합니다.

| 명령 | 설명 |
| --- | --- |
| `gm project init [--preset P] [--name N] [--force]` | 현재 디렉터리에 매니페스트를 생성합니다. |
| `gm project status` | 프로젝트 루트, 활성 세대, 현재 worktree, 서비스 상태를 표시합니다. |
| `gm worktree create <NAME> [--base REF]` | `.gm/worktrees/<NAME>`에 worktree와 같은 이름의 브랜치를 만듭니다. |
| `gm worktree list` | 관리되는 worktree를 나열합니다. `*`는 현재 worktree입니다. |
| `gm worktree run [WORKTREE] [--no-build] [--detach]` | worktree 코드를 직접 실행합니다. `--no-build`는 빌드를 생략하고, `--detach`는 배경에서 실행합니다. |
| `gm worktree remove <NAME> [--force]` | worktree를 제거하고 브랜치는 보존합니다. |
| `gm generation build [WORKTREE] [--note TEXT] [--activate]` | 설정된 빌드와 테스트를 실행해 새 세대를 저장합니다. |
| `gm generation list` | 세대와 상태(`built`, `healthy`, `rejected`)를 나열합니다. `*`는 활성 세대입니다. |
| `gm generation activate [GENERATION]` | 지정한 세대 또는 가장 최근 세대를 활성화합니다. |
| `gm generation rollback [GENERATION]` | 지정한 세대 또는 현재보다 번호가 낮은 세대 중 가장 최근 것을 활성화합니다. |
| `gm generation history` | 세대 전환 이력과 사유를 표시합니다. |
| `gm generation prune [--keep N]` | 최근 세대(기본 5개), 활성 세대, 기본 롤백 대상을 보존하고 나머지를 삭제합니다. |
| `gm service start` / `stop` / `restart` | 활성 세대를 배경에서 시작하거나, 실행 슬롯의 프로세스를 종료하거나, 종료 후 활성 세대를 다시 시작합니다. |
| `gm service status` / `logs [-n N]` | 실행 상태와 출처를 확인하거나 배경 실행 로그의 마지막 줄을 읽습니다. 기본 40줄입니다. |

worktree 이름은 `add-cache`처럼 경로 구분자가 없는 단일 디렉터리 이름이어야 합니다.

### 서비스 실행 슬롯

프로젝트마다 실행 슬롯이 하나 있습니다. 개발 실행과 세대 활성화는 기존 실행을 종료하고 슬롯을 인계받습니다. 개발 실행은 테스트와 헬스체크를 건너뛰고, 전경에서 실행한 명령의 종료 코드를 전달합니다. 개발 실행을 종료한 뒤 활성 세대를 다시 실행하려면 `gm service start`를 사용합니다.

`gm service start`는 슬롯이 비어 있을 때 활성 세대를 시작합니다. `gm service restart`는 슬롯을 점유한 실행을 종료한 뒤 활성 세대를 시작합니다. 두 명령 모두 헬스체크와 자동 롤백을 수행하지 않습니다. `gm service stop`은 배경에서 실행한 worktree도 종료할 수 있습니다.

## 로컬 상태

`.gm/`에는 worktree, 세대 산출물, 활성 포인터와 실행 상태가 저장됩니다.

| 경로 | 내용 |
| --- | --- |
| `.gm/worktrees/<NAME>/` | 관리되는 Git worktree |
| `.gm/store/<ENTRY>/` | 산출물 복사본(`root/`)과 세대 메타데이터(`meta.json`) |
| `.gm/generations/<ID>` | 저장된 세대를 가리키는 심볼릭 링크 |
| `.gm/current` | 활성 세대를 가리키며 원자적으로 교체되는 심볼릭 링크 |
| `.gm/history.jsonl` | 세대 전환 이력 |
| `.gm/run/state.json` | PID, 프로세스 시작 식별자, 실행 출처 |
| `.gm/run/service.log` | 배경 실행의 표준 출력과 표준 오류 |
| `.gm/lock` | 프로젝트 단위 파일 잠금 |

세대는 산출물의 복사본입니다. 빌드 재현성, 자동 재시작, 다중 서비스 관리는 지원 범위 밖입니다. 실행과 복구에 쓰이는 `[run]` 및 `[health]` 설정은 명령을 실행할 때 원본 프로젝트의 매니페스트에서 읽습니다.

프로세스 시작 식별자를 기록하지 않던 버전에서 업그레이드할 때는 기존 버전에서 서비스를 먼저 종료하세요. 업그레이드 후 실행 중인 프로세스의 상태 파일에 식별자가 없으면, 해당 프로세스를 직접 확인하고 종료한 뒤 서비스를 다시 시작하세요.

## 개발

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
```

[scripts/run.sh](scripts/run.sh)은 포맷팅, 린트, 빌드, 테스트를 순서대로 실행한 뒤 인자를 `gm`에 넘깁니다. 인자가 없으면 `gm --help`를 실행합니다. 포맷팅은 소스 파일을 수정하며, 스크립트는 저장소 루트에서 실행되므로 다른 프로젝트를 지정할 때는 절대 경로를 사용합니다.

```sh
bash scripts/run.sh -C /path/to/project project status
```

PowerShell에서는 [scripts/run.ps1](scripts/run.ps1)을 사용합니다. Windows에서는 기본 WSL 배포판에서 같은 Bash 스크립트를 실행합니다.

| 크레이트 | 역할 |
| --- | --- |
| [gm-cli](apps/gm-cli/) | 명령줄 인터페이스, 출력, 어댑터 조립 |
| [gm-core](packages/gm-core/) | 도메인 타입, 설정 검증, 활성화 상태와 세대 정책 |
| [gm-application](packages/gm-application/) | 빌드, 활성화, 서비스, worktree 유스케이스와 포트 |
| [gm-store](packages/gm-store/) | 매니페스트와 로컬 상태 저장, 잠금 |
| [gm-runner](packages/gm-runner/) | 셸, Git, 프로세스 제어, 산출물 복사, 헬스체크 |

### 릴리스 패키지

릴리스 스크립트는 포맷팅, 린트, 릴리스 빌드, 테스트를 거쳐 `target/dist/`에 현재 플랫폼용 패키지를 생성합니다.

| 플랫폼 | 실행 | 산출물 | 추가 도구 |
| --- | --- | --- | --- |
| macOS | `bash scripts/release.sh` | `.tar.gz`, `.pkg` | `pkgbuild` |
| Linux (x86_64, aarch64) | `bash scripts/release.sh` | `.tar.gz`, `.deb`, `.rpm` | `dpkg-deb`, `dpkg-shlibdeps`, `rpmbuild` |
| Windows 11 | `./scripts/release.ps1` | `.zip`, NSIS 설치 프로그램 | 기본 WSL 2 배포판, 배포판 안의 Rust, Windows의 `makensis.exe` |

Windows 패키지의 `gm.cmd`는 기본 WSL 배포판에서 함께 제공되는 Linux 바이너리를 실행합니다.

## 라이선스

Generation Manager는 모든 권리가 유보된 독점 소프트웨어입니다. 사용 조건은 [LICENSE](LICENSE)에 따릅니다.
타사 구성 요소에는 각 구성 요소의 라이선스가 적용됩니다.
