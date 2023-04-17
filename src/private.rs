pub trait Try {
    fn is_success(&self) -> bool;
}

impl<T> Try for Option<T> {
    #[inline(always)]
    fn is_success(&self) -> bool {
        self.is_some()
    }
}

impl<T, E> Try for Result<T, E> {
    #[inline(always)]
    fn is_success(&self) -> bool {
        self.is_ok()
    }
}
