#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn tune_allocator_for_low_memory() {
    unsafe {
        let _ = mallopt(M_ARENA_MAX, 1);
        let _ = mallopt(M_TRIM_THRESHOLD, 64 * 1024);
        let _ = mallopt(M_TOP_PAD, 0);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn trim_free_heap_pages() {
    unsafe {
        let _ = malloc_trim(0);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn mallopt(param: i32, value: i32) -> i32;
    fn malloc_trim(pad: usize) -> i32;
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
const M_TRIM_THRESHOLD: i32 = -1;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
const M_TOP_PAD: i32 = -2;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
const M_ARENA_MAX: i32 = -8;