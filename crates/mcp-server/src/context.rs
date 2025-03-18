use serde::{de, Serialize};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ops::Deref,
    sync::Arc,
};

/// Store of structs that may be injected in MCPServer tool handlers.
/// 
/// Registration must happen when the MCPServer is built. Afterwards, this HashMap cannot be modified.
#[derive(Default)]
pub struct Context {
    /// A map from type to the injected tool.
    map: HashMap<TypeId, Box<dyn Any>>,
}

impl Context {
    /// Register a struct of type T in the context.
    pub fn insert<T: 'static>(&mut self, state: Inject<T>) {
        self.map.insert(TypeId::of::<Inject<T>>(), Box::new(state));
    }

    /// Get a reference to a struct of type T from the context.
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

/// Inject wraps structs that can be injected into tool handler functions. These allow tool handlers
/// to have side effects and access shared state, outside of the handler's parameters.
/// The struct must be registered in the MCPServer's context to be injected.
/// 
/// # Examples
/// 
/// ```
/// # use mcp_server::context::Inject;
/// # use mcp_macros::tool;
/// # use mcp_core::ToolError;
/// struct MyState { counter: i32 }
/// 
/// #[tool]
/// async fn my_tool(my_state: Inject<MyState>) -> Result<(), ToolError> {
///    // MyState is injected from the MCPServer's context
///   my_state.counter += 1;
///   println!("{}", my_state.counter);
///   Ok(())
/// }
/// ```
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
