//! Predicates for matching HTTP responses and requests.
//!
//! Note that in addition to the predicates exported by this module,
//! [`Predicate`] is also implemented for `Fn(&T) -> bool + Copy`,
//! which is useful for quickly getting an arbitrary predicate.
use http::{header, Response};

/// Trait for predicates that check if a value matches them.
pub trait Predicate<T>: Clone {
    /// Check if the predicate matches the given value.
    fn check(&mut self, thing: &T) -> bool;
}

/// A predicate that matches based on [`Content-Type`] header.
///
/// [`Content-Type`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type
#[derive(Copy, Clone, Debug)]
pub struct ContentTypeStartsWithPredicate<Patt>(Patt);

impl<Patt: AsRef<str> + Copy> ContentTypeStartsWithPredicate<Patt> {
    /// Create a new [`ContentTypeStartsWithPredicate`] predicate.
    pub fn new(pattern: Patt) -> Self {
        ContentTypeStartsWithPredicate(pattern)
    }
}

impl<T, Patt: AsRef<str> + Copy> Predicate<Response<T>> for ContentTypeStartsWithPredicate<Patt> {
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
pub struct AlwaysPredicate;

impl<T> Predicate<T> for AlwaysPredicate {
    fn check<'a>(&mut self, _request: &'a T) -> bool {
        true
    }
}

impl<T, F> Predicate<T> for F
where
    F: Fn(&T) -> bool + Clone,
{
    fn check<'a>(&mut self, request: &'a T) -> bool {
        (self)(request)
    }
}
