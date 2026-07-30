# 키 유출 차단 — check는 놓치고 backup은 잡는다.
Output secret.gif
Set FontSize 16
Set FontFamily "D2Coding"
Set Width 1180
Set Height 440
Set Theme "rose-pine"
Set Padding 22
Set TypingSpeed 55ms
Set Framerate 24

Set Shell bash

# 준비 구간은 GIF에 담지 않는다.
Hide
Type "export PS1='$ ' && clear"  Enter
Type "cd /tmp/kigtit-demo/leaky"  Enter
Type "clear"  Enter
Show

Type "kigtit check"  Sleep 400ms  Enter
Sleep 2s
Type "kigtit backup"  Sleep 400ms  Enter
Sleep 5s
