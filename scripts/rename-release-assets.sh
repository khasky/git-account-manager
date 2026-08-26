#!/usr/bin/env bash
# Rename the installers on a published release to one flat pattern:
#
#   Git-Account-Manager-<version>-<win|mac|linux>-<x64|arm64>[-setup].<ext>
#
# Tauri v2 has no artifact-name template (the config schema carries productName,
# mainBinaryName and bundleName, none of which shape the bundle filename), so the
# names are fixed at build time and can only be changed afterwards. tauri-action
# has already created the release and written latest.json by the time this runs,
# so every rename is mirrored into latest.json - an updater pointing at a URL
# that no longer resolves is worse than an inconsistent filename.
#
# The updater's own payloads (*.nsis.zip, *.app.tar.gz) and the detached
# signatures are left alone: nobody downloads them by hand, and the macOS ones
# carry no architecture token to rename them by.
#
# Usage: rename-release-assets.sh            (needs GH_TOKEN, REPO, TAG)
#        rename-release-assets.sh --self-test
set -euo pipefail

# Classification is deliberately blind to the product-name prefix: GitHub
# rewrites spaces to dots on upload, and the prefix changes with productName.
# Extension decides the OS, a token anywhere in the name decides the arch, and
# anything that matches neither is an error - a silent skip would ship one asset
# under Tauri's name and the rest under this one.
classify() {
  local name=$1 version=$2 os ext suffix arch

  case "$name" in
    *.msi) os=win; ext=msi; suffix="" ;;
    *-setup.exe) os=win; ext=exe; suffix="-setup" ;;
    *.dmg) os=mac; ext=dmg; suffix="" ;;
    *.AppImage) os=linux; ext=AppImage; suffix="" ;;
    *.deb) os=linux; ext=deb; suffix="" ;;
    *.rpm) os=linux; ext=rpm; suffix="" ;;
    *) echo "unclassified release asset: $name" >&2; return 1 ;;
  esac

  case "$name" in
    *aarch64* | *arm64*) arch=arm64 ;;
    *x64* | *x86_64* | *amd64*) arch=x64 ;;
    *) echo "no architecture in asset name: $name" >&2; return 1 ;;
  esac

  printf 'Git-Account-Manager-%s-%s-%s%s.%s\n' "$version" "$os" "$arch" "$suffix" "$ext"
}

self_test() {
  local failed=0 got
  # Tauri 2.10 bundle names for productName "Git Account Manager", as GitHub
  # stores them (spaces -> dots).
  local cases=(
    "Git.Account.Manager_0.1.7_x64_en-US.msi|Git-Account-Manager-0.1.7-win-x64.msi"
    "Git.Account.Manager_0.1.7_x64-setup.exe|Git-Account-Manager-0.1.7-win-x64-setup.exe"
    "Git.Account.Manager_0.1.7_aarch64.dmg|Git-Account-Manager-0.1.7-mac-arm64.dmg"
    "Git.Account.Manager_0.1.7_x64.dmg|Git-Account-Manager-0.1.7-mac-x64.dmg"
    "Git.Account.Manager_0.1.7_amd64.AppImage|Git-Account-Manager-0.1.7-linux-x64.AppImage"
    "Git.Account.Manager_0.1.7_amd64.deb|Git-Account-Manager-0.1.7-linux-x64.deb"
    "Git-Account-Manager-0.1.7-1.x86_64.rpm|Git-Account-Manager-0.1.7-linux-x64.rpm"
  )
  for case in "${cases[@]}"; do
    got=$(classify "${case%%|*}" 0.1.7) || got="<error>"
    if [ "$got" != "${case##*|}" ]; then
      echo "FAIL ${case%%|*}: got $got, want ${case##*|}" >&2
      failed=1
    fi
  done

  # Anything unrecognized must fail loudly rather than pass through unrenamed.
  for bad in "Git.Account.Manager_0.1.7_x64-setup.nsis.zip" "Git.Account.Manager.app.tar.gz" "latest.json"; do
    if classify "$bad" 0.1.7 2>/dev/null; then
      echo "FAIL $bad: classified, expected an error" >&2
      failed=1
    fi
  done

  [ "$failed" -eq 0 ] && echo "rename-release-assets: self-test passed"
  return "$failed"
}

[ "${1:-}" = "--self-test" ] && { self_test; exit; }

: "${REPO:?REPO is required}" "${TAG:?TAG is required}"
version=${TAG#v}
release_id=$(gh api "repos/$REPO/releases/tags/$TAG" --jq .id)
renames=$(mktemp)

while IFS=$'\t' read -r asset_id name; do
  case "$name" in
    latest.json | *.sig | *.nsis.zip | *.app.tar.gz) continue ;;
  esac

  new=$(classify "$name" "$version")
  [ "$new" = "$name" ] && continue

  gh api --method PATCH "repos/$REPO/releases/assets/$asset_id" -f name="$new" >/dev/null
  printf '%s\t%s\n' "$name" "$new" >>"$renames"
  echo "$name -> $new"
done < <(gh api "repos/$REPO/releases/$release_id/assets" --paginate --jq '.[] | [.id, .name] | @tsv')

[ -s "$renames" ] || { echo "no assets needed renaming"; exit 0; }

# latest.json holds a download URL per platform, each ending in the filename we
# just changed. Literal string replacement, not a regex: the names are full of
# dots that a regex would treat as wildcards.
if gh release download "$TAG" --repo "$REPO" --pattern latest.json --clobber 2>/dev/null; then
  python3 - "$renames" <<'PY'
import sys

with open(sys.argv[1], encoding="utf-8") as f:
    pairs = [line.rstrip("\n").split("\t") for line in f if line.strip()]

manifest = open("latest.json", encoding="utf-8").read()
for old, new in pairs:
    manifest = manifest.replace(old, new)
open("latest.json", "w", encoding="utf-8").write(manifest)
PY
  gh release upload "$TAG" latest.json --repo "$REPO" --clobber
  echo "latest.json updated"
fi
