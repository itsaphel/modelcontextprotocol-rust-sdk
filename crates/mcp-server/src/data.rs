use std::{ops::Deref, sync::Arc};

use serde::{de, Serialize};

use crate::context::{Context, FromContext};

/// A wrapper for shared state that can be accessed from within the server.
/// 
/// It doesn't need to be used, but by using it, you don't need to implement
/// FromContext yourself + ensure it is thread-safe.
pub struct Inject<T: ?Sized>(Arc<T>);

impl<T> Inject<T> {
    pub fn new(state: T) -> Inject<T> {
        Inject(Arc::new(state))
    }
}

// Pass function calls through to the inner object
impl<T: ?Sized> Deref for Inject<T> {
    type Target = Arc<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> Clone for Inject<T> {
    fn clone(&self) -> Inject<T> {
        Inject(Arc::clone(&self.0))
    }
}

impl<T: Default> Default for Inject<T> {
    fn default() -> Self {
        Inject::new(T::default())
    }
}

impl<T> Serialize for Inject<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}
impl<'de, T> de::Deserialize<'de> for Inject<T>
where
    T: de::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(Inject::new(T::deserialize(deserializer)?))
    }
}

/// Implement ability to get a Data<T> from the context
impl<T: 'static> FromContext for Inject<T> {
    fn from_context(ctx: &Context) -> Self {
        if let Some(obj) = ctx.get::<Inject<T>>() {
            obj.clone()
        } else {
            panic!("Tried to inject an object not in the MCPServer's state!")
        }
    }
}
