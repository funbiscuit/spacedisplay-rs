#!/bin/bash
# This script sends predefined input to another tmux session
# To showcase usage of application
# Before running script you need to start a tmux session:
# tmux new -s spacedisplay-demo
#
# To convert recorded mp4 its best to use gifski:
# ffmpeg -i spacedisplay.mp4 frame%04d.png
# gifski -o demo.gif -Q 20 frame*.png

#disable status bar
tmux set -g status off
# Ctrl+L

# Useful aliases for sending keys
shopt -s expand_aliases
alias tsend="tmux send-keys -t spacedisplay-demo.0"
alias tsendd="sleep 0.5 && tmux send-keys -t spacedisplay-demo.0"

# Clear screen
tsend C-l

# Run app
tsend "cargo r --release" Enter
sleep 3

# Start new scan
tsendd "n"
tsendd Enter

# Show statistics while scanning
tsendd "s"

sleep 5

# Close statistics
tsend "s"

# Navigate tree
tsendd Down
tsendd Down
tsendd Up
tsendd Up
tsendd Up
tsendd Enter
tsendd Enter
tsendd Down
tsendd Down
sleep 0.5
tsendd Left
tsendd Enter
sleep 0.5
tsendd Left
tsendd Left

sleep 1

# Show delete dialog
tsendd "d"
sleep 0.5

# Cancel deletion
tsendd "n"

sleep 0.5
# Rescan
tsendd "r"
# Show Stats
tsendd "s"
sleep 4
tsendd "s"

# Quit app
sleep 5
tsend "q"
