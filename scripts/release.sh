#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

require_command() {
    if ! command -v "$1" >/dev/null; then
        printf 'Required command is missing: %s\n' "$1" >&2
        exit 1
    fi
}

platform=$(uname -s)
architecture=$(uname -m)
if [[ ${1:-} == --wsl && $# == 2 && $platform == Linux ]]; then
    platform=WSL
    export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
elif [[ $# != 0 ]]; then
    printf 'Usage: %s\n' "$0" >&2
    exit 2
fi

case "$platform" in
    Linux)
        require_command dpkg-deb
        require_command dpkg-shlibdeps
        require_command rpmbuild
        case "$architecture" in
            x86_64) deb_architecture=amd64 ;;
            aarch64) deb_architecture=arm64 ;;
            *)
                printf 'Unsupported Linux architecture: %s\n' "$architecture" >&2
                exit 1
                ;;
        esac
        ;;
    Darwin) require_command pkgbuild ;;
    WSL) ;;
    *)
        printf 'Unsupported release platform: %s\n' "$platform" >&2
        exit 1
        ;;
esac

require_command cargo
require_command rustc
host_target=$(rustc -vV | awk '/^host: / { print $2 }')
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"

printf '==> Formatting\n'
cargo fmt --all
printf '==> Linting\n'
cargo clippy --workspace --all-targets --release --target "$host_target" -- -D warnings
printf '==> Building\n'
cargo build --workspace --release --target "$host_target"
printf '==> Testing\n'
cargo test --workspace --release --target "$host_target"

binary="$CARGO_TARGET_DIR/$host_target/release/gm"
version=$("$binary" --version)
version=${version#gm }

if [[ $platform == WSL ]]; then
    install -m 755 "$binary" "$2/gm-linux"
    printf '%s\n%s\n' "$version" "$architecture" > "$2/release-info.txt"
    exit 0
fi

release_directory="$PWD/target/dist"
mkdir -p "$release_directory"
staging=$(mktemp -d "${TMPDIR:-/tmp}/gm-release.XXXXXX")
trap 'rm -rf -- "$staging"' EXIT
mkdir -p "$staging/artifacts" "$staging/bundle/bin"
install -m 755 "$binary" "$staging/bundle/bin/gm"

printf '==> Bundling\n'
case "$platform" in
    Linux) bundle_name="gm-$version-linux-$architecture" ;;
    Darwin) bundle_name="gm-$version-macos-$architecture" ;;
esac
install -m 644 LICENSE "$staging/bundle/LICENSE"
tar -czf "$staging/artifacts/$bundle_name.tar.gz" -C "$staging/bundle" bin LICENSE

printf '==> Packaging\n'
if [[ $platform == Linux ]]; then
    package_version=${version/-/\~}
    package_version=${package_version//-/.}
    deb_root="$staging/debian/generation-manager"
    mkdir -p "$deb_root/DEBIAN" "$deb_root/usr/bin"
    install -m 755 "$binary" "$deb_root/usr/bin/gm"
    install -Dm644 LICENSE "$deb_root/usr/share/doc/generation-manager/copyright"
    printf 'Source: generation-manager\n\nPackage: generation-manager\nArchitecture: any\n' > "$staging/debian/control"
    dependencies=$(cd "$staging" && dpkg-shlibdeps -O debian/generation-manager/usr/bin/gm)
    cat > "$deb_root/DEBIAN/control" <<EOF
Package: generation-manager
Version: $package_version
Architecture: $deb_architecture
Maintainer: mingyuchoo <mingyuchoo@gmail.com>
Depends: ${dependencies#shlibs:Depends=}
Suggests: git
Description: Worktree-to-generation development lifecycle manager
EOF
    dpkg-deb --build --root-owner-group "$deb_root" \
        "$staging/artifacts/generation-manager_${package_version}_${deb_architecture}.deb"

    mkdir -p "$staging/rpm/SOURCES"
    install -m 755 "$binary" "$staging/rpm/SOURCES/gm"
    install -m 644 LICENSE "$staging/rpm/SOURCES/LICENSE"
    cat > "$staging/generation-manager.spec" <<EOF
Name: generation-manager
Version: $package_version
Release: 1
Summary: Worktree-to-generation development lifecycle manager
License: LicenseRef-Proprietary
URL: https://github.com/mgch/generation-manager
Source0: gm
Source1: LICENSE
%global __os_install_post %{nil}
%global _build_id_links none

%description
Manage development worktrees, release generations, and rollbacks.

%install
install -Dm755 "%{SOURCE0}" "%{buildroot}%{_bindir}/gm"
install -Dm644 "%{SOURCE1}" "%{buildroot}%{_licensedir}/%{name}/LICENSE"

%files
%{_bindir}/gm
%license %{_licensedir}/%{name}/LICENSE
EOF
    rpmbuild -bb --nodeps --target "$architecture" --define "_topdir $staging/rpm" \
        --define "_rpmdir $staging/artifacts" --define '_rpmfilename %{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}.rpm' \
        "$staging/generation-manager.spec"
else
    mkdir -p "$staging/pkg/usr/local/bin"
    install -m 755 "$binary" "$staging/pkg/usr/local/bin/gm"
    mkdir -p "$staging/pkg/usr/local/share/doc/generation-manager"
    install -m 644 LICENSE "$staging/pkg/usr/local/share/doc/generation-manager/LICENSE"
    pkgbuild --root "$staging/pkg" --install-location / --identifier com.github.mgch.generation-manager \
        --version "${version%%[-+]*}" "$staging/artifacts/$bundle_name.pkg"
fi

mv -f -- "$staging/artifacts/"* "$release_directory/"
printf '==> Release artifacts: %s\n' "$release_directory"
