# 선택이 필요해요 — 작업 폴더는 건드리지 않는다.
Output sync.gif
Set FontSize 16
Set FontFamily "D2Coding"
Set Width 1180
Set Height 560
Set Theme "rose-pine"
Set Padding 22
Set TypingSpeed 55ms
Set Framerate 24

Set Shell bash

# 준비 구간은 GIF에 담지 않는다.
Hide
Type "export PS1='$ ' && clear"  Enter
Type "cd /tmp/kigtit-demo/desktop"  Enter
Type "clear"  Enter
Show

Type "kigtit sync"  Sleep 400ms  Enter
Sleep 5s
Type "cat config.js  # still my version"  Sleep 300ms  Enter
Sleep 2500ms
Type "kigtit sync --keep theirs"  Sleep 400ms  Enter
Sleep 5s
