//! The tests give one admin connection to the helpers of the crate and to their own driver
//! statements, so a test client also gives its driver client. Production code has no such path.

use std::ops::{Deref, DerefMut};

use super::Client;

impl Deref for Client {
    type Target = tokio_postgres::Client;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

impl<'a> Deref for super::Transaction<'a> {
    type Target = tokio_postgres::Transaction<'a>;

    fn deref(&self) -> &Self::Target {
        &self.transaction
    }
}
