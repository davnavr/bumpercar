//! Provides a smart pointer type for allocating values within an arena.
//!
//! A [`Box<'b, T>`](self::Box) Allows the running of destructors in objects stored within an
//! [`Arena`](crate::Arena).

use crate::Bump;
use core::mem::MaybeUninit;
use core::ops::DerefMut;

/// Provides ownership for a value stored in an [`Arena`](crate::Arena), and allows running its
/// [`Drop`](core::ops::Drop) implementation.
///
/// See the [module level documentation](crate::boxed) for more information.
#[derive(Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Box<'b, T: ?Sized> {
    value: &'b mut T,
}

impl<'b, T> Box<'b, T> {
    /// Allocates memory in the arena and moves the `value` into it.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpercar::{Allocator, Arena, boxed::Box};
    ///
    /// let mut arena = Arena::new();
    /// let allocator = arena.allocator();
    ///
    /// let six = Box::new(6, &allocator);
    /// ```
    pub fn new<'a, A: Bump<'a, 'b>>(value: T, allocator: &'a A) -> Self {
        Self {
            value: allocator.alloc(value),
        }
    }
}

impl<'b, T> Box<'b, MaybeUninit<T>> {
    /// Allocates memory in the arena for an instance of `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpercar::{Allocator, Arena, boxed::Box};
    ///
    /// let mut arena = Arena::new();
    /// let allocator = arena.allocator();
    ///
    /// let mut six = Box::new_uninit(&allocator);
    /// let six = unsafe { six.write(6) };
    /// assert_eq!(*six, 6);
    /// ```
    pub fn new_uninit<'a, A: Bump<'a, 'b>>(allocator: &'a A) -> Self {
        Self {
            value: allocator.alloc_uninit::<T>(),
        }
    }
}

impl<'b, T> Box<'b, [T]> {
    /// Allocates memory in the arena and copies the `slice` into it.
    pub fn new_slice<'a, A: Bump<'a, 'b>>(slice: &[T], allocator: &'a A) -> Self
    where
        T: Copy,
    {
        Self {
            value: allocator.alloc_slice(slice),
        }
    }

    /// Allocates memory in the arena for a slice, using the closure to fill the slice.
    ///
    /// See the documentation [`Bump::alloc_slice_with`](Bump::alloc_slice_with) for more
    /// information.
    ///
    /// # Example
    ///
    /// ```
    /// use bumpercar::{Allocator, Arena, boxed::Box};
    ///
    /// let mut arena = Arena::new();
    /// let allocator = arena.allocator();
    ///
    /// let items = Box::new_with(&allocator, 4, |i| i);
    /// assert_eq!(items.as_ref(), &[0usize, 1, 2, 3]);
    /// ```
    pub fn new_with<'a, A, F>(allocator: &'a A, length: usize, f: F) -> Self
    where
        A: Bump<'a, 'b>,
        F: FnMut(usize) -> T,
    {
        Self {
            value: allocator.alloc_slice_with(length, f),
        }
    }

    /// Allocates memory in the arena for a slice to contain the values yielded by an iterator.
    ///
    /// # Panics
    ///
    /// See the documentation for
    /// [`Bump::alloc_slice_from_iter`](crate::Bump::alloc_slice_from_iter) for more information.
    ///
    /// # Example
    ///
    /// ```
    /// use bumpercar::{Allocator, Arena, boxed::Box};
    ///
    /// let mut arena = Arena::new();
    /// let allocator = arena.allocator();
    ///
    /// let source = [1, 2, 3];
    /// let items = Box::from_iter(source.iter().copied(), &allocator);
    /// assert_eq!(items.as_ref(), source.as_slice());
    /// ```
    pub fn from_iter<'a, I, A>(items: I, allocator: &'a A) -> Self
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
        A: Bump<'a, 'b>,
    {
        Self {
            value: allocator.alloc_slice_from_iter(items),
        }
    }
}

impl<'b, T: ?Sized> Box<'b, T> {
    /// Consumes the [`Box<'b, T>`](self::Box), returning a raw pointer to the value stored in the arena.
    #[inline(always)]
    pub fn into_raw(this: Self) -> *mut T {
        // Prevents errors regarding running of Drop for Box
        let mut b = core::mem::ManuallyDrop::new(this);
        b.deref_mut().value as *mut T
    }

    /// Consumes the [`Box<'b, T>`](self::Box), returning a mutable reference to the value stored in the arena.
    ///
    /// Note that this means that the destructors for the value are **never run**.
    #[inline(always)]
    pub fn leak(this: Self) -> &'b mut T {
        // Safety: value lives for lifetime 'b
        unsafe { &mut *Self::into_raw(this) }
    }
}

impl<T: ?Sized> core::ops::Drop for Box<'_, T> {
    fn drop(&mut self) {
        // Safety: self is the sole owner of the reference to T, so destructor will only be run once
        unsafe { core::ptr::drop_in_place(self.value) }
    }
}

impl<T: ?Sized> core::ops::Deref for Box<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> DerefMut for Box<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<T: ?Sized> core::convert::AsRef<T> for Box<'_, T> {
    fn as_ref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> core::convert::AsMut<T> for Box<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<T: ?Sized> core::borrow::Borrow<T> for Box<'_, T> {
    fn borrow(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> core::borrow::BorrowMut<T> for Box<'_, T> {
    fn borrow_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<T> core::default::Default for Box<'_, [T]> {
    fn default() -> Self {
        Self {
            value: Default::default(),
        }
    }
}

impl core::default::Default for Box<'_, str> {
    fn default() -> Self {
        Self {
            value: Default::default(),
        }
    }
}

impl<T: core::fmt::Debug + ?Sized> core::fmt::Debug for Box<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.value, f)
    }
}

impl<T: core::fmt::Display + ?Sized> core::fmt::Display for Box<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.value, f)
    }
}

impl<T: ?Sized> core::fmt::Pointer for Box<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <*const T as core::fmt::Pointer>::fmt(&(self.value as *const T), f)
    }
}

#[cfg(feature = "std")]
impl<T: std::error::Error> std::error::Error for Box<'_, T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        <T as std::error::Error>::source(self)
    }
}

impl<T: core::iter::Iterator + ?Sized> core::iter::Iterator for Box<'_, T> {
    type Item = <T as core::iter::Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.value.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.value.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.value.last()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.value.count()
    }
}

impl<T: core::iter::ExactSizeIterator + ?Sized> core::iter::ExactSizeIterator for Box<'_, T> {
    fn len(&self) -> usize {
        self.value.len()
    }
}

impl<T: core::iter::DoubleEndedIterator + ?Sized> core::iter::DoubleEndedIterator for Box<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.value.next_back()
    }
}

impl<T: core::iter::FusedIterator + ?Sized> core::iter::FusedIterator for Box<'_, T> {}
