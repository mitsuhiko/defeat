use std::any::TypeId;
use std::error;
use std::fmt::{Debug, Display};

use backtrace::Backtrace;

/// An error trait
pub trait Error: Debug + Display {
    /// Returns the origin of this error which can be another error.
    fn origin(&self) -> Option<&(dyn Error + 'static)> {
        self.sync_origin().map(|x| &*x as &_)
    }

    /// Like `origin` but only returns an error if it's sync and send.
    fn sync_origin(&self) -> Option<&(dyn Error + Sync + Send + 'static)> {
        None
    }

    /// Return the backtrace of this error if available.
    fn backtrace(&self) -> Option<&Backtrace> {
        None
    }

    /// Get the `TypeId` of `self`
    #[doc(hidden)]
    fn type_id(&self) -> TypeId
    where
        Self: 'static,
    {
        TypeId::of::<Self>()
    }

    // -- Deprecated methods

    #[doc(hidden)]
    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    #[doc(hidden)]
    fn cause(&self) -> Option<&dyn Error> {
        self.origin().map(|x| &*x as &_)
    }
}

impl Error + 'static {
    /// Returns true if the boxed type is the same as `T`
    #[inline]
    pub fn is<T: Error + 'static>(&self) -> bool {
        let t = TypeId::of::<T>();
        let boxed = self.type_id();
        t == boxed
    }

    /// Returns some reference to the boxed value if it is of type `T`, or
    /// `None` if it isn't.
    #[inline]
    pub fn downcast_ref<T: Error + 'static>(&self) -> Option<&T> {
        if self.is::<T>() {
            unsafe { Some(&*(self as *const Error as *const T)) }
        } else {
            None
        }
    }

    /// Returns some mutable reference to the boxed value if it is of type `T`, or
    /// `None` if it isn't.
    #[inline]
    pub fn downcast_mut<T: Error + 'static>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            unsafe { Some(&mut *(self as *mut Error as *mut T)) }
        } else {
            None
        }
    }
}

impl Error + 'static + Send {
    /// Forwards to the method defined on the type `Any`.
    #[inline]
    pub fn is<T: Error + 'static>(&self) -> bool {
        <Error + 'static>::is::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    #[inline]
    pub fn downcast_ref<T: Error + 'static>(&self) -> Option<&T> {
        <Error + 'static>::downcast_ref::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    #[inline]
    pub fn downcast_mut<T: Error + 'static>(&mut self) -> Option<&mut T> {
        <Error + 'static>::downcast_mut::<T>(self)
    }
}

impl Error + 'static + Send + Sync {
    /// Forwards to the method defined on the type `Any`.
    #[inline]
    pub fn is<T: Error + 'static>(&self) -> bool {
        <Error + 'static>::is::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    #[inline]
    pub fn downcast_ref<T: Error + 'static>(&self) -> Option<&T> {
        <Error + 'static>::downcast_ref::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    #[inline]
    pub fn downcast_mut<T: Error + 'static>(&mut self) -> Option<&mut T> {
        <Error + 'static>::downcast_mut::<T>(self)
    }
}

impl<T: error::Error> Error for T {}
