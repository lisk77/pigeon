#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn trim_free_heap_pages() {
    unsafe {
        let _ = malloc_trim(0);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub fn trim_free_heap_pages() {}
