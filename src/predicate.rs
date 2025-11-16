//! Predicates for matching HTTP responses and requests.
//!
//! Note that in addition to the predicates exported by this module,
//! [`Predicate`] is also implemented for `Fn(&T) -> bool + Copy`,
//! which is useful for quickly constructing an arbitrary predicate.
use http::{header, Response};

/// Trait for predicates that check if a value matches them.
pub trait Predicate<T>: Copy {
    /// Check if the predicate matches the given value.
    fn check(&mut self, thing: &T) -> bool;
}

/// A predicate that matches based on [`Content-Type`] header.
///
/// [`Content-Type`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type
#[derive(Copy, Clone, Debug)]
pub struct ContentTypeStartsWith<Patt>(Patt);

impl<Patt: AsRef<str> + Copy> ContentTypeStartsWith<Patt> {
    /// Create a new [`ContentTypeStartsWith`] predicate.
    pub fn new(pattern: Patt) -> Self {
        ContentTypeStartsWith(pattern)
    }
}

impl<T, Patt: AsRef<str> + Copy> Predicate<Response<T>> for ContentTypeStartsWith<Patt> {
    fn check(&mut self, response: &Response<T>) -> bool {
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|val| val.to_str().ok().map(|s| s.starts_with(self.0.as_ref())))
            .unwrap_or(false)
    }
}

/// A predicate that matches any request or response.
#[derive(Copy, Clone, Debug)]
pub struct Always;

impl<T> Predicate<T> for Always {
    fn check(&mut self, _thing: &T) -> bool {
        true
    }
}

impl<T, F> Predicate<T> for F
where
    F: Fn(&T) -> bool + Copy,
{
    fn check(&mut self, thing: &T) -> bool {
        (self)(thing)
    }
}
