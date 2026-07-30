# 자동 저장 — 저장 버튼이 없다.
Output watch.gif
Set FontSize 16
Set FontFamily "D2Coding"
Set Width 1280
Set Height 620
Set Theme "rose-pine"
Set Padding 22
Set TypingSpeed 55ms
Set Framerate 24

Set Shell bash

# 준비 구간은 GIF에 담지 않는다.
Hide
Type "export PS1='$ ' && clear"  Enter
Type "cd /tmp/kigtit-demo/live"  Enter
Type "clear"  Enter
Show

# watch는 포그라운드를 잡으므로 배경으로 돌린다. 그래야 다음 명령이 실행된다.
Type "kigtit watch --idle 2 &"  Sleep 400ms  Enter
Sleep 2500ms
Type@0ms "  # AI creates a file"  Enter
Sleep 800ms
Type@0ms "echo 'def tax(p:' > tax.py"  Enter
Sleep 18s
