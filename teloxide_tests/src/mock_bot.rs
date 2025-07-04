//! Mock bot that sends requests to the fake server
use std::{
    env,
    fmt::Debug,
    hash::Hash,
    mem::discriminant,
    panic,
    sync::{atomic::AtomicI32, Arc, Mutex, MutexGuard, PoisonError},
};

use gag::Gag;
use lazy_static::lazy_static;
use teloxide::{
    dispatching::{
        dialogue::{ErasedStorage, GetChatId, InMemStorage, Storage},
        UpdateHandler,
    },
    error_handlers::ErrorHandler,
    prelude::*,
    types::{MaybeInaccessibleMessage, Me, UpdateKind},
};

// Needed for trait bound stuff
pub use crate::utils::DistributionKey;
use crate::{
    dataset::{IntoUpdate, MockMe},
    listener::InsertingListener,
    server,
    server::ServerManager,
    state::State,
    utils::{assert_eqn, default_distribution_function, find_chat_id},
};

lazy_static! {
    static ref BOT_LOCK: Mutex<()> = Mutex::new(());
}

const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;

/// A mocked bot that sends requests to the fake server
/// Please check the [`new`] function docs and [github examples](https://github.com/LasterAlex/teloxide_tests/tree/master/examples) for more information.
///
/// If you are having troubles with generics while trying to store `MockBot`, just do this:
///
/// `MockBot<Box<dyn std::error::Error + Send + Sync>, teloxide_tests::mock_bot::DistributionKey>`
///
/// [`new`]: crate::MockBot::new
pub struct MockBot<Err, Key> {
    /// The bot with a fake server url
    pub bot: Bot,
    /// The thing that dptree::entry() returns
    pub handler_tree: UpdateHandler<Err>,
    /// Updates to send as user
    pub updates: Vec<Update>,
    /// Bot parameters are here
    pub me: Me,
    /// If you have something like a state, you should add the storage here using .dependencies()
    pub dependencies: DependencyMap,
    /// The stack size of the runtime for running updates
    pub stack_size: usize,

    distribution_f: fn(&Update) -> Option<Key>,
    error_handler: Arc<dyn ErrorHandler<Err> + Send + Sync>,

    current_update_id: AtomicI32,
    state: Arc<Mutex<State>>,
    _bot_lock: Option<MutexGuard<'static, ()>>,
}

impl<Err> MockBot<Err, DistributionKey>
where
    Err: Debug + Send + Sync + 'static,
{
    /// Creates a new MockBot, using something that can be turned into Updates, and a handler tree.
    /// You can't create a new bot while you have another bot in scope. Otherwise you will have a
    /// lot of race conditions. If you still somehow manage to create two bots at the same time
    /// (idk how),
    /// please look into [this crate for serial testing](https://crates.io/crates/serial_test)
    ///
    /// The `update` is just any Mock type, like `MockMessageText` or `MockCallbackQuery` or
    /// `vec![MockMessagePhoto]` if you want! All updates will be sent consecutively and asynchronously.
    /// The `handler_tree` is the same as in `dptree::entry()`, you will need to make your handler
    /// tree into a separate function, like this:
    /// ```no_run
    /// use teloxide::dispatching::UpdateHandler;
    /// fn handler_tree() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    ///     teloxide::dptree::entry() /* your handlers go here */
    /// }
    /// ```
    ///
    /// # Full example
    ///
    /// ```no_run
    /// use teloxide::dispatching::UpdateHandler;
    /// use teloxide::types::Update;
    /// use teloxide_tests::{MockBot, MockMessageText};
    /// use teloxide::dispatching::dialogue::GetChatId;
    /// use teloxide::prelude::*;
    ///
    /// fn handler_tree() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    ///     teloxide::dptree::entry().endpoint(|update: Update, bot: Bot| async move {
    ///         bot.send_message(update.chat_id().unwrap(), "Hello!").await?;
    ///         Ok(())
    ///     })
    /// }
    ///
    /// #[tokio::main]  // Change for tokio::test in your implementation
    /// async fn main() {
    ///     let mut bot = MockBot::new(MockMessageText::new().text("Hi!"), handler_tree());
    ///     bot.dispatch().await;
    ///     let responses = bot.get_responses();
    ///     let message = responses
    ///         .sent_messages
    ///         .last()
    ///         .expect("No sent messages were detected!");
    ///     assert_eq!(message.text(), Some("Hello!"));
    /// }
    /// ```
    ///
    pub fn new<T>(
        update: T, // This 'T' is just anything that can be turned into an Update, like a
        // MockMessageText or MockCallbackQuery, or a vec[MockMessagePhoto] if you want!
        handler_tree: UpdateHandler<Err>,
    ) -> Self
    where
        T: IntoUpdate, // And that code just "proves" that it can be turned into an update
        Err: Debug,
    {
        let _ = pretty_env_logger::try_init();

        let token = "1234567890:QWERTYUIOPASDFGHJKLZXCVBNMQWERTYUIO";
        let bot = Bot::new(token);
        let current_update_id = AtomicI32::new(42);
        let state = Arc::new(Mutex::new(State::default()));

        // If the lock is poisoned, we don't care, some other bot panicked and can't do anything
        let lock = Some(BOT_LOCK.lock().unwrap_or_else(PoisonError::into_inner));

        Self {
            bot,
            me: MockMe::new().build(),
            updates: update.into_update(&current_update_id),
            handler_tree,
            dependencies: DependencyMap::new(),
            stack_size: DEFAULT_STACK_SIZE,
            error_handler: LoggingErrorHandler::new(),
            distribution_f: default_distribution_function,
            _bot_lock: lock,
            current_update_id,
            state,
        }
    }
}

