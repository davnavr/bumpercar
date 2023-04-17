pub trait Try {
    type Output;
    type Residual;

    fn is_success(&self) -> bool;
    fn into_result(self) -> Result<Self::Output, Self::Residual>;
    fn from_output(output: Self::Output) -> Self;
    fn from_residual(residual: Self::Residual) -> Self;
}

impl<T> Try for Option<T> {
    type Output = T;
    type Residual = Option<core::convert::Infallible>;

    #[inline(always)]
    fn is_success(&self) -> bool {
        self.is_some()
    }

    #[inline(always)]
    fn into_result(self) -> Result<Self::Output, Self::Residual> {
        match self {
            Some(value) => Ok(value),
            None => Err(None),
        }
    }

    #[inline(always)]
    fn from_output(output: Self::Output) -> Self {
        Some(output)
    }

    #[inline(always)]
    fn from_residual(_: Self::Residual) -> Self {
        None
    }
}

impl<T, E> Try for Result<T, E> {
    type Output = T;
    type Residual = E;

    #[inline(always)]
    fn is_success(&self) -> bool {
        self.is_ok()
    }

    #[inline(always)]
    fn into_result(self) -> Result<Self::Output, Self::Residual> {
        self
    }

    #[inline(always)]
    fn from_output(output: Self::Output) -> Self {
        Ok(output)
    }

    #[inline(always)]
    fn from_residual(residual: Self::Residual) -> Self {
        Err(residual)
    }
}
