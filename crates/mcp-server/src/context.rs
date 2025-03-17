use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

/// Store states registered with the MCPServer.
/// 
/// structs must be registered in the context when the MCPServer is built to
/// be accessible from tool handlers.
#[derive(Default)]
pub struct Context {
    /// A map from type to the injected tool.
    map: HashMap<TypeId, Box<dyn Any>>,
}

impl Context {
    /// Insert a struct of type T into the context.
    pub fn insert<T: 'static>(&mut self, state: T) {
        self.map
            .insert(TypeId::of::<T>(), Box::new(state));
    }
    
    /// Get a reference to the struct of type T from the context.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }
}

/// A trait to go from a Context to a type T.
/// 
/// Implementing this for a struct allows that struct to be injected into tool
/// handlers as a parameter.
pub trait FromContext {
    fn from_context(ctx: &Context) -> Self;
}
