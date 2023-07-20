#!/bin/bash

sed -e '1s/^/[\'$'\n''/' -e '$s/,$/\'$'\n'']/' $(find . -name '*.json') > compile_commands.json