// Trait bound things.
impl<Err, Key> MockBot<Err, Key>
where
    Err: Debug + Send + Sync + 'static,
    Key: Hash + Eq + Clone + Send + 'static,
{
    /// Same as [`new`], but it inserts a distribution_function into the dispatcher
    ///
    /// [`new`]: crate::MockBot::new
    // It is its own function instead of `.distribution_function` setter because of the Key
    // generic. If `new` sets the Key to DefaultKey, it's impossible to swich back to a different
    // one, even if it fits all the trait bounds.
    pub fn new_with_distribution_function<T>(
        update: T,
        handler_tree: UpdateHandler<Err>,
        f: fn(&Update) -> Option<Key>,
    ) -> Self
    where
        T: IntoUpdate,
        Err: Debug,
    {
        // Again, trait bounds stuff, the generic Key is hard to work around
        let MockBot {
            bot,
            me,
            updates,
            handler_tree,
            dependencies,
            stack_size,
            error_handler,
            distribution_f: _,
            _bot_lock,
            current_update_id,
            state,
        } = MockBot::new(update, handler_tree);

        Self {
            bot,
            me,
            updates,
            handler_tree,
            dependencies,
            stack_size,
            error_handler,
            distribution_f: f,
            _bot_lock,
            current_update_id,
            state,
        }
    }

    /// Sets the dependencies of the dptree. The same as deps![] in bot dispatching.
    /// Just like in this teloxide example: <https://github.com/teloxide/teloxide/blob/master/crates/teloxide/examples/dialogue.rs>
    /// You can use it to add dependencies to your handler tree.
    /// For more examples - look into `get_state` method documentation
    pub fn dependencies(&mut self, deps: DependencyMap) {
        self.dependencies = deps;
    }

    /// Sets the bot parameters, like supports_inline_queries, first_name, etc.
    pub fn me(&mut self, me: MockMe) {
        self.me = me.build();
    }

    /// Sets the updates. Useful for reusing the same mocked bot instance in different tests
    /// Reminder: You can pass in `vec![MockMessagePhoto]` or something else!
    pub fn update<T: IntoUpdate>(&mut self, update: T) {
        self.updates = update.into_update(&self.current_update_id);
    }

    /// Sets the error_handler for Dispather
    pub fn error_handler(&mut self, handler: Arc<dyn ErrorHandler<Err> + Send + Sync>) {
        self.error_handler = handler;
    }

    /// Just inserts the updates into the state, returning them
    fn insert_updates(&self, updates: &mut [Update]) {
        for update in updates.iter_mut() {
            match update.kind.clone() {
                UpdateKind::Message(mut message) => {
                    // Add the message to the list of messages, so the bot can interact with it
                    self.state.lock().unwrap().add_message(&mut message);
                    update.kind = UpdateKind::Message(message.clone());
                }
                UpdateKind::EditedMessage(mut message) => {
                    self.state.lock().unwrap().edit_message(&mut message);
                    update.kind = UpdateKind::EditedMessage(message.clone());
                }
                UpdateKind::CallbackQuery(mut callback) => {
                    if let Some(MaybeInaccessibleMessage::Regular(ref mut message)) =
                        callback.message
                    {
                        self.state.lock().unwrap().add_message(message);
                    }
                    update.kind = UpdateKind::CallbackQuery(callback.clone());
                }
                _ => {}
            }
        }
    }

    async fn run_updates(&self, bot: Bot, updates: Vec<Update>) {
        let handler_tree = self.handler_tree.clone();
        let deps = self.dependencies.clone();
        let stack_size = self.stack_size;
        let distribution_f = self.distribution_f.clone();
        let error_handler = self.error_handler.clone();

        tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .thread_stack_size(stack_size) // Not needed, but just in case
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                Dispatcher::builder(bot.clone(), handler_tree.clone())
                    .dependencies(deps)
                    .distribution_function(distribution_f)
                    .error_handler(error_handler)
                    .build()
                    .dispatch_with_listener(
                        InsertingListener { updates },
                        LoggingErrorHandler::new(),
                    )
                    .await;
            });
        })
        .await
        .expect("Dispatcher panicked!");
    }

    /// Actually dispatches the bot, calling the update through the handler tree.
    /// All the requests made through the bot will be stored in `responses`, and can be retrieved
    /// with `get_responses`. All the responses are unique to that dispatch, and will be erased for
    /// every new dispatch.
    ///
    /// This method overrides env variables `TELOXIDE_TOKEN` and `TELOXIDE_API_URL`, so anyone can
    /// call `Bot::from_env()` and get an actual bot that is connected to the fake server
    pub async fn dispatch(&mut self) {
        self.state.lock().unwrap().reset();

        let server = ServerManager::start(self.me.clone(), self.state.clone())
            .await
            .unwrap();

        let mut updates = self.updates.clone();
        self.insert_updates(&mut updates);

        let api_url = reqwest::Url::parse(&format!("http://127.0.0.1:{}", server.port)).unwrap();
        let bot = self.bot.clone().set_api_url(api_url.clone());

        env::set_var("TELOXIDE_TOKEN", bot.token());
        env::set_var("TELOXIDE_API_URL", api_url.to_string());

        self.run_updates(bot, updates).await;

        server.stop().await.unwrap();
    }

    /// Returns the responses stored in `responses`
    /// Should be treated as a variable, because it kinda is
    pub fn get_responses(&self) -> server::Responses {
        self.state.lock().unwrap().responses.clone()
    }

    async fn get_potential_storages<S>(
        &self,
    ) -> (
        Option<Arc<Arc<InMemStorage<S>>>>,
        Option<Arc<Arc<ErasedStorage<S>>>>,
    )
    where
        S: Send + 'static + Clone,
    {
        let default_panic = panic::take_hook();
        let in_mem_storage: Option<Arc<Arc<InMemStorage<S>>>>;
        let erased_storage: Option<Arc<Arc<ErasedStorage<S>>>>;
        // No trace storage cuz who uses it
        let dependencies = Arc::new(self.dependencies.clone());
        // Get dependencies into Arc cuz otherwise it complaints about &self being moved

        panic::set_hook(Box::new(|_| {
            // Do nothing to ignore the panic
        }));
        let print_gag = Gag::stderr().unwrap(); // Otherwise the panic will be printed
        in_mem_storage = std::thread::spawn(move || {
            // Try to convert one of dptrees fields into an InMemStorage
            dependencies.get()
        })
        .join()
        .ok();

        let dependencies = Arc::new(self.dependencies.clone());
        // Dependencies were moved to a prev. thread, so create a new one
        erased_storage = std::thread::spawn(move || {
            // The same for ErasedStorage
            dependencies.get()
        })
        .join()
        .ok();

        panic::set_hook(default_panic); // Restore the default panic hook
        drop(print_gag);
        (in_mem_storage, erased_storage)
    }

    /// Sets the state of the dialogue, if the storage exists in dependencies
    /// Panics if no storage was found
    ///
    /// The only supported storages are `InMemStorage` and `ErasedStorage`,
    /// using raw storages without `.erase()` is not supported.
    ///
    /// For example on how to make `ErasedStorage` from `RedisStorage` or `SqliteStorage` go to [this teloxide example](https://github.com/teloxide/teloxide/blob/master/crates/teloxide/examples/db_remember.rs#L41)
    ///
    /// # Example
    /// ```no_run
    /// use teloxide::dispatching::UpdateHandler;
    /// use teloxide::types::Update;
    /// use teloxide_tests::{MockBot, MockMessageText};
    /// use teloxide::dispatching::dialogue::GetChatId;
    /// use teloxide::prelude::*;
    /// use teloxide::{
    ///     dispatching::{
    ///         dialogue::{self, InMemStorage},
    ///         UpdateFilterExt,
    ///     }
    /// };
    /// use dptree::deps;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
    /// enum State {
    ///     #[default]
    ///     Start,
    ///     NotStart
    /// }
    ///
    /// type MyDialogue = Dialogue<State, InMemStorage<State>>;
    ///
    /// fn handler_tree() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    ///     dialogue::enter::<Update, InMemStorage<State>, State, _>().endpoint(|update: Update, bot: Bot, dialogue: MyDialogue| async move {
    ///         let message = bot.send_message(update.chat_id().unwrap(), "Hello!").await?;
    ///         dialogue.update(State::NotStart).await?;
    ///         Ok(())
    ///     })
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut bot = MockBot::new(MockMessageText::new().text("Hi!"), handler_tree());
    ///     bot.dependencies(deps![InMemStorage::<State>::new()]);
    ///     bot.set_state(State::Start).await;
    ///     // Yes, Start is the default state, but this just shows how it works
    ///
    ///     bot.dispatch().await;
    ///
    ///     let state: State = bot.get_state().await;
    ///     // The `: State` type annotation is nessessary! Otherwise the compiler wont't know, what to return
    ///     assert_eq!(state, State::NotStart);
    ///
    ///     let responses = bot.get_responses();
    ///     let message = responses
    ///         .sent_messages
    ///         .last()
    ///         .expect("No sent messages were detected!");
    ///     assert_eq!(message.text(), Some("Hello!"));
    /// }
    /// ```
    ///
    pub async fn set_state<S>(&self, state: S)
    where
        S: Send + 'static + Clone,
    {
        let (in_mem_storage, erased_storage) = self.get_potential_storages().await;
        let first_update = self.updates.first().expect("No updates were detected!");
        let chat_id = match first_update.chat_id() {
            Some(chat_id) => chat_id,
            None => match find_chat_id(serde_json::to_value(first_update).unwrap()) {
                Some(id) => ChatId(id),
                None => {
                    log::error!("No chat id was detected in the update! Did you send an update without a chat identifier? Like MockCallbackQuery without an attached message?");
                    panic!("No chat id was detected!");
                }
            },
        };
        if let Some(storage) = in_mem_storage {
            // If memory storage exists
            (*storage)
                .clone()
                .update_dialogue(chat_id, state)
                .await
                .expect("Failed to update dialogue");
        } else if let Some(storage) = erased_storage {
            // If erased storage exists
            (*storage)
                .clone()
                .update_dialogue(chat_id, state)
                .await
                .expect("Failed to update dialogue");
        } else {
            log::error!("No storage was detected! Did you add it to bot.dependencies(deps![get_bot_storage().await]); ? Did you specify the type ::<State> ?");
            panic!("No storage was detected! Did you add it to bot.dependencies(deps![get_bot_storage().await]); ? Did you specify the type ::<State> ?");
        }
    }

    /// Helper function to fetch the state of the dialogue and assert its value
    pub async fn assert_state<S>(&self, state: S)
    where
        S: Send + Default + 'static + Clone + Debug + PartialEq,
    {
        assert_eqn!(self.get_state::<S>().await, state, "States are not equal!")
    }

    /// Gets the state of the dialogue, if the storage exists in dependencies
    /// Panics if no storage was found
    /// You need to use type annotation to get the state, please refer to the [`set_state`]
    /// documentation example
    ///
    /// [`set_state`]: crate::MockBot::set_state
    pub async fn get_state<S>(&self) -> S
    where
        S: Send + Default + 'static + Clone,
    {
        self.try_get_state().await.unwrap_or(S::default())
    }

    /// Same as [`get_state`], but returns None if the state is None, instead of the default
    ///
    /// [`get_state`]: crate::MockBot::get_state
    pub async fn try_get_state<S>(&self) -> Option<S>
    where
        S: Send + 'static + Clone,
    {
        let (in_mem_storage, erased_storage) = self.get_potential_storages().await;
        let first_update = self.updates.first().expect("No updates were detected!");
        let chat_id = match first_update.chat_id() {
            Some(chat_id) => chat_id,
            None => match find_chat_id(serde_json::to_value(first_update).unwrap()) {
                Some(id) => ChatId(id),
                None => {
                    panic!("No chat id was detected!");
                }
            },
        };
        if let Some(storage) = in_mem_storage {
            // If memory storage exists
            (*storage)
                .clone()
                .get_dialogue(chat_id)
                .await
                .ok()
                .flatten()
        } else if let Some(storage) = erased_storage {
            // If erased storage exists
            (*storage)
                .clone()
                .get_dialogue(chat_id)
                .await
                .ok()
                .flatten()
        } else {
            log::error!("No storage was detected! Did you add it to bot.dependencies(deps![get_bot_storage().await]); ? Did you specify the type ::<State> ?");
            panic!("No storage was detected! Did you add it to bot.dependencies(deps![get_bot_storage().await]); ? Did you specify the type ::<State> ?");
        }
    }

    //
    // Syntactic sugar
    //

    /// Dispatches and checks the last sent message text or caption. Pass in an empty string if you
    /// want the text or caption to be None
    pub async fn dispatch_and_check_last_text(&mut self, text_or_caption: &str) {
        self.dispatch().await;

        let responses = self.get_responses();
        let message = responses
            .sent_messages
            .last()
            .expect("No sent messages were detected!");

        if let Some(text) = message.text() {
            assert_eqn!(text, text_or_caption, "Texts are not equal!");
        } else if let Some(caption) = message.caption() {
            assert_eqn!(caption, text_or_caption, "Captions are not equal!");
        } else if !text_or_caption.is_empty() {
            panic!("Message has no text or caption!");
        }
    }

    /// Same as `dispatch_and_check_last_text`, but also checks the state. You need to derive
    /// PartialEq, Clone and Debug for the state like in `set_state` example
    pub async fn dispatch_and_check_last_text_and_state<S>(
        &mut self,
        text_or_caption: &str,
        state: S,
    ) where
        S: Send + Default + 'static + Clone + std::fmt::Debug + PartialEq,
    {
        self.dispatch().await;

        let responses = self.get_responses();
        let message = responses
            .sent_messages
            .last()
            .expect("No sent messages were detected!");

        if let Some(text) = message.text() {
            assert_eqn!(text, text_or_caption, "Texts are not equal!");
        } else if let Some(caption) = message.caption() {
            assert_eqn!(caption, text_or_caption, "Captions are not equal!");
        } else if !text_or_caption.is_empty() {
            panic!("Message has no text or caption!");
        }

        self.assert_state(state).await;
    }

    /// Same as `dispatch_and_check_last_text`, but also checks, if the variants of the state are the same
    ///
    /// For example, `State::Start { some_field: "value" }` and `State::Start { some_field: "other value" }` are the same in this function
    pub async fn dispatch_and_check_last_text_and_state_discriminant<S>(
        &mut self,
        text_or_caption: &str,
        state: S,
    ) where
        S: Send + PartialEq + Debug + Default + 'static + Clone,
    {
        self.dispatch().await;

        let responses = self.get_responses();
        let message = responses
            .sent_messages
            .last()
            .expect("No sent messages were detected!");

        if let Some(text) = message.text() {
            assert_eqn!(text, text_or_caption, "Texts are not equal!");
        } else if let Some(caption) = message.caption() {
            assert_eqn!(caption, text_or_caption, "Captions are not equal!");
        } else if !text_or_caption.is_empty() {
            panic!("Message has no text or caption!");
        }

        let got_state: S = self.get_state().await;
        if discriminant(&got_state) != discriminant(&state) {
            assert_eqn!(got_state, state, "State variants are not equal!")
        }
    }

    /// Just checks the state after dispathing the update, like `dispatch_and_check_last_text_and_state`
    pub async fn dispatch_and_check_state<S>(&mut self, state: S)
    where
        S: Send + Default + 'static + Clone + std::fmt::Debug + PartialEq,
    {
        self.dispatch().await;
        self.assert_state(state).await;
    }

    /// Just checks the state discriminant after dispathing the update, like `dispatch_and_check_last_text_and_state_discriminant`
    pub async fn dispatch_and_check_state_discriminant<S>(&mut self, state: S)
    where
        S: Send + Debug + PartialEq + Default + 'static + Clone,
    {
        self.dispatch().await;
        let got_state: S = self.get_state().await;
        if discriminant(&got_state) != discriminant(&state) {
            assert_eqn!(got_state, state, "State variants are not equal!")
        }
    }
}
