#!/bin/bash

adb devices | grep emulator | cut -f1 | xargs -I {} adb -s "{}" emu kill