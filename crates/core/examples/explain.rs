//! 충돌 설명이 실제로 사람 말이 되는지 눈으로 확인하는 예제.
fn main() {
    let dir = std::env::args().nth(1).expect("폴더를 넘겨 주세요");
    let path = std::env::args().nth(2).expect("파일을 넘겨 주세요");
    let project = kigtit_core::Project::open(&dir).unwrap();
    let e = kigtit_core::sync::explain(&project, &path, kigtit_core::ai::detect()).unwrap();
    println!(
        "파일: {}\n\n[내 컴퓨터에서]\n{}\n\n[GitHub 쪽에서]\n{}",
        e.path, e.mine, e.theirs
    );
}
