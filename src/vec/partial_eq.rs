//! [`PartialEq`] implementations for [`Vec<T>`](crate::vec::Vec).

use crate::vec::Vec;
use crate::Bump;
use core::cmp::PartialEq;

impl<'alloc, 'arena, T, U, A> PartialEq<[U]> for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &[U]) -> bool {
        self.as_slice() == other
    }
}

impl<'alloc, 'arena, T, U, A, const N: usize> PartialEq<[U; N]> for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &[U; N]) -> bool {
        self.as_slice() == other
    }
}

impl<'alloc, 'arena, T, U, A> PartialEq<&[U]> for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&[U]) -> bool {
        self.as_slice() == *other
    }
}

impl<'alloc, 'arena, T, U, A> PartialEq<&mut [U]> for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&mut [U]) -> bool {
        self.as_slice() == *other
    }
}

impl<'alloc, 'arena, T, U, A, const N: usize> PartialEq<&[U; N]> for Vec<'alloc, 'arena, T, A>
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&[U; N]) -> bool {
        self.as_slice() == *other
    }
}

impl<'alloc, 'arena, T, U, A> PartialEq<Vec<'alloc, 'arena, U, A>> for [T]
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, 'arena, U, A>) -> bool {
        self == other.as_slice()
    }
}

impl<'alloc, 'arena, T, U, A> PartialEq<Vec<'alloc, 'arena, U, A>> for &[T]
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, 'arena, U, A>) -> bool {
        *self == other.as_slice()
    }
}

impl<'alloc, 'arena, T, U, A> PartialEq<Vec<'alloc, 'arena, U, A>> for &mut [T]
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, 'arena, U, A>) -> bool {
        *self == other.as_slice()
    }
}

impl<'alloc, 'arena, T, U, A> PartialEq<Vec<'alloc, 'arena, U, A>> for alloc::borrow::Cow<'_, [T]>
where
    A: Bump<'alloc, 'arena>,
    T: PartialEq<U> + Clone,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc, 'arena, U, A>) -> bool {
        *self == other.as_slice()
    }
}

impl<'alloc1, 'alloc2, 'arena1, 'arena2, T, U, A1, A2> PartialEq<Vec<'alloc2, 'arena2, U, A2>>
    for Vec<'alloc1, 'arena1, T, A1>
where
    A1: Bump<'alloc1, 'arena1>,
    A2: Bump<'alloc2, 'arena2>,
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Vec<'alloc2, 'arena2, U, A2>) -> bool {
        *self == other.as_slice()
    }
}
