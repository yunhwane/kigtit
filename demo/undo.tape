# 되돌리기 — 잘 켜졌던 시점으로. 되돌린 것도 되돌릴 수 있다.
Output undo.gif
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
Type "cd /tmp/kigtit-demo/cafe"  Enter
Type "clear"  Enter
Show

Type "kigtit health"  Sleep 400ms  Enter
Sleep 3s
Type "kigtit undo"  Sleep 400ms  Enter
Sleep 2500ms
Type "kigtit health"  Sleep 400ms  Enter
Sleep 3500ms
