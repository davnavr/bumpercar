//! [`PartialEq`] implementations for [`Vec<T>`](crate::vec::Vec).

use crate::vec::Vec;
use crate::Bump;
use core::cmp::PartialEq;

impl<'alloc, T, U, A> PartialEq<[U]> for Vec<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &[U]) -> bool {
        self.as_slice() == other
    }
}

impl<'alloc, T, U, A, const N: usize> PartialEq<[U; N]> for Vec<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &[U; N]) -> bool {
        self.as_slice() == other
    }
}

impl<'alloc, T, U, A> PartialEq<&[U]> for Vec<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&[U]) -> bool {
        self.as_slice() == *other
    }
}

impl<'alloc, T, U, A> PartialEq<&mut [U]> for Vec<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&mut [U]) -> bool {
        self.as_slice() == *other
    }
}

impl<'alloc, T, U, A, const N: usize> PartialEq<&[U; N]> for Vec<'alloc, T, A>
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&[U; N]) -> bool {
        self.as_slice() == *other
    }
}

impl<'alloc, T, U, A> PartialEq<Vec<'alloc, U, A>> for [T]
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, U, A>) -> bool {
        self == other.as_slice()
    }
}

impl<'alloc, T, U, A> PartialEq<Vec<'alloc, U, A>> for &[T]
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, U, A>) -> bool {
        *self == other.as_slice()
    }
}

impl<'alloc, T, U, A> PartialEq<Vec<'alloc, U, A>> for &mut [T]
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, U, A>) -> bool {
        *self == other.as_slice()
    }
}

impl<'alloc, T, U, A> PartialEq<Vec<'alloc, U, A>> for alloc::borrow::Cow<'_, [T]>
where
    A: for<'arena> Bump<'alloc, 'arena>,
    T: PartialEq<U> + Clone,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, U, A>) -> bool {
        *self == other.as_slice()
    }
}

impl<'alloc1, 'alloc2, T, U, A1, A2> PartialEq<Vec<'alloc2, U, A2>> for Vec<'alloc1, T, A1>
where
    A1: for<'arena> Bump<'alloc1, 'arena>,
    A2: for<'arena> Bump<'alloc2, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc2, U, A2>) -> bool {
        *self == other.as_slice()
    }
}
