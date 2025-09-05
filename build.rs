fn main() {
    #[cfg(windows)]
    build_proxy();
}

#[cfg(windows)]
fn build_proxy() {
    cc::Build::new()
        .cpp(true)
        .file("windows.cpp")
        .compile("tsukiproxy");
}
