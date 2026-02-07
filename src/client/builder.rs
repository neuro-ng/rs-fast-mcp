use crate::client::{Client, ClientTransport};
use std::time::Duration;

pub struct ClientBuilder {
    transport: Box<dyn ClientTransport>,
    timeout: Option<Duration>,
}

impl ClientBuilder {
    pub fn new(transport: Box<dyn ClientTransport>) -> Self {
        Self {
            transport,
            timeout: None,
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Client {
        let mut client = Client::new(self.transport);
        // We need a way to set timeout on existing client or new() should take it.
        // Since Client::new uses `timeout: None` hardcoded in my previous edit, 
        // I need to update Client definition to either expose a setter (interior mutability?) 
        // or update `new` to take options?
        // Or cleaner: `Client::new` stays simple, but I add `set_timeout`?
        // Wait, Client is immutable (fields are not pub and not Mutex). 
        // So I must construct it correctly.
        // I should have updated `Client::new` to accept options OR used interior mutability.
        // Actually, I can use struct update syntax if fields were pub, but they aren't.
        // I'll update `Client::from_builder` or similar logic.
        // Or simply:
        
        /*
        let mut client = Client::new(self.transport);
        client.timeout = self.timeout; // Field is private.
        */
        
        // Revised plan: Update Client to have a `with_timeout` or `from_parts` or equivalent.
        // Or better: Update `Client::new` to `Client::_new` private, and public `new` calls it.
        // Actually I will add `pub(crate) fn set_timeout(&mut self, timeout: Option<Duration>)`
        // But `Client` is returned by value, so `mut` is fine before returning.
        
        client.set_timeout(self.timeout);
        client
    }
}
