use http::{header, Response};

pub trait Predicate<T>: Clone {
    fn check(&mut self, thing: &T) -> bool;
}

#[derive(Copy, Clone, Debug)]
pub struct ContentTypeStartsWithPredicate<Patt>(Patt);

impl<Patt: AsRef<str> + Copy> ContentTypeStartsWithPredicate<Patt> {
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
