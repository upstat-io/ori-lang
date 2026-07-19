#!/bin/bash

initialize_test_all_log() {
    local log_file="$1"
    local initial

    initial=$(mktemp "${log_file}.init.XXXXXX") || return 1
    if ! mv -f -- "$initial" "$log_file"; then
        rm -f -- "$initial"
        return 1
    fi
}
