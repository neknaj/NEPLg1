#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR_RAW="${EDITOR_SAMPLE_DEST:-${SCRIPT_DIR}/..}"
mkdir -p "${ROOT_DIR_RAW}"
ROOT_DIR="$(cd "${ROOT_DIR_RAW}" && pwd)"
DEST_DIR="${ROOT_DIR}/web/vendor/editorsample"
REPO_URL="${EDITOR_SAMPLE_REPO:-https://github.com/bem130/editorsample}"
REF_NAME="${EDITOR_SAMPLE_REF:-}"

if [ -e "${DEST_DIR}" ] && [ ! -d "${DEST_DIR}/.git" ]; then
    echo "Destination ${DEST_DIR} exists but is not a git repository" >&2
    exit 1
fi

mkdir -p "$(dirname "${DEST_DIR}")"

update_repo() {
    if [ -n "${REF_NAME}" ]; then
        git -C "${DEST_DIR}" fetch --depth=1 origin "${REF_NAME}"
    else
        git -C "${DEST_DIR}" fetch --depth=1 origin
    fi
    git -C "${DEST_DIR}" reset --hard FETCH_HEAD
    git -C "${DEST_DIR}" clean -fdx
}

if [ -d "${DEST_DIR}/.git" ]; then
    git -C "${DEST_DIR}" remote set-url origin "${REPO_URL}"
    update_repo
else
    git clone --depth=1 "${REPO_URL}" "${DEST_DIR}"
    if [ -n "${REF_NAME}" ]; then
        git -C "${DEST_DIR}" fetch --depth=1 origin "${REF_NAME}"
        git -C "${DEST_DIR}" reset --hard FETCH_HEAD
    fi
fi
