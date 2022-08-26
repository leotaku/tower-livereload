use http::{header, Response};

pub trait Predicate<Response>: Copy {
    type Response;

    fn check(&mut self, response: Response) -> Result<Self::Response, ()>;
}

#[derive(Copy, Clone, Debug)]
pub struct ContentTypeStartsWithPredicate<Patt>(Patt);

impl<Patt: AsRef<str> + Copy> ContentTypeStartsWithPredicate<Patt> {
    pub fn new(pattern: Patt) -> Self {
        ContentTypeStartsWithPredicate(pattern)
    }
}

impl<T, Patt: AsRef<str> + Copy> Predicate<Response<T>> for ContentTypeStartsWithPredicate<Patt> {
    type Response = Response<T>;

    fn check(&mut self, response: Response<T>) -> Result<Self::Response, ()> {
        let matches = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|val| val.to_str().ok().map(|s| s.starts_with(self.0.as_ref())))
            .unwrap_or(false);

        if matches {
            Ok(response)
        } else {
            Err(())
        }
    }
}
