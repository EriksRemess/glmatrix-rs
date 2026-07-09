fn main() {
    println!("cargo:rustc-link-lib=wayland-client");
    println!("cargo:rustc-link-lib=wayland-egl");
    println!("cargo:rustc-link-lib=EGL");
    println!("cargo:rustc-link-lib=GL");
}
