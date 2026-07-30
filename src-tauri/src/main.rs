// 배포 빌드에서 콘솔 창이 따라 뜨지 않게 한다.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    kigtit_app::run()
}
