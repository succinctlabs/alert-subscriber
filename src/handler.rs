use tracing::Level;

use std::future::Future;

/// The `seal` module for sending Succinct `Seal` alerts.
pub mod seal;
pub use seal::*;

pub trait AlertHandler: Send + 'static {
    type Error: std::error::Error;

    fn handle_alert(&self, level: &Level, msg: &str) -> impl AlertFuture<Self::Error>;
}

/// The future type returned by the `handle_alert` method.
pub trait AlertFuture<Error>: Future<Output = Result<(), Error>> + Send {}

/// A generic implementation of `AlertFuture` for any future that returns a `Result` and is `Send`.
impl<F, Error> AlertFuture<Error> for F where F: Future<Output = Result<(), Error>> + Send {}
